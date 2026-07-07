use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};
use ratatui::{Frame, Terminal};
use rave::decode_helpers::{
    b_immediate, div, divu, divuw, divw, i_immediate, j_immediate, mulh, mulhsu, mulhu, rem, remu,
    remuw, remw, s_immediate, sign_extend_word, upper_immediate,
};
use rave::{
    decode_compressed_instruction, encoded_instruction_size, AddressAccess, Command, Debugger,
    Machine, StopReason, REGISTER_NAMES,
};
use std::collections::BTreeSet;
use std::io::{self, stdout};
use std::time::{Duration, Instant};

const HELP: &str =
    "start | step(s) | next(n) | break(b) ADDR | continue(c) | uart TEXT | set REG VALUE | undo(u) | F7 page tables/code | PgUp/PgDown scroll | quit(q)";
const EXIT_CONFIRMATION_WINDOW: Duration = Duration::from_secs(1);
const PC_INDEX: usize = 32;
const MSIP_INDEX: usize = 33;
const MTIME_INDEX: usize = 34;
const MTIMECMP_INDEX: usize = 35;
const SATP_INDEX: usize = 36;
const FIRST_PSEUDO_REGISTER_INDEX: usize = MSIP_INDEX;
const LAST_EDITABLE_INDEX: usize = MTIMECMP_INDEX;
const INSTRUCTION_SIZE: u64 = 4;
const MOUSE_WHEEL_CODE_ROWS: i64 = 3;
const PANEL_BORDER_HEIGHT: u16 = 2;
const BRANCH_OPCODE: u32 = 0x63;
const INSTRUCTION_ECALL: u32 = 0x0000_0073;
const INSTRUCTION_MRET: u32 = 0x3020_0073;
const INSTRUCTION_SRET: u32 = 0x1020_0073;
const CSR_MTVEC: u16 = 0x305;
const CSR_MEPC: u16 = 0x341;
const CSR_SEPC: u16 = 0x141;
const REGISTER_NAME_WIDTH: u16 = 8;
const REGISTER_VALUE_WIDTH: u16 = 18;
const REGISTER_TABLE_DECORATION_WIDTH: u16 = 6;
const REGISTER_PANE_WIDTH: u16 =
    REGISTER_NAME_WIDTH + REGISTER_VALUE_WIDTH + REGISTER_TABLE_DECORATION_WIDTH;
