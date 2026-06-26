use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
use rave::{Command, Debugger, Machine, StopReason, REGISTER_NAMES};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

const HELP: &str =
    "start | step(s) | next(n) | break(b) ADDR | continue(c) | uart TEXT | set REG VALUE | undo(u) | quit(q)";
const EXIT_CONFIRMATION_WINDOW: Duration = Duration::from_secs(1);
const PC_INDEX: usize = 32;
const INSTRUCTION_SIZE: u64 = 4;
const PANEL_BORDER_HEIGHT: u16 = 2;
const BRANCH_OPCODE: u32 = 0x63;
const REGISTER_NAME_WIDTH: u16 = 8;
const REGISTER_VALUE_WIDTH: u16 = 18;
const REGISTER_TABLE_DECORATION_WIDTH: u16 = 6;
const REGISTER_PANE_WIDTH: u16 =
    REGISTER_NAME_WIDTH + REGISTER_VALUE_WIDTH + REGISTER_TABLE_DECORATION_WIDTH;
const INSTRUCTION_CLASS_WIDTH: usize = 10;

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
struct MemInfo {
    mnemonic: &'static str,
    register: usize,
    rs1: usize,
    offset: i64,
    address: u64,
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
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(key, &mut debugger, &mut app);
                }
            }
        }
    }
    Ok(())
}

struct ScreenGuard;

impl ScreenGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
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
    app.pending_exit = None;

    match app.mode {
        Mode::Command => handle_command_key(key, debugger, app),
        Mode::RegisterSelect => handle_register_key(key, debugger, app),
        Mode::RegisterEdit => handle_edit_key(key, debugger, app),
        Mode::UartInput => handle_uart_input_key(key, debugger, app),
    }
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
            app.selected_register = (app.selected_register + 1).min(PC_INDEX)
        }
        KeyCode::Home => app.selected_register = 0,
        KeyCode::End => app.selected_register = PC_INDEX,
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
    match debugger.execute(command, Machine::INSTRUCTION_LIMIT) {
        Ok(Some(reason)) => {
            app.status = format_stop(reason);
            if reason == StopReason::UartInput {
                enter_uart_input(app);
            }
        }
        Ok(None) => app.status = description.unwrap_or_else(|| "ok".into()),
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
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(REGISTER_PANE_WIDTH)])
        .split(outer[0]);
    draw_code(frame, body[0], debugger);
    draw_registers(frame, body[1], debugger, app);
    draw_uart(frame, outer[1], debugger);
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().title(" Status ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        outer[2],
    );
    draw_prompt(frame, outer[3], app);
}