const INSTRUCTION_CLASS_WIDTH: usize = 10;
const SATP_MODE_SHIFT: u64 = 60;
const SATP_MODE_BARE: u64 = 0;
const SATP_MODE_SV39: u64 = 8;
const SATP_PPN_MASK: u64 = (1 << 44) - 1;
const PAGE_SHIFT: u64 = 12;
const PTE_SIZE: u64 = 8;
const PTE_V: u64 = 1 << 0;
const PTE_R: u64 = 1 << 1;
const PTE_W: u64 = 1 << 2;
const PTE_X: u64 = 1 << 3;
const PTE_U: u64 = 1 << 4;
const PTE_G: u64 = 1 << 5;
const PTE_A: u64 = 1 << 6;
const PTE_D: u64 = 1 << 7;
const PTE_PPN_SHIFT: u64 = 10;
const PTE_PPN_MASK: u64 = (1 << 44) - 1;
const CSR_SATP: u16 = 0x180;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BranchInfo {
    mnemonic: &'static str,
    rs1: usize,
    rs2: usize,
    target: u64,
    taken: bool,
    operator: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JumpInfo {
    mnemonic: &'static str,
    rd: usize,
    rs1: usize,
    target: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrapFlowInfo {
    mnemonic: &'static str,
    target_label: &'static str,
    target: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodeFlow {
    target: u64,
    taken: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemInfo {
    mnemonic: &'static str,
    register: usize,
    rs1: usize,
    offset: i64,
    address: u64,
    physical_address: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AluInfo {
    mnemonic: &'static str,
    rd: usize,
    rs1: usize,
    rhs: AluRhs,
    result: u64,
    operator: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpperInfo {
    mnemonic: &'static str,
    rd: usize,
    immediate: u64,
    result: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AluRhs {
    Register(usize),
    Immediate(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CsrInfo {
    mnemonic: &'static str,
    rd: usize,
    csr: u16,
    operand: CsrOperand,
    old_value: u64,
    new_value: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CsrOperand {
    Register(usize),
    Immediate(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AmoInfo {
    mnemonic: &'static str,
    rd: usize,
    rs1: usize,
    rs2: usize,
    address: u64,
    physical_address: Option<u64>,
    width: AmoWidth,
    old_value: Option<u64>,
    new_value: Option<u64>,
    sc_success: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmoWidth {
    Word,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitChord {
    ControlC,
    ControlD,
}

#[derive(Debug, Clone, Copy)]
struct RegisterEdit {
    index: usize,
    previous_value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Command,
    RegisterSelect,
    RegisterEdit,
    UartInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainView {
    Code,
    PageTables,
}

struct App {
    mode: Mode,
    command: String,
    last_command: Option<String>,
    edit_value: String,
    uart_input: String,
    selected_register: usize,
    status: String,
    quit: bool,
    pending_exit: Option<(ExitChord, Instant)>,
    edit_history: Vec<RegisterEdit>,
    main_view: MainView,
    code_scroll_rows: i64,
    visible_code_rows: u64,
    page_table_scroll_rows: i64,
    visible_page_table_rows: u64,
}

impl App {
    fn new() -> Self {
        Self {
            mode: Mode::Command,
            command: String::new(),
            last_command: None,
            edit_value: String::new(),
            uart_input: String::new(),
            selected_register: 0,
            status: "loaded; use start, step, or continue".into(),
            quit: false,
            pending_exit: None,
            edit_history: Vec::new(),
            main_view: MainView::Code,
            code_scroll_rows: 0,
            visible_code_rows: 1,
            page_table_scroll_rows: 0,
            visible_page_table_rows: 1,
        }
    }
}

pub fn run(image: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut debugger = Debugger::new(image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE)?;
    let _screen = ScreenGuard::enter()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    let mut app = App::new();

    while !app.quit {
        terminal.draw(|frame| draw(frame, &debugger, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(key, &mut debugger, &mut app);
                }
                Event::Mouse(mouse) => {
                    handle_mouse(mouse, &mut app);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

struct ScreenGuard;

impl ScreenGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn handle_key(key: KeyEvent, debugger: &mut Debugger, app: &mut App) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        app.quit = true;
        return;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('z') {
        undo_last_edit(debugger, app);
        return;
    }
    if let Some(chord) = exit_chord(key) {
        confirm_exit(chord, app);
        return;
    }
    if key.code == KeyCode::F(7) {
        toggle_main_view(app);
        return;
    }
    if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
        let direction = if key.code == KeyCode::PageUp { -1 } else { 1 };
        scroll_main_view_page(app, direction);
        return;
    }
    app.pending_exit = None;

    match app.mode {
        Mode::Command => handle_command_key(key, debugger, app),
        Mode::RegisterSelect => handle_register_key(key, debugger, app),
        Mode::RegisterEdit => handle_edit_key(key, debugger, app),
        Mode::UartInput => handle_uart_input_key(key, debugger, app),
    }
}

fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    match mouse.kind {
        MouseEventKind::ScrollUp => scroll_main_view_rows(app, -MOUSE_WHEEL_CODE_ROWS),
        MouseEventKind::ScrollDown => scroll_main_view_rows(app, MOUSE_WHEEL_CODE_ROWS),
        _ => {}
    }
}

fn toggle_main_view(app: &mut App) {
    match app.main_view {
        MainView::Code => show_page_tables(app),
        MainView::PageTables => show_code(app),
    }
}

fn show_page_tables(app: &mut App) {
    app.main_view = MainView::PageTables;
    app.status = "page-table browser; press F7 for code view".into();
}

fn show_code(app: &mut App) {
    app.main_view = MainView::Code;
    app.status = "code view".into();
}

fn scroll_main_view_page(app: &mut App, direction: i64) {
    let rows = match app.main_view {
        MainView::Code => app.visible_code_rows,
        MainView::PageTables => app.visible_page_table_rows,
    }
    .saturating_sub(1)
    .max(1);
    scroll_main_view_rows(app, direction.saturating_mul(rows as i64));
}

fn scroll_main_view_rows(app: &mut App, rows: i64) {
    match app.main_view {
        MainView::Code => scroll_code_rows(app, rows),
        MainView::PageTables => scroll_page_table_rows(app, rows),
    }
}

fn scroll_code_rows(app: &mut App, rows: i64) {
    app.code_scroll_rows = app.code_scroll_rows.saturating_add(rows);
    app.status = if app.code_scroll_rows == 0 {
        "code view centered on current instruction".into()
    } else {
        format!(
            "code view offset by {} row(s); step, next, continue, or start to return to current",
            app.code_scroll_rows
        )
    };
}

fn scroll_page_table_rows(app: &mut App, rows: i64) {
    app.page_table_scroll_rows = app.page_table_scroll_rows.saturating_add(rows);
    app.status = format!(
        "page-table view offset by {} row(s)",
        app.page_table_scroll_rows
    );
}

fn follow_current_code(app: &mut App) {
    app.code_scroll_rows = 0;
}

fn command_recenters_code(command: &Command) -> bool {
    matches!(
        command,
        Command::Start | Command::Step | Command::Next | Command::Continue
    )
}

fn handle_command_key(key: KeyEvent, debugger: &mut Debugger, app: &mut App) {
    match key.code {
        KeyCode::Tab => app.mode = Mode::RegisterSelect,
        KeyCode::Enter => {
            submit_command(debugger, app);
        }
        KeyCode::Backspace => {
            app.command.pop();
        }
        KeyCode::Esc => app.command.clear(),
        KeyCode::F(5) => execute_command("continue", debugger, app),
        KeyCode::F(10) => execute_command("next", debugger, app),
        KeyCode::F(11) => execute_command("step", debugger, app),
        KeyCode::F(6) => enter_uart_input(app),
        KeyCode::Char('q') if app.command.is_empty() => app.quit = true,
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.command.push(character)
        }
        _ => {}
    }
}

fn handle_register_key(key: KeyEvent, debugger: &mut Debugger, app: &mut App) {
    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Tab | KeyCode::Esc => app.mode = Mode::Command,
        KeyCode::Up | KeyCode::Char('k') => {
            app.selected_register = app.selected_register.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.selected_register = (app.selected_register + 1).min(LAST_EDITABLE_INDEX)
        }
        KeyCode::Home => app.selected_register = 0,
        KeyCode::End => app.selected_register = LAST_EDITABLE_INDEX,
        KeyCode::Enter | KeyCode::Char('e') => {
            app.edit_value = format!("0x{:x}", selected_value(debugger, app.selected_register));
            app.mode = Mode::RegisterEdit;
        }
        KeyCode::Char('r') => execute_command("start", debugger, app),
        KeyCode::Char('s') | KeyCode::F(11) => execute_command("step", debugger, app),
        KeyCode::Char('n') | KeyCode::F(10) => execute_command("next", debugger, app),
        KeyCode::Char('c') | KeyCode::F(5) => execute_command("continue", debugger, app),
        KeyCode::Char('u') => undo_last_edit(debugger, app),
        KeyCode::Char('i') | KeyCode::F(6) => enter_uart_input(app),
        _ => {}
    }
}

fn handle_edit_key(key: KeyEvent, debugger: &mut Debugger, app: &mut App) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::RegisterSelect,
        KeyCode::Backspace => {
            app.edit_value.pop();
        }
        KeyCode::Enter => match rave::debugger_parse_number(&app.edit_value) {
            Ok(value) => {
                record_and_set(debugger, app, app.selected_register, value);
                app.status = format!("{} = {value:#018x}", register_label(app.selected_register));
                app.mode = Mode::RegisterSelect;
            }
            Err(error) => app.status = error.to_string(),
        },
        KeyCode::Char(character)
            if character.is_ascii_hexdigit()
                || character == 'x'
                || character == 'X'
                || character == '_' =>
        {
            app.edit_value.push(character)
        }
        _ => {}
    }
}

fn handle_uart_input_key(key: KeyEvent, debugger: &mut Debugger, app: &mut App) {
    match key.code {
        KeyCode::Esc => {
            app.uart_input.clear();
            app.mode = Mode::Command;
        }
        KeyCode::Backspace => {
            app.uart_input.pop();
        }
        KeyCode::Enter => submit_uart_input(debugger, app),
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.uart_input.push(character)
        }
        _ => {}
    }
}

fn enter_uart_input(app: &mut App) {
    app.uart_input.clear();
    app.mode = Mode::UartInput;
    app.status = "type UART input and press Enter to send newline".into();
}

fn submit_uart_input(debugger: &mut Debugger, app: &mut App) {
    let input = std::mem::take(&mut app.uart_input);
    let byte_count = input.len() + 1;
    match debugger.execute(Command::UartInput(input), Machine::INSTRUCTION_LIMIT) {
        Ok(_) => {
            app.mode = Mode::Command;
            match debugger.continue_execution(Machine::INSTRUCTION_LIMIT) {
                Ok(reason) => {
                    follow_current_code(app);
                    app.status =
                        format!("queued {byte_count} UART byte(s); {}", format_stop(reason));
                    if reason == StopReason::UartInput {
                        enter_uart_input(app);
                    }
                }
                Err(error) => app.status = error.to_string(),
            }
        }
        Err(error) => app.status = error.to_string(),
    }
}

fn execute_command(input: &str, debugger: &mut Debugger, app: &mut App) {
    let command = match input.parse::<Command>() {
        Ok(command) => command,
        Err(error) => {
            app.status = error.to_string();
            return;
        }
    };
    if command == Command::Quit {
        app.quit = true;
        return;
    }
    if command == Command::Help {
        app.status = HELP.into();
        return;
    }
    if command == Command::Undo {
        undo_last_edit(debugger, app);
        return;
    }
    if let Command::SetRegister { index, .. } = command {
        app.edit_history.push(RegisterEdit {
            index,
            previous_value: debugger.machine.cpu.register(index),
        });
    }
    let description = match &command {
        Command::Break(address) => Some(format!("breakpoint set at {address:#018x}")),
        Command::SetRegister { index, value } => {
            Some(format!("{} = {value:#018x}", register_label(*index)))
        }
        Command::UartInput(input) => Some(format!("queued {} UART byte(s)", input.len() + 1)),
        _ => None,
    };
    let recenter_code = command_recenters_code(&command);
    match debugger.execute(command, Machine::INSTRUCTION_LIMIT) {
        Ok(Some(reason)) => {
            if recenter_code {
                follow_current_code(app);
            }
            app.status = format_stop(reason);
            if reason == StopReason::UartInput {
                enter_uart_input(app);
            }
        }
        Ok(None) => {
            if recenter_code {
                follow_current_code(app);
            }
            app.status = description.unwrap_or_else(|| "ok".into());
        }
        Err(error) => app.status = error.to_string(),
    }
}

fn submit_command(debugger: &mut Debugger, app: &mut App) {
    let typed = std::mem::take(&mut app.command);
    let input = if typed.trim().is_empty() {
        app.last_command.clone().unwrap_or_else(|| "step".into())
    } else {
        typed
    };

    if input.parse::<Command>().is_ok() {
        app.last_command = Some(input.clone());
    }
    execute_command(&input, debugger, app);
}

fn format_stop(reason: StopReason) -> String {
    match reason {
        StopReason::Started => "program reset at entry point".into(),
        StopReason::Stepped => "executed one instruction".into(),
        StopReason::Breakpoint(address) => format!("breakpoint hit at {address:#018x}"),
        StopReason::UartInput => "guest is waiting for UART input".into(),
        StopReason::Halted(reason) => format!("guest halted: {reason:?}"),
    }
}

fn draw(frame: &mut Frame<'_>, debugger: &Debugger, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(REGISTER_PANE_WIDTH)])
        .split(outer[0]);
    draw_main_view(frame, body[0], debugger, app);
    draw_right_column(frame, body[1], debugger, app);
    draw_uart(frame, outer[1], debugger);
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().title(" Status ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        outer[2],
    );
    draw_prompt(frame, outer[3], app);
}

fn draw_main_view(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &mut App) {
    match app.main_view {
        MainView::Code => draw_code(frame, area, debugger, app),
        MainView::PageTables => draw_page_tables(frame, area, debugger, app),
    }
}

fn draw_code(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &mut App) {
    let pc = debugger.machine.cpu.pc;
    let code_rows = visible_code_rows(area);
    app.visible_code_rows = code_rows;
    let rows_before_pc = code_rows / 2;
    let first = scrolled_code_start(pc, rows_before_pc, app.code_scroll_rows);
    let addresses = code_addresses(first, code_rows, debugger);
    let last = addresses.last().copied().unwrap_or(first);
    let current_flow = read_display_instruction(pc, debugger)
        .ok()
        .and_then(|instruction| code_flow(instruction.expanded, pc, debugger));
    let lines: Vec<Line<'_>> = addresses
        .into_iter()
        .map(|address| {
            let current = address == pc;
            let breakpoint = debugger.breakpoints().contains(&address);
            let arrow = current_flow
                .as_ref()
                .map(|flow| branch_arrow(address, pc, flow.target, first, last))
                .unwrap_or("    ");
            let marker = match (current, breakpoint) {
                (true, true) => "=>●",
                (true, false) => "=> ",
                (false, true) => "  ●",
                (false, false) => "   ",
            };
            let base = if current {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if breakpoint {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };
            let mut spans = vec![
                Span::styled(
                    arrow,
                    base.patch(Style::default().fg(
                        if current_flow.as_ref().is_none_or(|flow| flow.taken) {
                            Color::LightGreen
                        } else {
                            Color::LightRed
                        },
                    )),
                ),
                Span::styled(format!("{marker} "), base),
                Span::styled(
                    format!("{address:#018x}"),
                    base.patch(Style::default().fg(Color::Cyan)),
                ),
            ];
            if let Ok(translation) = debugger.machine.cpu.translate_address_for_debug(
                &debugger.machine.bus,
                address,
                AddressAccess::Fetch,
            ) {
                if translation.paging_active {
                    spans.extend([
                        Span::styled(" -> ", base),
                        Span::styled(
                            format!("{:#018x}", translation.physical_address),
                            base.patch(Style::default().fg(Color::LightCyan)),
                        ),
                    ]);
                }
            }
            spans.push(Span::styled("  ", base));
            match read_display_instruction(address, debugger) {
                Ok(display) => {
                    spans.push(Span::styled(
                        display.encoding.clone(),
                        base.patch(Style::default().fg(Color::Magenta)),
                    ));
                    spans.push(Span::styled("  ", base));
                    spans.push(Span::styled(
                        format!(
                            "{:<width$}",
                            display.name(),
                            width = INSTRUCTION_CLASS_WIDTH
                        ),
                        base.patch(Style::default().fg(Color::Green)),
                    ));
                    let instruction = display.expanded;
                    if let Some(branch) = branch_info(instruction, address, debugger) {
                        spans.extend(branch_spans(&branch, base, debugger));
                    } else if let Some(jump) = decode_jump(instruction, address, debugger) {
                        let rd_name = if jump.rd == 0 {
                            "zero"
                        } else {
                            REGISTER_NAMES[jump.rd]
                        };
                        spans.extend([
                            Span::styled(
                                if jump.mnemonic == "jal" {
                                    format!("{} {},", jump.mnemonic, rd_name)
                                } else {
                                    format!(
                                        "{} {},{}({}) -> ",
                                        jump.mnemonic,
                                        rd_name,
                                        i_immediate(instruction),
                                        REGISTER_NAMES[jump.rs1]
                                    )
                                },
                                base,
                            ),
                            Span::styled(
                                format!("{:#018x}", jump.target),
                                base.patch(Style::default().fg(Color::Cyan)),
                            ),
                        ]);
                    } else if let Some(trap) = decode_trap_flow(instruction, debugger) {
                        spans.extend(trap_flow_spans(&trap, base));
                    } else if let Some(upper) = decode_upper(instruction, address) {
                        let rd_name = if upper.rd == 0 {
                            "zero"
                        } else {
                            REGISTER_NAMES[upper.rd]
                        };
                        spans.extend([
                            Span::styled(format!("{} {},", upper.mnemonic, rd_name), base),
                            Span::styled(
                                format!("{:#x}", upper.immediate),
                                base.patch(Style::default().fg(Color::Yellow)),
                            ),
                            Span::styled(" -> ", base),
                            Span::styled(
                                format!("{:#018x}", upper.result),
                                base.patch(Style::default().fg(Color::Cyan)),
                            ),
                        ]);
                    } else if let Some(misc) = decode_misc_mem(instruction) {
                        spans.push(Span::styled(misc, base));
                    } else if let Some(mem) = decode_mem(instruction, debugger) {
                        let register_name = if mem.register == 0 {
                            "zero"
                        } else {
                            REGISTER_NAMES[mem.register]
                        };
                        let base_val = debugger.machine.cpu.register(mem.rs1);
                        spans.extend([
                            Span::styled(
                                format!(
                                    "{} {},{}({}) [",
                                    mem.mnemonic,
                                    register_name,
                                    mem.offset,
                                    REGISTER_NAMES[mem.rs1]
                                ),
                                base,
                            ),
                            Span::styled(
                                format!("{base_val:#x}"),
                                base.patch(Style::default().fg(Color::Yellow)),
                            ),
                            Span::styled(format!(" + {}] @ ", mem.offset), base),
                            Span::styled(
                                format!("{:#018x}", mem.address),
                                base.patch(Style::default().fg(Color::Cyan)),
                            ),
                        ]);
                        if let Some(physical) = mem.physical_address {
                            spans.extend([
                                Span::styled(" -> ", base),
                                Span::styled(
                                    format!("{physical:#018x}"),
                                    base.patch(Style::default().fg(Color::LightCyan)),
                                ),
                            ]);
                        }
                    } else if let Some(amo) = decode_amo(instruction, debugger) {
                        spans.extend(amo_spans(&amo, base));
                    } else if let Some(alu) = decode_alu(instruction, debugger) {
                        let rd_name = if alu.rd == 0 {
                            "zero"
                        } else {
                            REGISTER_NAMES[alu.rd]
                        };
                        let lhs = debugger.machine.cpu.register(alu.rs1);
                        let (rhs_name, rhs_value) = match alu.rhs {
                            AluRhs::Register(rs2) => {
                                let val = debugger.machine.cpu.register(rs2);
                                (REGISTER_NAMES[rs2].to_string(), val)
                            }
                            AluRhs::Immediate(imm) => (imm.to_string(), imm as u64),
                        };
                        spans.extend([
                            Span::styled(
                                format!(
                                    "{} {},{},{} [",
                                    alu.mnemonic, rd_name, REGISTER_NAMES[alu.rs1], rhs_name
                                ),
                                base,
                            ),
                            Span::styled(
                                format!("{lhs:#x}"),
                                base.patch(Style::default().fg(Color::Yellow)),
                            ),
                            Span::styled(format!(" {} ", alu.operator), base),
                            Span::styled(
                                format!("{rhs_value:#x}"),
                                base.patch(Style::default().fg(Color::Yellow)),
                            ),
                            Span::styled("] -> ", base),
                            Span::styled(
                                format!("{:#018x}", alu.result),
                                base.patch(Style::default().fg(Color::Cyan)),
                            ),
                        ]);
                    } else if let Some(csr) = decode_csr(instruction, debugger) {
                        let rd_name = if csr.rd == 0 {
                            "zero"
                        } else {
                            REGISTER_NAMES[csr.rd]
                        };
                        let operand_name = match csr.operand {
                            CsrOperand::Register(rs1) => REGISTER_NAMES[rs1].to_string(),
                            CsrOperand::Immediate(value) => value.to_string(),
                        };
                        spans.extend([
                            Span::styled(
                                format!(
                                    "{} {},{},{} [old ",
                                    csr.mnemonic,
                                    rd_name,
                                    csr_name(csr.csr),
                                    operand_name
                                ),
                                base,
                            ),
                            Span::styled(
                                format!("{:#x}", csr.old_value),
                                base.patch(Style::default().fg(Color::Yellow)),
                            ),
                            Span::styled("] -> ", base),
                            Span::styled(
                                format!("{}={:#018x}", rd_name, csr.old_value),
                                base.patch(Style::default().fg(Color::Cyan)),
                            ),
                        ]);
                        if let Some(new_value) = csr.new_value {
                            spans.extend([
                                Span::styled(", ", base),
                                Span::styled(
                                    format!("{}={:#018x}", csr_name(csr.csr), new_value),
                                    base.patch(Style::default().fg(Color::Cyan)),
                                ),
                            ]);
                        }
                    }
                }
                Err(_) => spans.push(Span::styled(
                    "????????",
                    base.patch(Style::default().fg(Color::Red)),
                )),
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Code (● breakpoint) ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn draw_page_tables(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &mut App) {
    let rows = visible_code_rows(area);
    app.visible_page_table_rows = rows;
    let all_lines = page_table_lines(debugger);
    let start = scroll_start(app.page_table_scroll_rows, all_lines.len(), rows as usize);
    let visible: Vec<Line<'_>> = all_lines
        .into_iter()
        .skip(start)
        .take(rows as usize)
        .collect();
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .title(" Page tables (F7 code view) ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn scroll_start(scroll_rows: i64, len: usize, visible_rows: usize) -> usize {
    let max_start = len.saturating_sub(visible_rows.max(1));
    if scroll_rows <= 0 {
        0
    } else {
        (scroll_rows as usize).min(max_start)
    }
}

fn page_table_lines(debugger: &Debugger) -> Vec<Line<'static>> {
    let satp = debugger.machine.cpu.csr(CSR_SATP);
    let mode = satp >> SATP_MODE_SHIFT;
    let root = (satp & SATP_PPN_MASK) << PAGE_SHIFT;
    let mut lines = vec![Line::from(vec![
        Span::styled("satp ", Style::default().fg(Color::Blue)),
        Span::styled(format!("{satp:#018x}"), Style::default().fg(Color::Yellow)),
        Span::raw(format!("  mode {}", satp_mode_name(mode))),
    ])];

    if mode == SATP_MODE_BARE {
        lines.push(Line::from(
            "paging disabled: virtual addresses are physical addresses",
        ));
        return lines;
    }
    if mode != SATP_MODE_SV39 {
        lines.push(Line::from(format!("unsupported satp mode {mode}")));
        return lines;
    }

    lines.push(Line::from(vec![
        Span::styled("root ", Style::default().fg(Color::Blue)),
        Span::styled(
            format!("{root:#018x}"),
            Style::default().fg(Color::LightCyan),
        ),
    ]));
    let mut visited = BTreeSet::new();
    collect_page_table_lines(
        &debugger.machine.bus,
        root,
        2,
        0,
        0,
        &mut visited,
        &mut lines,
    );
    lines
}

fn satp_mode_name(mode: u64) -> &'static str {
    match mode {
        SATP_MODE_BARE => "Bare",
        SATP_MODE_SV39 => "Sv39",
        _ => "unknown",
    }
}

fn collect_page_table_lines(
    bus: &rave::Bus,
    table: u64,
    level: usize,
    va_prefix: u64,
    depth: usize,
    visited: &mut BTreeSet<u64>,
    lines: &mut Vec<Line<'static>>,
) {
    if !visited.insert(table) {
        lines.push(Line::from(format!(
            "{}cycle: page table {table:#018x} already visited",
            page_table_indent(depth)
        )));
        return;
    }

    let mut nonzero = 0usize;
    for index in 0..512u64 {
        let pte_address = table + index * PTE_SIZE;
        let Ok(pte) = bus.peek_u64(pte_address) else {
            continue;
        };
        if pte == 0 {
            continue;
        }
        nonzero += 1;
        let shift = PAGE_SHIFT + 9 * level as u64;
        let child_prefix = va_prefix | (index << shift);
        let flags = pte_flags(pte);
        let ppn = (pte >> PTE_PPN_SHIFT) & PTE_PPN_MASK;
        let physical = ppn << PAGE_SHIFT;
        let indent = page_table_indent(depth);

        if pte & PTE_V == 0 || (pte & PTE_W != 0 && pte & PTE_R == 0) {
            lines.push(Line::from(vec![
                Span::raw(format!("{indent}L{level} [{index:03}] invalid ")),
                Span::styled(
                    format!("pte@{pte_address:#018x}"),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(format!(" = {pte:#018x} {flags}")),
            ]));
            continue;
        }

        if pte & (PTE_R | PTE_X) == 0 {
            lines.push(Line::from(vec![
                Span::raw(format!("{indent}L{level} [{index:03}] table  ")),
                Span::styled(
                    format!("pte@{pte_address:#018x}"),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" -> "),
                Span::styled(
                    format!("{physical:#018x}"),
                    Style::default().fg(Color::LightCyan),
                ),
                Span::raw(format!(" {flags}")),
            ]));
            if level > 0 {
                collect_page_table_lines(
                    bus,
                    physical,
                    level - 1,
                    child_prefix,
                    depth + 1,
                    visited,
                    lines,
                );
            }
            continue;
        }

        let size = page_size_for_level(level);
        let va_start = sv39_sign_extend(child_prefix);
        let va_end = va_start.wrapping_add(size - 1);
        let pa_end = physical.wrapping_add(size - 1);
        lines.push(Line::from(vec![
            Span::raw(format!("{indent}L{level} [{index:03}] leaf   ")),
            Span::styled(
                format!("{va_start:#018x}"),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(".."),
            Span::styled(format!("{va_end:#018x}"), Style::default().fg(Color::Cyan)),
            Span::raw(" -> "),
            Span::styled(
                format!("{physical:#018x}"),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw(".."),
            Span::styled(
                format!("{pa_end:#018x}"),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw(format!(" {flags} {}", page_size_label(size))),
        ]));
    }

    if nonzero == 0 {
        lines.push(Line::from(format!(
            "{}L{level} table {table:#018x}: empty",
            page_table_indent(depth)
        )));
    }
}

fn page_table_indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn pte_flags(pte: u64) -> String {
    [
        (PTE_V, 'V'),
        (PTE_R, 'R'),
        (PTE_W, 'W'),
        (PTE_X, 'X'),
        (PTE_U, 'U'),
        (PTE_G, 'G'),
        (PTE_A, 'A'),
        (PTE_D, 'D'),
    ]
    .into_iter()
    .map(|(bit, label)| if pte & bit != 0 { label } else { '-' })
    .collect()
}

fn page_size_for_level(level: usize) -> u64 {
    1 << (PAGE_SHIFT + level as u64 * 9)
}

fn page_size_label(size: u64) -> &'static str {
    match size {
        0x1000 => "4K",
        0x20_0000 => "2M",
        0x4000_0000 => "1G",
        _ => "?",
    }
}

fn sv39_sign_extend(address: u64) -> u64 {
    let mask = (1u64 << 39) - 1;
    let value = address & mask;
    if value & (1 << 38) != 0 {
        value | !mask
    } else {
        value
    }
}

fn scrolled_code_start(pc: u64, rows_before_pc: u64, scroll_rows: i64) -> u64 {
    let centered = pc
        .saturating_sub(INSTRUCTION_SIZE.saturating_mul(rows_before_pc))
        .wrapping_add(pc & 1);
    let scroll_bytes = INSTRUCTION_SIZE.saturating_mul(scroll_rows.unsigned_abs());
    if scroll_rows < 0 {
        centered.saturating_sub(scroll_bytes)
    } else {
        centered.wrapping_add(scroll_bytes)
    }
}

struct DisplayInstruction {
    expanded: u32,
    encoding: String,
    compressed: Option<u16>,
}

impl DisplayInstruction {
    fn name(&self) -> &'static str {
        self.compressed
            .map(compressed_instruction_name)
            .unwrap_or_else(|| instruction_name(self.expanded))
    }
}

fn code_addresses(first: u64, rows: u64, debugger: &Debugger) -> Vec<u64> {
    let mut addresses = Vec::with_capacity(rows as usize);
    let mut address = first & !1;
    for _ in 0..rows {
        addresses.push(address);
        let physical = debugger
            .machine
            .cpu
            .translate_address_for_debug(&debugger.machine.bus, address, AddressAccess::Fetch)
            .map(|translation| translation.physical_address)
            .unwrap_or(address);
        let size = debugger
            .machine
            .bus
            .peek_u16(physical)
            .map(encoded_instruction_size)
            .unwrap_or(INSTRUCTION_SIZE);
        address = address.wrapping_add(size);
    }
    addresses
}

fn read_display_instruction(
    address: u64,
    debugger: &Debugger,
) -> Result<DisplayInstruction, Box<dyn std::error::Error>> {
    let physical = debugger
        .machine
        .cpu
        .translate_address_for_debug(&debugger.machine.bus, address, AddressAccess::Fetch)?
        .physical_address;
    let half = debugger.machine.bus.peek_u16(physical)?;
    if encoded_instruction_size(half) == INSTRUCTION_SIZE {
        let expanded = debugger.machine.bus.peek_u32(physical)?;
        Ok(DisplayInstruction {
            expanded,
            encoding: format!("{expanded:08x}"),
            compressed: None,
        })
    } else {
        Ok(DisplayInstruction {
            expanded: decode_compressed_instruction(half).unwrap_or(u32::from(half)),
            encoding: format!("    {half:04x}"),
            compressed: Some(half),
        })
    }
}

fn visible_code_rows(area: Rect) -> u64 {
    u64::from(area.height.saturating_sub(PANEL_BORDER_HEIGHT).max(1))
}

fn draw_uart(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger) {
    let output = debugger.machine.bus.uart_output();
    let lines: Vec<Line<'_>> = uart_output_text_lines(output, visible_uart_rows(area))
        .into_iter()
        .map(Line::from)
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!(" UART output ({} bytes) ", output.len()))
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn visible_uart_rows(area: Rect) -> usize {
    usize::from(area.height.saturating_sub(PANEL_BORDER_HEIGHT).max(1))
}

fn uart_output_text_lines(output: &[u8], max_lines: usize) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }

    let mut lines = vec![String::new()];
    for byte in output {
        match *byte {
            b'\n' => lines.push(String::new()),
            b'\r' => {}
            b'\t' => lines.last_mut().unwrap().push('\t'),
            0x20..=0x7e => lines.last_mut().unwrap().push(char::from(*byte)),
            _ => lines
                .last_mut()
                .unwrap()
                .push_str(&format!("\\x{byte:02x}")),
        }
    }

    let start = lines.len().saturating_sub(max_lines);
    lines[start..].to_vec()
}

fn draw_right_column(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(6)])
        .split(area);
    draw_registers(frame, chunks[0], debugger, app);
    draw_pseudo_registers(frame, chunks[1], debugger, app);
}

fn draw_registers(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &App) {
    let rows = (0..=PC_INDEX).map(|index| editable_row(debugger, app, index));
    let selected = (app.selected_register <= PC_INDEX).then_some(app.selected_register);
    let mut state = TableState::default().with_selected(selected);
    let title = register_pane_title(debugger, app);
    let table = editable_table(rows, ["register", "value"])
        .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_pseudo_registers(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &App) {
    let rows =
        (FIRST_PSEUDO_REGISTER_INDEX..=SATP_INDEX).map(|index| editable_row(debugger, app, index));
    let selected = (app.selected_register >= FIRST_PSEUDO_REGISTER_INDEX)
        .then(|| app.selected_register - FIRST_PSEUDO_REGISTER_INDEX);
    let mut state = TableState::default().with_selected(selected);
    let table = editable_table_without_header(rows).block(
        Block::default()
            .title(pseudo_register_pane_title(app))
            .borders(Borders::ALL),
    );
    frame.render_stateful_widget(table, area, &mut state);
}

fn editable_row(debugger: &Debugger, app: &App, index: usize) -> Row<'static> {
    let live_edit = app.mode == Mode::RegisterEdit && index == app.selected_register;
    let value = if live_edit {
        rave::debugger_parse_number(&app.edit_value).ok()
    } else {
        Some(selected_value(debugger, index))
    };
    let value_text = value
        .map(|value| format!("{value:#018x}"))
        .unwrap_or_else(|| app.edit_value.clone());
    Row::new(vec![
        Cell::from(register_label(index)).style(Style::default().fg(Color::Blue)),
        Cell::from(value_text).style(Style::default().fg(if live_edit {
            Color::LightGreen
        } else {
            Color::Yellow
        })),
    ])
}

fn editable_table<'a>(
    rows: impl IntoIterator<Item = Row<'a>>,
    header: [&'static str; 2],
) -> Table<'a> {
    Table::new(
        rows,
        [
            Constraint::Length(REGISTER_NAME_WIDTH),
            Constraint::Length(REGISTER_VALUE_WIDTH),
        ],
    )
    .header(Row::new(header).style(Style::default().fg(Color::Yellow)))
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ")
}

fn editable_table_without_header<'a>(rows: impl IntoIterator<Item = Row<'a>>) -> Table<'a> {
    Table::new(
        rows,
        [
            Constraint::Length(REGISTER_NAME_WIDTH),
            Constraint::Length(REGISTER_VALUE_WIDTH),
        ],
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ")
}

fn register_pane_title(debugger: &Debugger, app: &App) -> String {
    let mode = debugger.machine.cpu.privilege_mode().label();
    match app.mode {
        Mode::RegisterSelect => format!(" Registers [{mode}] "),
        Mode::RegisterEdit => {
            format!(" Registers [{mode}; editing] ")
        }
        Mode::Command | Mode::UartInput => format!(" Registers [{mode}] "),
    }
}

fn pseudo_register_pane_title(app: &App) -> &'static str {
    match app.mode {
        Mode::RegisterSelect => " Pseudo-registers ",
        Mode::RegisterEdit if app.selected_register >= FIRST_PSEUDO_REGISTER_INDEX => {
            " Pseudo-registers [editing] "
        }
        _ => " Pseudo-registers ",
    }
}

fn draw_prompt(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let (title, content) = match app.mode {
        Mode::Command => (
            " Command [Enter repeats last; F5 continue, F6 UART, F7 page tables/code, F10 next, F11 step; PgUp/PgDown scroll] ",
            app.command.as_str(),
        ),
        Mode::RegisterSelect => (
            " Right pane navigation ",
            "Up/Down select, Enter edit, u undo, r/s/n/c execute, F7 view, q quit",
        ),
        Mode::RegisterEdit => (" New register value ", app.edit_value.as_str()),
        Mode::UartInput => (
            " UART input [Enter sends newline, Esc cancels] ",
            app.uart_input.as_str(),
        ),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::raw(content),
        ]))
        .block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn selected_value(debugger: &Debugger, index: usize) -> u64 {
    match index {
        PC_INDEX => debugger.machine.cpu.pc,
        MSIP_INDEX => debugger.machine.bus.msip(),
        MTIME_INDEX => debugger.machine.bus.mtime(),
        MTIMECMP_INDEX => debugger.machine.bus.mtimecmp(),
        SATP_INDEX => debugger.machine.cpu.csr(0x180),
        _ => debugger.machine.cpu.register(index),
    }
}

fn register_label(index: usize) -> String {
    match index {
        PC_INDEX => "pc".into(),
        MSIP_INDEX => "msip".into(),
        MTIME_INDEX => "mtime".into(),
        MTIMECMP_INDEX => "mtimecmp".into(),
        SATP_INDEX => "satp".into(),
        _ => format!("x{index:<2} {}", REGISTER_NAMES[index]),
    }
}

fn exit_chord(key: KeyEvent) -> Option<ExitChord> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char('c') => Some(ExitChord::ControlC),
        KeyCode::Char('d') => Some(ExitChord::ControlD),
        _ => None,
    }
}

fn confirm_exit(chord: ExitChord, app: &mut App) {
    let now = Instant::now();
    if app.pending_exit.is_some_and(|(pending, time)| {
        pending == chord && now.duration_since(time) <= EXIT_CONFIRMATION_WINDOW
    }) {
        app.quit = true;
        return;
    }
    app.pending_exit = Some((chord, now));
    let key = match chord {
        ExitChord::ControlC => "Ctrl-C",
        ExitChord::ControlD => "Ctrl-D",
    };
    app.status = format!("press {key} again within one second to quit");
}

fn record_and_set(debugger: &mut Debugger, app: &mut App, index: usize, value: u64) {
    app.edit_history.push(RegisterEdit {
        index,
        previous_value: selected_value(debugger, index),
    });
    set_value(debugger, index, value);
}

fn set_value(debugger: &mut Debugger, index: usize, value: u64) {
    match index {
        PC_INDEX => debugger.machine.cpu.pc = value,
        MSIP_INDEX => debugger.machine.bus.set_msip(value),
        MTIME_INDEX => debugger.machine.bus.set_mtime(value),
        MTIMECMP_INDEX => debugger.machine.bus.set_mtimecmp(value),
        SATP_INDEX => {} // read-only in the pane; never fall through to registers
        _ if index < REGISTER_NAMES.len() => debugger.machine.cpu.set_register(index, value),
        _ => {}
    }
}

fn undo_last_edit(debugger: &mut Debugger, app: &mut App) {
    match app.edit_history.pop() {
        Some(edit) => {
            set_value(debugger, edit.index, edit.previous_value);
            app.status = format!(
                "restored {} to {:#018x}",
                register_label(edit.index),
                edit.previous_value
            );
        }
        None => app.status = "no register edit to undo".into(),
    }
}

fn instruction_name(instruction: u32) -> &'static str {
    match instruction & 0x7f {
        0x03 => "load",
        0x0f => "fence",
        0x13 => "op-imm",
        0x17 => "auipc",
        0x1b => "op-imm-32",
        0x23 => "store",
        0x2f => "amo",
        0x33 => "op",
        0x37 => "lui",
        0x3b => "op-32",
        0x63 => "branch",
        0x67 => "jalr",
        0x6f => "jal",
        0x73 if instruction == 0x0010_0073 => "ebreak",
        0x73 if instruction == 0x0000_0073 => "ecall",
        0x73 if instruction == 0x3020_0073 => "mret",
        0x73 if instruction == 0x1020_0073 => "sret",
        0x73 if instruction == 0x1050_0073 => "wfi",
        0x73 => "system",
        _ => "unknown",
    }
}

fn compressed_instruction_name(instruction: u16) -> &'static str {
    let raw = u32::from(instruction);
    let quadrant = raw & 0b11;
    let funct3 = (raw >> 13) & 0b111;
    match (quadrant, funct3) {
        (0b00, 0b000) => "c.addi4spn",
        (0b00, 0b010) => "c.lw",
        (0b00, 0b011) => "c.ld",
        (0b00, 0b110) => "c.sw",
        (0b00, 0b111) => "c.sd",
        (0b01, 0b000) if raw == 0x0001 => "c.nop",
        (0b01, 0b000) => "c.addi",
        (0b01, 0b001) => "c.addiw",
        (0b01, 0b010) => "c.li",
        (0b01, 0b011) if ((raw >> 7) & 0x1f) == 2 => "c.addi16sp",
        (0b01, 0b011) => "c.lui",
        (0b01, 0b100) => compressed_misc_alu_name(raw),
        (0b01, 0b101) => "c.j",
        (0b01, 0b110) => "c.beqz",
        (0b01, 0b111) => "c.bnez",
        (0b10, 0b000) => "c.slli",
        (0b10, 0b010) => "c.lwsp",
        (0b10, 0b011) => "c.ldsp",
        (0b10, 0b100) => compressed_jalr_mv_name(raw),
        (0b10, 0b110) => "c.swsp",
        (0b10, 0b111) => "c.sdsp",
        _ => "c.unknown",
    }
}

fn compressed_misc_alu_name(raw: u32) -> &'static str {
    match ((raw >> 10) & 0b11, (raw >> 12) & 1, (raw >> 5) & 0b11) {
        (0b00, _, _) => "c.srli",
        (0b01, _, _) => "c.srai",
        (0b10, _, _) => "c.andi",
        (0b11, 0, 0b00) => "c.sub",
        (0b11, 0, 0b01) => "c.xor",
        (0b11, 0, 0b10) => "c.or",
        (0b11, 0, 0b11) => "c.and",
        (0b11, 1, 0b00) => "c.subw",
        (0b11, 1, 0b01) => "c.addw",
        _ => "c.unknown",
    }
}

fn compressed_jalr_mv_name(raw: u32) -> &'static str {
    let rd_rs1 = (raw >> 7) & 0x1f;
    let rs2 = (raw >> 2) & 0x1f;
    match ((raw >> 12) & 1, rd_rs1, rs2) {
        (0, _, 0) => "c.jr",
        (0, _, _) => "c.mv",
        (1, 0, 0) => "c.ebreak",
        (1, _, 0) => "c.jalr",
        (1, _, _) => "c.add",
        _ => "c.unknown",
    }
}

fn code_flow(instruction: u32, address: u64, debugger: &Debugger) -> Option<CodeFlow> {
    if let Some(branch) = branch_info(instruction, address, debugger) {
        return Some(CodeFlow {
            target: branch.target,
            taken: branch.taken,
        });
    }
    if let Some(jump) = decode_jump(instruction, address, debugger) {
        return Some(CodeFlow {
            target: jump.target,
            taken: true,
        });
    }
    decode_trap_flow(instruction, debugger).map(|trap| CodeFlow {
        target: trap.target,
        taken: true,
    })
}

fn decode_trap_flow(instruction: u32, debugger: &Debugger) -> Option<TrapFlowInfo> {
    match instruction {
        INSTRUCTION_ECALL => Some(TrapFlowInfo {
            mnemonic: "ecall",
            target_label: "mtvec",
            target: debugger.machine.cpu.csr(CSR_MTVEC) & !0b11,
        }),
        INSTRUCTION_MRET => Some(TrapFlowInfo {
            mnemonic: "mret",
            target_label: "mepc",
            target: debugger.machine.cpu.csr(CSR_MEPC),
        }),
        INSTRUCTION_SRET => Some(TrapFlowInfo {
            mnemonic: "sret",
            target_label: "sepc",
            target: debugger.machine.cpu.csr(CSR_SEPC),
        }),
        _ => None,
    }
}

fn trap_flow_spans<'a>(trap: &TrapFlowInfo, base: Style) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!("{} -> {} ", trap.mnemonic, trap.target_label), base),
        Span::styled(
            format!("{:#018x}", trap.target),
            base.patch(Style::default().fg(Color::Cyan)),
        ),
    ]
}

fn branch_info(instruction: u32, address: u64, debugger: &Debugger) -> Option<BranchInfo> {
    if instruction & 0x7f != BRANCH_OPCODE {
        return None;
    }
    let funct3 = (instruction >> 12) & 0x7;
    let rs1 = ((instruction >> 15) & 0x1f) as usize;
    let rs2 = ((instruction >> 20) & 0x1f) as usize;
    let lhs = debugger.machine.cpu.register(rs1);
    let rhs = debugger.machine.cpu.register(rs2);
    let (mnemonic, operator, taken) = match funct3 {
        0 => ("beq", "==", lhs == rhs),
        1 => ("bne", "!=", lhs != rhs),
        4 => ("blt", "<s", (lhs as i64) < (rhs as i64)),
        5 => ("bge", ">=s", (lhs as i64) >= (rhs as i64)),
        6 => ("bltu", "<u", lhs < rhs),
        7 => ("bgeu", ">=u", lhs >= rhs),
        _ => return None,
    };
    Some(BranchInfo {
        mnemonic,
        rs1,
        rs2,
        target: address.wrapping_add(b_immediate(instruction) as u64),
        taken,
        operator,
    })
}


fn branch_spans<'a>(branch: &BranchInfo, base: Style, debugger: &Debugger) -> Vec<Span<'a>> {
    let lhs = debugger.machine.cpu.register(branch.rs1);
    let rhs = debugger.machine.cpu.register(branch.rs2);
    let result_color = if branch.taken {
        Color::LightGreen
    } else {
        Color::LightRed
    };
    vec![
        Span::styled(
            format!(
                "{} {},{} [",
                branch.mnemonic, REGISTER_NAMES[branch.rs1], REGISTER_NAMES[branch.rs2]
            ),
            base,
        ),
        Span::styled(
            format!("{lhs:#x}"),
            base.patch(Style::default().fg(Color::Yellow)),
        ),
        Span::styled(format!(" {} ", branch.operator), base),
        Span::styled(
            format!("{rhs:#x}"),
            base.patch(Style::default().fg(Color::Yellow)),
        ),
        Span::styled(
            if branch.taken {
                ": taken] -> "
            } else {
                ": not taken] -> "
            },
            base.patch(Style::default().fg(result_color)),
        ),
        Span::styled(
            format!("{:#018x}", branch.target),
            base.patch(Style::default().fg(Color::Cyan)),
        ),
    ]
}

fn branch_arrow(address: u64, source: u64, target: u64, first: u64, last: u64) -> &'static str {
    if target < first || target > last || target == source {
        return "    ";
    }
    if address == target {
        return "+-> ";
    }
    if address == source {
        return "+-- ";
    }
    if address > source.min(target) && address < source.max(target) {
        return "|   ";
    }
    "    "
}

// Jump decoding helpers




fn decode_upper(instruction: u32, address: u64) -> Option<UpperInfo> {
    let rd = ((instruction >> 7) & 0x1f) as usize;
    let immediate = upper_immediate(instruction);
    match instruction & 0x7f {
        0x17 => Some(UpperInfo {
            mnemonic: "auipc",
            rd,
            immediate,
            result: address.wrapping_add(immediate),
        }),
        0x37 => Some(UpperInfo {
            mnemonic: "lui",
            rd,
            immediate,
            result: immediate,
        }),
        _ => None,
    }
}

fn decode_misc_mem(instruction: u32) -> Option<&'static str> {
    if instruction == 0x0000_100f {
        Some("fence.i")
    } else if instruction & 0x7f == 0x0f && ((instruction >> 12) & 0x7) == 0 {
        Some("fence")
    } else {
        None
    }
}

fn decode_jump(instruction: u32, address: u64, debugger: &Debugger) -> Option<JumpInfo> {
    let opcode = instruction & 0x7f;
    let rd = ((instruction >> 7) & 0x1f) as usize;
    if opcode == 0x6f {
        let target = address.wrapping_add(j_immediate(instruction) as u64);
        Some(JumpInfo {
            mnemonic: "jal",
            rd,
            rs1: 0,
            target,
        })
    } else if opcode == 0x67 && ((instruction >> 12) & 0x7) == 0 {
        let rs1 = ((instruction >> 15) & 0x1f) as usize;
        let imm = i_immediate(instruction);
        let base = debugger.machine.cpu.register(rs1);
        let target = base.wrapping_add(imm as u64) & !1;
        Some(JumpInfo {
            mnemonic: "jalr",
            rd,
            rs1,
            target,
        })
    } else {
        None
    }
}