fn draw_code(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger) {
    let pc = debugger.machine.cpu.pc;
    let code_rows = visible_code_rows(area);
    let rows_before_pc = code_rows / 2;
    let first = pc.saturating_sub(INSTRUCTION_SIZE * rows_before_pc);
    let last = first.wrapping_add(code_rows.saturating_sub(1) * INSTRUCTION_SIZE);
    let current_branch = debugger
        .machine
        .bus
        .read_u32(pc)
        .ok()
        .and_then(|instruction| branch_info(instruction, pc, debugger));
    let lines: Vec<Line<'_>> = (0..code_rows)
        .map(|index| {
            let address = first.wrapping_add(index * INSTRUCTION_SIZE);
            let current = address == pc;
            let breakpoint = debugger.breakpoints().contains(&address);
            let arrow = current_branch
                .as_ref()
                .map(|branch| branch_arrow(address, pc, branch.target, first, last))
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
                        if current_branch.as_ref().is_some_and(|branch| branch.taken) {
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
                Span::styled("  ", base),
            ];
            match debugger.machine.bus.read_u32(address) {
                Ok(instruction) => {
                    spans.push(Span::styled(
                        format!("{instruction:08x}"),
                        base.patch(Style::default().fg(Color::Magenta)),
                    ));
                    spans.push(Span::styled("  ", base));
                    spans.push(Span::styled(
                        format!(
                            "{:<width$}",
                            instruction_name(instruction),
                            width = INSTRUCTION_CLASS_WIDTH
                        ),
                        base.patch(Style::default().fg(Color::Green)),
                    ));
                    if let Some(branch) = branch_info(instruction, address, debugger) {
                        spans.extend(branch_spans(&branch, base, debugger));
                    } else {
                        // Decode jumps
                        if let Some(jump) = decode_jump(instruction, address, debugger) {
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
                        }
                        // Decode memory ops
                        else if let Some(mem) = decode_mem(instruction, debugger) {
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
                        }
                        // Decode ALU ops
                        else if let Some(alu) = decode_alu(instruction, debugger) {
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

fn draw_registers(frame: &mut Frame<'_>, area: Rect, debugger: &Debugger, app: &App) {
    let rows = (0..=PC_INDEX).map(|index| {
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
    });
    let mut state = TableState::default().with_selected(app.selected_register);
    let title = match app.mode {
        Mode::RegisterSelect => " Registers [Tab: prompt, Enter: edit] ",
        Mode::RegisterEdit => " Registers [live edit; Enter commit, Esc cancel] ",
        Mode::Command | Mode::UartInput => " Registers [Tab: select] ",
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(REGISTER_NAME_WIDTH),
            Constraint::Length(REGISTER_VALUE_WIDTH),
        ],
    )
    .header(Row::new(["register", "value"]).style(Style::default().fg(Color::Yellow)))
    .block(Block::default().title(title).borders(Borders::ALL))
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ");
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_prompt(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let (title, content) = match app.mode {
        Mode::Command => (
            " Command [Enter repeats last; F5 continue, F10 next, F11 step] ",
            app.command.as_str(),
        ),
        Mode::RegisterSelect => (
            " Register navigation ",
            "↑/↓ select, Enter edit, u undo, r/s/n/c execute, q quit",
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
    if index == PC_INDEX {
        debugger.machine.cpu.pc
    } else {
        debugger.machine.cpu.register(index)
    }
}

fn register_label(index: usize) -> String {
    if index == PC_INDEX {
        "pc".into()
    } else {
        format!("x{index:<2} {}", REGISTER_NAMES[index])
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
    if index == PC_INDEX {
        debugger.machine.cpu.pc = value;
    } else {
        debugger.machine.cpu.set_register(index, value);
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
        0x33 => "op",
        0x37 => "lui",
        0x3b => "op-32",
        0x63 => "branch",
        0x67 => "jalr",
        0x6f => "jal",
        0x73 if instruction == 0x0010_0073 => "ebreak",
        0x73 if instruction == 0x0000_0073 => "ecall",
        0x73 => "system",
        _ => "unknown",
    }
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
        target: address.wrapping_add(branch_immediate(instruction) as u64),
        taken,
        operator,
    })
}

fn branch_immediate(instruction: u32) -> i64 {
    let immediate = ((instruction >> 31) << 12)
        | (((instruction >> 7) & 1) << 11)
        | (((instruction >> 25) & 0x3f) << 5)
        | (((instruction >> 8) & 0xf) << 1);
    ((immediate << 19) as i32 >> 19) as i64
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
fn jal_immediate(instruction: u32) -> i64 {
    let value = ((instruction >> 31) << 20)
        | (((instruction >> 12) & 0xff) << 12)
        | (((instruction >> 20) & 1) << 11)
        | (((instruction >> 21) & 0x3ff) << 1);
    ((value << 11) as i32 >> 11) as i64
}

fn i_immediate(instruction: u32) -> i64 {
    let imm = (instruction >> 20) & 0xfff;
    ((imm << 20) as i32 >> 20) as i64
}

fn s_immediate(instruction: u32) -> i64 {
    let immediate = ((instruction >> 25) << 5) | ((instruction >> 7) & 0x1f);
    ((immediate << 20) as i32 >> 20) as i64
}

fn decode_jump(instruction: u32, address: u64, debugger: &Debugger) -> Option<JumpInfo> {
    let opcode = instruction & 0x7f;
    let rd = ((instruction >> 7) & 0x1f) as usize;
    if opcode == 0x6f {
        let target = address.wrapping_add(jal_immediate(instruction) as u64);
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
    })
}

fn csr_name(address: u16) -> String {
    match address {
        0x300 => "mstatus".into(),
        0x301 => "misa".into(),
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

fn sign_extend_word(value: u32) -> u64 {
    value as i32 as i64 as u64
}

fn mulh(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as i64 as i128)) >> 64) as u64
}

fn mulhsu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as u128 as i128)) >> 64) as u64
}

fn mulhu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as u128) * (rhs as u128)) >> 64) as u64
}

fn div(lhs: u64, rhs: u64) -> u64 {
    let dividend = lhs as i64;
    let divisor = rhs as i64;
    if divisor == 0 {
        u64::MAX
    } else if dividend == i64::MIN && divisor == -1 {
        lhs
    } else {
        dividend.wrapping_div(divisor) as u64
    }
}

fn divu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        u64::MAX
    } else {
        lhs / rhs
    }
}

fn rem(lhs: u64, rhs: u64) -> u64 {
    let dividend = lhs as i64;
    let divisor = rhs as i64;
    if divisor == 0 {
        lhs
    } else if dividend == i64::MIN && divisor == -1 {
        0
    } else {
        dividend.wrapping_rem(divisor) as u64
    }
}

fn remu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

fn divw(lhs: u32, rhs: u32) -> u32 {
    let dividend = lhs as i32;
    let divisor = rhs as i32;
    if divisor == 0 {
        u32::MAX
    } else if dividend == i32::MIN && divisor == -1 {
        lhs
    } else {
        dividend.wrapping_div(divisor) as u32
    }
}

fn divuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        u32::MAX
    } else {
        lhs / rhs
    }
}

fn remw(lhs: u32, rhs: u32) -> u32 {
    let dividend = lhs as i32;
    let divisor = rhs as i32;
    if divisor == 0 {
        lhs
    } else if dividend == i32::MIN && divisor == -1 {
        0
    } else {
        dividend.wrapping_rem(divisor) as u32
    }
}

fn remuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
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
    fn decoded_instruction_column_has_fixed_width() {
        assert_eq!(format!("{:<INSTRUCTION_CLASS_WIDTH$}", "op"), "op        ");
        assert_eq!(
            format!("{:<INSTRUCTION_CLASS_WIDTH$}", "op-imm-32").len(),
            10
        );
    }
}