fn decode_mem(instruction: u32, debugger: &Debugger) -> Option<MemInfo> {
    let opcode = instruction & 0x7f;
    let funct3 = (instruction >> 12) & 0x7;
    if opcode != 0x03 && opcode != 0x23 {
        return None;
    }
    let rs1 = ((instruction >> 15) & 0x1f) as usize;
    let base = debugger.machine.cpu.register(rs1);
    let (register, offset) = if opcode == 0x03 {
        (
            ((instruction >> 7) & 0x1f) as usize,
            i_immediate(instruction),
        )
    } else {
        (
            ((instruction >> 20) & 0x1f) as usize,
            s_immediate(instruction),
        )
    };
    let addr = base.wrapping_add(offset as u64);
    let access = if opcode == 0x03 {
        AddressAccess::Load
    } else {
        AddressAccess::Store
    };
    let physical_address = debugger
        .machine
        .cpu
        .translate_address_for_debug(&debugger.machine.bus, addr, access)
        .ok()
        .and_then(|translation| {
            translation
                .paging_active
                .then_some(translation.physical_address)
        });
    let mnemonic = match (opcode, funct3) {
        (0x03, 0) => "lb",
        (0x03, 1) => "lh",
        (0x03, 2) => "lw",
        (0x03, 3) => "ld",
        (0x03, 4) => "lbu",
        (0x03, 5) => "lhu",
        (0x03, 6) => "lwu",
        (0x23, 0) => "sb",
        (0x23, 1) => "sh",
        (0x23, 2) => "sw",
        (0x23, 3) => "sd",
        _ => return None,
    };
    Some(MemInfo {
        mnemonic,
        register,
        rs1,
        offset,
        address: addr,
        physical_address,
    })
}

fn decode_amo(instruction: u32, debugger: &Debugger) -> Option<AmoInfo> {
    if instruction & 0x7f != 0x2f {
        return None;
    }
    let rd = ((instruction >> 7) & 0x1f) as usize;
    let funct3 = (instruction >> 12) & 0x7;
    let rs1 = ((instruction >> 15) & 0x1f) as usize;
    let rs2 = ((instruction >> 20) & 0x1f) as usize;
    let funct5 = (instruction >> 27) & 0x1f;
    let width = match funct3 {
        2 => AmoWidth::Word,
        3 => AmoWidth::Double,
        _ => return None,
    };
    let suffix = match width {
        AmoWidth::Word => "w",
        AmoWidth::Double => "d",
    };
    let operation = match funct5 {
        0b00010 if rs2 == 0 => "lr",
        0b00011 => "sc",
        0b00001 => "amoswap",
        0b00000 => "amoadd",
        0b00100 => "amoxor",
        0b01100 => "amoand",
        0b01000 => "amoor",
        0b10000 => "amomin",
        0b10100 => "amomax",
        0b11000 => "amominu",
        0b11100 => "amomaxu",
        _ => return None,
    };
    let mnemonic = amo_mnemonic(operation, suffix)?;
    let address = debugger.machine.cpu.register(rs1);
    let access = if operation == "lr" {
        AddressAccess::Load
    } else {
        AddressAccess::Store
    };
    let translation = debugger
        .machine
        .cpu
        .translate_address_for_debug(&debugger.machine.bus, address, access)
        .ok();
    let read_address = translation
        .as_ref()
        .map(|translation| translation.physical_address)
        .unwrap_or(address);
    let display_physical_address = translation.and_then(|translation| {
        translation
            .paging_active
            .then_some(translation.physical_address)
    });
    let old_value = match width {
        AmoWidth::Word => debugger
            .machine
            .bus
            .peek_u32(read_address)
            .ok()
            .map(sign_extend_word),
        AmoWidth::Double => debugger.machine.bus.peek_u64(read_address).ok(),
    };
    let rhs = debugger.machine.cpu.register(rs2);
    let sc_success = (operation == "sc").then(|| debugger.machine.cpu.reservation_matches(address));
    let new_value = match operation {
        "lr" => None,
        "sc" => sc_success.and_then(|success| success.then_some(width_value(width, rhs))),
        _ => old_value.map(|old| amo_new_value(operation, width, old, rhs)),
    };

    Some(AmoInfo {
        mnemonic,
        rd,
        rs1,
        rs2,
        address,
        physical_address: display_physical_address,
        width,
        old_value,
        new_value,
        sc_success,
    })
}

fn amo_mnemonic(operation: &str, suffix: &str) -> Option<&'static str> {
    match (operation, suffix) {
        ("lr", "w") => Some("lr.w"),
        ("lr", "d") => Some("lr.d"),
        ("sc", "w") => Some("sc.w"),
        ("sc", "d") => Some("sc.d"),
        ("amoswap", "w") => Some("amoswap.w"),
        ("amoswap", "d") => Some("amoswap.d"),
        ("amoadd", "w") => Some("amoadd.w"),
        ("amoadd", "d") => Some("amoadd.d"),
        ("amoxor", "w") => Some("amoxor.w"),
        ("amoxor", "d") => Some("amoxor.d"),
        ("amoand", "w") => Some("amoand.w"),
        ("amoand", "d") => Some("amoand.d"),
        ("amoor", "w") => Some("amoor.w"),
        ("amoor", "d") => Some("amoor.d"),
        ("amomin", "w") => Some("amomin.w"),
        ("amomin", "d") => Some("amomin.d"),
        ("amomax", "w") => Some("amomax.w"),
        ("amomax", "d") => Some("amomax.d"),
        ("amominu", "w") => Some("amominu.w"),
        ("amominu", "d") => Some("amominu.d"),
        ("amomaxu", "w") => Some("amomaxu.w"),
        ("amomaxu", "d") => Some("amomaxu.d"),
        _ => None,
    }
}

fn width_value(width: AmoWidth, value: u64) -> u64 {
    match width {
        AmoWidth::Word => sign_extend_word(value as u32),
        AmoWidth::Double => value,
    }
}

fn amo_new_value(operation: &str, width: AmoWidth, old: u64, rhs: u64) -> u64 {
    match width {
        AmoWidth::Word => {
            let lhs = old as u32;
            let rhs = rhs as u32;
            let value = match operation {
                "amoswap" => rhs,
                "amoadd" => lhs.wrapping_add(rhs),
                "amoxor" => lhs ^ rhs,
                "amoand" => lhs & rhs,
                "amoor" => lhs | rhs,
                "amomin" => ((lhs as i32).min(rhs as i32)) as u32,
                "amomax" => ((lhs as i32).max(rhs as i32)) as u32,
                "amominu" => lhs.min(rhs),
                "amomaxu" => lhs.max(rhs),
                _ => lhs,
            };
            sign_extend_word(value)
        }
        AmoWidth::Double => match operation {
            "amoswap" => rhs,
            "amoadd" => old.wrapping_add(rhs),
            "amoxor" => old ^ rhs,
            "amoand" => old & rhs,
            "amoor" => old | rhs,
            "amomin" => ((old as i64).min(rhs as i64)) as u64,
            "amomax" => ((old as i64).max(rhs as i64)) as u64,
            "amominu" => old.min(rhs),
            "amomaxu" => old.max(rhs),
            _ => old,
        },
    }
}

fn amo_spans<'a>(amo: &AmoInfo, base: Style) -> Vec<Span<'a>> {
    let rd_name = REGISTER_NAMES[amo.rd];
    let rs2_name = REGISTER_NAMES[amo.rs2];
    let rs1_name = REGISTER_NAMES[amo.rs1];
    let mut spans = vec![Span::styled(
        if amo.mnemonic.starts_with("lr.") {
            format!("{} {},({}) @ ", amo.mnemonic, rd_name, rs1_name)
        } else {
            format!(
                "{} {},{},({}) @ ",
                amo.mnemonic, rd_name, rs2_name, rs1_name
            )
        },
        base,
    )];
    spans.push(Span::styled(
        format!("{:#018x}", amo.address),
        base.patch(Style::default().fg(Color::Cyan)),
    ));
    if let Some(physical) = amo.physical_address {
        spans.extend([
            Span::styled(" -> ", base),
            Span::styled(
                format!("{physical:#018x}"),
                base.patch(Style::default().fg(Color::LightCyan)),
            ),
        ]);
    }
    if let Some(old_value) = amo.old_value {
        spans.push(Span::styled(" [old ", base));
        spans.push(Span::styled(
            format!("{old_value:#x}"),
            base.patch(Style::default().fg(Color::Yellow)),
        ));
        if let Some(new_value) = amo.new_value {
            spans.push(Span::styled(" -> ", base));
            spans.push(Span::styled(
                format!("{new_value:#x}"),
                base.patch(Style::default().fg(Color::Cyan)),
            ));
        }
        spans.push(Span::styled("]", base));
    }
    if let Some(success) = amo.sc_success {
        spans.push(Span::styled(
            if success { " success" } else { " fail" },
            base.patch(Style::default().fg(if success {
                Color::LightGreen
            } else {
                Color::LightRed
            })),
        ));
    }
    spans
}

fn csr_name(address: u16) -> String {
    match address {
        0x100 => "sstatus".into(),
        0x104 => "sie".into(),
        0x105 => "stvec".into(),
        0x140 => "sscratch".into(),
        0x141 => "sepc".into(),
        0x142 => "scause".into(),
        0x143 => "stval".into(),
        0x144 => "sip".into(),
        0x180 => "satp".into(),
        0x300 => "mstatus".into(),
        0x301 => "misa".into(),
        0x302 => "medeleg".into(),
        0x303 => "mideleg".into(),
        0x304 => "mie".into(),
        0x305 => "mtvec".into(),
        0x340 => "mscratch".into(),
        0x341 => "mepc".into(),
        0x342 => "mcause".into(),
        0x343 => "mtval".into(),
        0x344 => "mip".into(),
        0xc00 => "cycle".into(),
        0xc01 => "time".into(),
        0xc02 => "instret".into(),
        0xf11 => "mvendorid".into(),
        0xf12 => "marchid".into(),
        0xf13 => "mimpid".into(),
        0xf14 => "mhartid".into(),
        _ => format!("{address:#05x}"),
    }
}

fn decode_csr(instruction: u32, debugger: &Debugger) -> Option<CsrInfo> {
    if instruction & 0x7f != 0x73 {
        return None;
    }
    let rd = ((instruction >> 7) & 0x1f) as usize;
    let funct3 = (instruction >> 12) & 0x7;
    let rs1 = ((instruction >> 15) & 0x1f) as usize;
    let csr = ((instruction >> 20) & 0xfff) as u16;
    let old_value = debugger.machine.cpu.csr(csr);
    let register_operand = debugger.machine.cpu.register(rs1);
    let immediate_operand = rs1 as u64;

    let (mnemonic, operand, new_value) = match funct3 {
        1 => ("csrrw", CsrOperand::Register(rs1), Some(register_operand)),
        2 => (
            "csrrs",
            CsrOperand::Register(rs1),
            (rs1 != 0).then_some(old_value | register_operand),
        ),
        3 => (
            "csrrc",
            CsrOperand::Register(rs1),
            (rs1 != 0).then_some(old_value & !register_operand),
        ),
        5 => (
            "csrrwi",
            CsrOperand::Immediate(immediate_operand),
            Some(immediate_operand),
        ),
        6 => (
            "csrrsi",
            CsrOperand::Immediate(immediate_operand),
            (rs1 != 0).then_some(old_value | immediate_operand),
        ),
        7 => (
            "csrrci",
            CsrOperand::Immediate(immediate_operand),
            (rs1 != 0).then_some(old_value & !immediate_operand),
        ),
        _ => return None,
    };

    Some(CsrInfo {
        mnemonic,
        rd,
        csr,
        operand,
        old_value,
        new_value,
    })
}













fn decode_alu(instruction: u32, debugger: &Debugger) -> Option<AluInfo> {
    let opcode = instruction & 0x7f;
    let rd = ((instruction >> 7) & 0x1f) as usize;
    let rs1 = ((instruction >> 15) & 0x1f) as usize;
    let funct3 = (instruction >> 12) & 0x7;
    let lhs = debugger.machine.cpu.register(rs1);

    let (mnemonic, rhs, result, operator) = if opcode == 0x13 {
        let imm = i_immediate(instruction);
        let shift = u64::from((instruction >> 20) & 0x3f);
        match funct3 {
            0 => (
                "addi",
                AluRhs::Immediate(imm),
                lhs.wrapping_add(imm as u64),
                "+",
            ),
            1 if instruction >> 26 == 0 => {
                ("slli", AluRhs::Immediate(shift as i64), lhs << shift, "<<")
            }
            2 => (
                "slti",
                AluRhs::Immediate(imm),
                ((lhs as i64) < imm) as u64,
                "<s",
            ),
            3 => (
                "sltiu",
                AluRhs::Immediate(imm),
                (lhs < imm as u64) as u64,
                "<u",
            ),
            4 => ("xori", AluRhs::Immediate(imm), lhs ^ imm as u64, "^"),
            5 if instruction >> 26 == 0 => {
                ("srli", AluRhs::Immediate(shift as i64), lhs >> shift, ">>")
            }
            5 if instruction >> 26 == 0x10 => (
                "srai",
                AluRhs::Immediate(shift as i64),
                ((lhs as i64) >> shift) as u64,
                ">>s",
            ),
            6 => ("ori", AluRhs::Immediate(imm), lhs | imm as u64, "|"),
            7 => ("andi", AluRhs::Immediate(imm), lhs & imm as u64, "&"),
            _ => return None,
        }
    } else if opcode == 0x1b {
        let imm = i_immediate(instruction);
        let shift = u64::from((instruction >> 20) & 0x1f);
        let word = match funct3 {
            0 => (lhs.wrapping_add(imm as u64) as u32, "addiw", "+w"),
            1 if (instruction >> 25) & 0x7f == 0 => ((lhs as u32) << shift, "slliw", "<<w"),
            5 if (instruction >> 25) & 0x7f == 0 => ((lhs as u32) >> shift, "srliw", ">>w"),
            5 if (instruction >> 25) & 0x7f == 0x20 => {
                (((lhs as u32 as i32) >> shift) as u32, "sraiw", ">>sw")
            }
            _ => return None,
        };
        (
            word.1,
            AluRhs::Immediate(if funct3 == 0 { imm } else { shift as i64 }),
            sign_extend_word(word.0),
            word.2,
        )
    } else if opcode == 0x33 {
        let rs2 = ((instruction >> 20) & 0x1f) as usize;
        let rhs_val = debugger.machine.cpu.register(rs2);
        let funct7 = (instruction >> 25) & 0x7f;
        let shift = rhs_val & 0x3f;
        let (mnemonic, result, operator) = match (funct3, funct7) {
            (0, 0) => ("add", lhs.wrapping_add(rhs_val), "+"),
            (0, 0x20) => ("sub", lhs.wrapping_sub(rhs_val), "-"),
            (0, 1) => ("mul", lhs.wrapping_mul(rhs_val), "*"),
            (1, 0) => ("sll", lhs << shift, "<<"),
            (1, 1) => ("mulh", mulh(lhs, rhs_val), "*h"),
            (2, 0) => ("slt", ((lhs as i64) < (rhs_val as i64)) as u64, "<s"),
            (2, 1) => ("mulhsu", mulhsu(lhs, rhs_val), "*hsu"),
            (3, 0) => ("sltu", (lhs < rhs_val) as u64, "<u"),
            (3, 1) => ("mulhu", mulhu(lhs, rhs_val), "*hu"),
            (4, 0) => ("xor", lhs ^ rhs_val, "^"),
            (4, 1) => ("div", div(lhs, rhs_val), "/s"),
            (5, 0) => ("srl", lhs >> shift, ">>"),
            (5, 0x20) => ("sra", ((lhs as i64) >> shift) as u64, ">>s"),
            (5, 1) => ("divu", divu(lhs, rhs_val), "/u"),
            (6, 0) => ("or", lhs | rhs_val, "|"),
            (6, 1) => ("rem", rem(lhs, rhs_val), "%s"),
            (7, 0) => ("and", lhs & rhs_val, "&"),
            (7, 1) => ("remu", remu(lhs, rhs_val), "%u"),
            _ => return None,
        };
        (mnemonic, AluRhs::Register(rs2), result, operator)
    } else if opcode == 0x3b {
        let rs2 = ((instruction >> 20) & 0x1f) as usize;
        let rhs_val = debugger.machine.cpu.register(rs2);
        let lhs_word = lhs as u32;
        let rhs_word = rhs_val as u32;
        let funct7 = (instruction >> 25) & 0x7f;
        let shift = rhs_val & 0x1f;
        let (mnemonic, word, operator) = match (funct3, funct7) {
            (0, 0) => ("addw", lhs_word.wrapping_add(rhs_word), "+w"),
            (0, 0x20) => ("subw", lhs_word.wrapping_sub(rhs_word), "-w"),
            (0, 1) => ("mulw", lhs_word.wrapping_mul(rhs_word), "*w"),
            (1, 0) => ("sllw", lhs_word << shift, "<<w"),
            (4, 1) => ("divw", divw(lhs_word, rhs_word), "/sw"),
            (5, 0) => ("srlw", lhs_word >> shift, ">>w"),
            (5, 0x20) => ("sraw", ((lhs_word as i32) >> shift) as u32, ">>sw"),
            (5, 1) => ("divuw", divuw(lhs_word, rhs_word), "/uw"),
            (6, 1) => ("remw", remw(lhs_word, rhs_word), "%sw"),
            (7, 1) => ("remuw", remuw(lhs_word, rhs_word), "%uw"),
            _ => return None,
        };
        (
            mnemonic,
            AluRhs::Register(rs2),
            sign_extend_word(word),
            operator,
        )
    } else {
        return None;
    };

    Some(AluInfo {
        mnemonic,
        rd,
        rs1,
        rhs,
        result,
        operator,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i_type(immediate: u32, rs1: u32, funct3: u32, rd: u32, opcode: u32) -> u32 {
        (immediate << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
    }

    fn r_type(funct7: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | 0x33
    }

    fn s_type(immediate: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
        ((immediate >> 5) << 25)
            | (rs2 << 20)
            | (rs1 << 15)
            | (funct3 << 12)
            | ((immediate & 0x1f) << 7)
            | 0x23
    }

    fn csr_type(csr: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (csr << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | 0x73
    }

    fn amo_type(funct5: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (funct5 << 27) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | 0x2f
    }

    fn debugger() -> Debugger {
        Debugger::new(&0x0010_0073_u32.to_le_bytes(), Machine::LOAD_ADDRESS, 4096).unwrap()
    }

    #[test]
    fn exit_chords_require_two_matching_presses() {
        let mut app = App::new();
        confirm_exit(ExitChord::ControlC, &mut app);
        assert!(!app.quit);
        confirm_exit(ExitChord::ControlD, &mut app);
        assert!(!app.quit);
        confirm_exit(ExitChord::ControlD, &mut app);
        assert!(app.quit);
    }

    #[test]
    fn undo_restores_register_and_pc_edits() {
        let mut debugger = debugger();
        let mut app = App::new();

        record_and_set(&mut debugger, &mut app, 10, 0x55);
        record_and_set(&mut debugger, &mut app, PC_INDEX, 0x8000_1000);
        undo_last_edit(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.pc, Machine::LOAD_ADDRESS);
        undo_last_edit(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.register(10), 0);
    }

    #[test]
    fn timer_pseudo_registers_are_editable() {
        let mut debugger = debugger();
        let mut app = App::new();
        record_and_set(&mut debugger, &mut app, MSIP_INDEX, 1);
        record_and_set(&mut debugger, &mut app, MTIME_INDEX, 12);
        record_and_set(&mut debugger, &mut app, MTIMECMP_INDEX, 34);

        assert_eq!(debugger.machine.bus.msip(), 1);
        assert_eq!(debugger.machine.bus.mtime(), 12);
        assert_eq!(debugger.machine.bus.mtimecmp(), 34);
        undo_last_edit(&mut debugger, &mut app);
        assert_eq!(debugger.machine.bus.mtimecmp(), u64::MAX);
    }

    #[test]
    fn live_edit_value_is_parsed_before_commit() {
        let mut app = App::new();
        app.edit_value = "0x1234".into();
        assert_eq!(rave::debugger_parse_number(&app.edit_value), Ok(0x1234));
    }

    #[test]
    fn empty_enter_repeats_last_command() {
        let image: Vec<u8> = [0x0010_8093_u32, 0x0010_8093, 0x0010_0073]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let mut debugger =
            Debugger::new(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE).unwrap();
        let mut app = App::new();
        app.command = "step".into();
        submit_command(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.pc, Machine::LOAD_ADDRESS + 4);

        submit_command(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.pc, Machine::LOAD_ADDRESS + 8);
        assert_eq!(debugger.machine.cpu.register(1), 2);
        assert_eq!(app.last_command.as_deref(), Some("step"));
    }

    #[test]
    fn empty_enter_without_history_steps() {
        let image: Vec<u8> = [0x0010_8093_u32, 0x0010_0073]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let mut debugger =
            Debugger::new(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE).unwrap();
        let mut app = App::new();
        submit_command(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.pc, Machine::LOAD_ADDRESS + 4);
        assert_eq!(debugger.machine.cpu.register(1), 1);
        assert_eq!(app.last_command.as_deref(), Some("step"));
    }

    #[test]
    fn compressed_instruction_names_show_original_encoding() {
        assert_eq!(compressed_instruction_name(0x4085), "c.li");
        assert_eq!(compressed_instruction_name(0x9002), "c.ebreak");
        assert_eq!(compressed_instruction_name(0xc011), "c.beqz");
        assert_eq!(compressed_instruction_name(0x6105), "c.addi16sp");
        assert_eq!(compressed_instruction_name(0x9005), "c.srli");
        assert_eq!(compressed_instruction_name(0x8405), "c.srai");
        assert_eq!(compressed_instruction_name(0x987d), "c.andi");
    }

    #[test]
    fn decodes_and_evaluates_branch_conditions() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(1, 7);
        debugger.machine.cpu.set_register(2, 7);
        let branch = branch_info(0x0020_8463, Machine::LOAD_ADDRESS, &debugger).unwrap();
        assert_eq!(branch.mnemonic, "beq");
        assert!(branch.taken);
        assert_eq!(branch.target, Machine::LOAD_ADDRESS + 8);
    }

    #[test]
    fn trap_flow_points_ecall_to_mtvec_and_mret_to_mepc() {
        let mut debugger = debugger();
        debugger
            .machine
            .cpu
            .set_register(1, Machine::LOAD_ADDRESS + 0x40);
        debugger
            .machine
            .bus
            .write_u32(
                Machine::LOAD_ADDRESS,
                csr_type(u32::from(CSR_MTVEC), 1, 1, 0),
            )
            .unwrap();
        debugger.machine.step().unwrap();
        debugger
            .machine
            .cpu
            .set_register(1, Machine::LOAD_ADDRESS + 0x80);
        debugger
            .machine
            .bus
            .write_u32(
                Machine::LOAD_ADDRESS + 4,
                csr_type(u32::from(CSR_MEPC), 1, 1, 0),
            )
            .unwrap();
        debugger.machine.step().unwrap();

        let ecall = decode_trap_flow(0x0000_0073, &debugger).unwrap();
        assert_eq!(ecall.mnemonic, "ecall");
        assert_eq!(ecall.target_label, "mtvec");
        assert_eq!(ecall.target, Machine::LOAD_ADDRESS + 0x40);

        let mret = decode_trap_flow(0x3020_0073, &debugger).unwrap();
        assert_eq!(mret.mnemonic, "mret");
        assert_eq!(mret.target_label, "mepc");
        assert_eq!(mret.target, Machine::LOAD_ADDRESS + 0x80);

        assert_eq!(
            code_flow(0x3020_0073, Machine::LOAD_ADDRESS + 8, &debugger),
            Some(CodeFlow {
                target: Machine::LOAD_ADDRESS + 0x80,
                taken: true,
            })
        );
    }

    #[test]
    fn branch_arrow_connects_source_to_visible_target() {
        let source = Machine::LOAD_ADDRESS + 4;
        let target = Machine::LOAD_ADDRESS + 12;
        assert_eq!(
            branch_arrow(source, source, target, Machine::LOAD_ADDRESS, target),
            "+-- "
        );
        assert_eq!(
            branch_arrow(source + 4, source, target, Machine::LOAD_ADDRESS, target),
            "|   "
        );
        assert_eq!(
            branch_arrow(target, source, target, Machine::LOAD_ADDRESS, target),
            "+-> "
        );
    }

    #[test]
    fn register_pane_title_includes_privilege_mode() {
        let debugger = debugger();
        let app = App::new();
        assert_eq!(register_pane_title(&debugger, &app), " Registers [M] ");
    }

    #[test]
    fn split_right_column_renders_at_common_terminal_sizes() {
        use ratatui::backend::TestBackend;

        for (width, height) in [(80, 24), (100, 30), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            let debugger = debugger();
            let mut app = App::new();
            terminal
                .draw(|frame| draw(frame, &debugger, &mut app))
                .unwrap();

            app.mode = Mode::RegisterSelect;
            app.selected_register = MTIMECMP_INDEX;
            terminal
                .draw(|frame| draw(frame, &debugger, &mut app))
                .unwrap();

            app.mode = Mode::RegisterEdit;
            app.edit_value = "0x10".into();
            terminal
                .draw(|frame| draw(frame, &debugger, &mut app))
                .unwrap();
        }
    }

    #[test]
    fn register_pane_is_only_as_wide_as_its_columns() {
        assert_eq!(REGISTER_PANE_WIDTH, 32);
        assert_eq!(REGISTER_VALUE_WIDTH, "0xffffffffffffffff".len() as u16);
        assert!(register_label(27).len() <= REGISTER_NAME_WIDTH as usize);
    }

    #[test]
    fn code_view_uses_all_rows_inside_its_border() {
        assert_eq!(visible_code_rows(Rect::new(0, 0, 80, 12)), 10);
        assert_eq!(visible_code_rows(Rect::new(0, 0, 80, 40)), 38);
        assert_eq!(visible_code_rows(Rect::new(0, 0, 80, 1)), 1);
    }

    #[test]
    fn uart_input_mode_queues_line_and_resumes_execution() {
        let image: Vec<u8> = [
            0x0050_c283_u32,
            0x0012_f293,
            0xfe02_8ce3,
            0x0000_c503,
            0x0010_0073,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
        let mut debugger = Debugger::new(&image, Machine::LOAD_ADDRESS, 4096).unwrap();
        debugger.machine.cpu.set_register(1, 0x1000_0000);
        let mut app = App::new();

        execute_command("continue", &mut debugger, &mut app);
        assert_eq!(app.mode, Mode::UartInput);

        app.uart_input = "A".into();
        submit_uart_input(&mut debugger, &mut app);
        assert_eq!(debugger.machine.cpu.register(10), u64::from(b'A'));
        assert!(app.status.contains("guest halted"));
    }

    #[test]
    fn uart_view_uses_rows_inside_its_border() {
        assert_eq!(visible_uart_rows(Rect::new(0, 0, 80, 5)), 3);
        assert_eq!(visible_uart_rows(Rect::new(0, 0, 80, 1)), 1);
    }

    #[test]
    fn uart_output_view_tails_and_escapes_bytes() {
        assert_eq!(
            uart_output_text_lines(b"one\ntwo\nthree", 2),
            vec!["two".to_string(), "three".to_string()]
        );
        assert_eq!(
            uart_output_text_lines(b"A\x00\xffZ", 1),
            vec!["A\\x00\\xffZ".to_string()]
        );
    }

    #[test]
    fn store_decoder_uses_rs2_and_s_immediate() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(2, 0x8000_1000);
        let instruction = s_type(0xff8, 10, 2, 3); // sd a0, -8(sp)
        let decoded = decode_mem(instruction, &debugger).unwrap();
        assert_eq!(decoded.mnemonic, "sd");
        assert_eq!(decoded.register, 10);
        assert_eq!(decoded.offset, -8);
        assert_eq!(decoded.address, 0x8000_0ff8);
    }

    #[test]
    fn amo_decoder_previews_atomic_memory_operations() {
        let mut debugger = debugger();
        debugger
            .machine
            .cpu
            .set_register(1, Machine::LOAD_ADDRESS + 64);
        debugger.machine.cpu.set_register(2, 5);
        debugger
            .machine
            .bus
            .write_u64(Machine::LOAD_ADDRESS + 64, 10)
            .unwrap();
        let amoadd = amo_type(0b00000, 2, 1, 3, 5);
        let decoded = decode_amo(amoadd, &debugger).unwrap();

        assert_eq!(decoded.mnemonic, "amoadd.d");
        assert_eq!(decoded.rd, 5);
        assert_eq!(decoded.rs2, 2);
        assert_eq!(decoded.address, Machine::LOAD_ADDRESS + 64);
        assert_eq!(decoded.old_value, Some(10));
        assert_eq!(decoded.new_value, Some(15));
    }

    #[test]
    fn amo_decoder_previews_lr_sc_reservation_status() {
        let mut debugger = debugger();
        debugger
            .machine
            .cpu
            .set_register(1, Machine::LOAD_ADDRESS + 64);
        debugger.machine.cpu.set_register(2, 0x99);
        debugger
            .machine
            .bus
            .write_u32(Machine::LOAD_ADDRESS + 64, 0x8000_0000)
            .unwrap();

        let lr = amo_type(0b00010, 0, 1, 2, 5);
        let sc = amo_type(0b00011, 2, 1, 2, 6);
        debugger
            .machine
            .bus
            .write_u32(Machine::LOAD_ADDRESS, lr)
            .unwrap();
        assert_eq!(decode_amo(lr, &debugger).unwrap().mnemonic, "lr.w");
        assert_eq!(decode_amo(sc, &debugger).unwrap().sc_success, Some(false));

        debugger.machine.step().unwrap();
        let decoded = decode_amo(sc, &debugger).unwrap();
        assert_eq!(decoded.mnemonic, "sc.w");
        assert_eq!(decoded.sc_success, Some(true));
        assert_eq!(decoded.old_value, Some(0xffff_ffff_8000_0000));
        assert_eq!(decoded.new_value, Some(0x99));
    }

    #[test]
    fn alu_decoder_distinguishes_similar_funct3_encodings() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(1, 0x8000_0000_0000_0000);
        debugger.machine.cpu.set_register(2, 65);

        let ori = decode_alu(i_type(7, 1, 6, 3, 0x13), &debugger).unwrap();
        assert_eq!(ori.mnemonic, "ori");

        let srai = decode_alu(i_type(0x401, 1, 5, 3, 0x13), &debugger).unwrap();
        assert_eq!(srai.mnemonic, "srai");
        assert_eq!(srai.result, 0xc000_0000_0000_0000);

        let sub = decode_alu(r_type(0x20, 2, 1, 0, 3), &debugger).unwrap();
        assert_eq!(sub.mnemonic, "sub");

        let sra = decode_alu(r_type(0x20, 2, 1, 5, 3), &debugger).unwrap();
        assert_eq!(sra.mnemonic, "sra");
        assert_eq!(sra.result, 0xc000_0000_0000_0000);
    }

    #[test]
    fn csr_decoder_previews_register_and_immediate_forms() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(5, 0x1200);
        debugger.machine.cpu.set_register(6, 0x34);

        let write = decode_csr(csr_type(0x340, 5, 1, 0), &debugger).unwrap();
        assert_eq!(write.mnemonic, "csrrw");
        assert_eq!(write.csr, 0x340);
        assert_eq!(write.operand, CsrOperand::Register(5));
        assert_eq!(write.old_value, 0);
        assert_eq!(write.new_value, Some(0x1200));

        let read = decode_csr(csr_type(0x301, 0, 6, 10), &debugger).unwrap();
        assert_eq!(read.mnemonic, "csrrsi");
        assert_eq!(read.csr, 0x301);
        assert_eq!(read.operand, CsrOperand::Immediate(0));
        assert_ne!(read.old_value, 0);
        assert_eq!(read.new_value, None);
    }

    #[test]
    fn csr_name_prefers_known_machine_names() {
        assert_eq!(csr_name(0x340), "mscratch");
        assert_eq!(csr_name(0x777), "0x777");
    }

    #[test]
    fn upper_immediate_decoder_previews_lui_and_auipc() {
        let lui = decode_upper(0x1234_52b7, Machine::LOAD_ADDRESS).unwrap();
        assert_eq!(lui.mnemonic, "lui");
        assert_eq!(lui.rd, 5);
        assert_eq!(lui.immediate, 0x1234_5000);
        assert_eq!(lui.result, 0x1234_5000);

        let auipc = decode_upper(0xffff_f317, Machine::LOAD_ADDRESS + 4).unwrap();
        assert_eq!(auipc.mnemonic, "auipc");
        assert_eq!(auipc.rd, 6);
        assert_eq!(auipc.immediate, 0xffff_ffff_ffff_f000);
        assert_eq!(auipc.result, Machine::LOAD_ADDRESS - 0xffc);
    }

    #[test]
    fn misc_mem_decoder_names_fences() {
        assert_eq!(decode_misc_mem(0x0000_000f), Some("fence"));
        assert_eq!(decode_misc_mem(0x0000_100f), Some("fence.i"));
        assert_eq!(decode_misc_mem(0x0000_200f), None);
    }

    #[test]
    fn alu_decoder_previews_word_immediate_operations() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(1, 0xffff_ffff_8000_0001);

        let addiw = decode_alu(i_type(0xfff, 1, 0, 5, 0x1b), &debugger).unwrap();
        assert_eq!(addiw.mnemonic, "addiw");
        assert_eq!(addiw.rd, 5);
        assert_eq!(addiw.rs1, 1);
        assert_eq!(addiw.rhs, AluRhs::Immediate(-1));
        assert_eq!(addiw.result, 0xffff_ffff_8000_0000);

        let slliw = decode_alu(i_type(1, 1, 1, 6, 0x1b), &debugger).unwrap();
        assert_eq!(slliw.mnemonic, "slliw");
        assert_eq!(slliw.result, 2);

        let srliw = decode_alu(i_type(1, 1, 5, 7, 0x1b), &debugger).unwrap();
        assert_eq!(srliw.mnemonic, "srliw");
        assert_eq!(srliw.result, 0x4000_0000);

        let sraiw = decode_alu(i_type(0x401, 1, 5, 8, 0x1b), &debugger).unwrap();
        assert_eq!(sraiw.mnemonic, "sraiw");
        assert_eq!(sraiw.result, 0xffff_ffff_c000_0000);
    }

    #[test]
    fn alu_decoder_previews_rv64m_operations() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(10, 6);
        debugger.machine.cpu.set_register(13, 37);
        debugger.machine.cpu.set_register(16, 222);

        let mul = decode_alu(0x02a6_8833, &debugger).unwrap();
        assert_eq!(mul.mnemonic, "mul");
        assert_eq!(mul.rd, 16);
        assert_eq!(mul.rs1, 13);
        assert_eq!(mul.rhs, AluRhs::Register(10));
        assert_eq!(mul.result, 222);

        let divu = decode_alu(0x02d8_5633, &debugger).unwrap();
        assert_eq!(divu.mnemonic, "divu");
        assert_eq!(divu.rd, 12);
        assert_eq!(divu.rs1, 16);
        assert_eq!(divu.rhs, AluRhs::Register(13));
        assert_eq!(divu.result, 6);
    }

    #[test]
    fn alu_decoder_previews_rv64m_word_operations() {
        let mut debugger = debugger();
        debugger.machine.cpu.set_register(15, 3);
        debugger.machine.cpu.set_register(17, 300_000);

        let divuw = decode_alu(0x02f8_d73b, &debugger).unwrap();
        assert_eq!(divuw.mnemonic, "divuw");
        assert_eq!(divuw.rd, 14);
        assert_eq!(divuw.rs1, 17);
        assert_eq!(divuw.rhs, AluRhs::Register(15));
        assert_eq!(divuw.result, 100_000);

        let subw = decode_alu(0x40f8_87bb, &debugger).unwrap();
        assert_eq!(subw.mnemonic, "subw");
        assert_eq!(subw.rd, 15);
        assert_eq!(subw.rs1, 17);
        assert_eq!(subw.rhs, AluRhs::Register(15));
        assert_eq!(subw.result, 299_997);
    }

    #[test]
    fn trap_return_instruction_is_named_in_code_view() {
        assert_eq!(instruction_name(0x3020_0073), "mret");
        assert_eq!(instruction_name(0x0000_0073), "ecall");
    }

    #[test]
    fn decoded_instruction_column_has_fixed_width() {
        assert_eq!(format!("{:<INSTRUCTION_CLASS_WIDTH$}", "op"), "op        ");
        assert_eq!(
            format!("{:<INSTRUCTION_CLASS_WIDTH$}", "op-imm-32").len(),
            10
        );
    }
}
