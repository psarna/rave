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
    "start | step(s) | next(n) | break(b) ADDR | continue(c) | set REG VALUE | undo(u) | quit(q)";
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct BranchInfo {
    mnemonic: &'static str,
    rs1: usize,
    rs2: usize,
    target: u64,
    taken: bool,
    operator: &'static str,
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
}

struct App {
    mode: Mode,
    command: String,
    last_command: Option<String>,
    edit_value: String,
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
        _ => None,
    };
    match debugger.execute(command, Machine::INSTRUCTION_LIMIT) {
        Ok(Some(reason)) => app.status = format_stop(reason),
        Ok(None) => app.status = description.unwrap_or_else(|| "ok".into()),
        Err(error) => app.status = error.to_string(),
    }
}

fn submit_command(debugger: &mut Debugger, app: &mut App) {
    let typed = std::mem::take(&mut app.command);
    let input = if typed.trim().is_empty() {
        match app.last_command.clone() {
            Some(command) => command,
            None => {
                app.status = "no previous command".into();
                return;
            }
        }
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
        StopReason::Halted(reason) => format!("guest halted: {reason:?}"),
    }
}

fn draw(frame: &mut Frame<'_>, debugger: &Debugger, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),
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
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().title(" Status ").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        outer[1],
    );
    draw_prompt(frame, outer[2], app);
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
                        instruction_name(instruction),
                        base.patch(Style::default().fg(Color::Green)),
                    ));
                    if let Some(branch) = branch_info(instruction, address, debugger) {
                        spans.extend(branch_spans(&branch, base, debugger));
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
        Mode::Command => " Registers [Tab: select] ",
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
                " {} {},{} [",
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn empty_enter_without_history_is_a_noop() {
        let mut debugger = debugger();
        let mut app = App::new();
        submit_command(&mut debugger, &mut app);
        assert_eq!(app.status, "no previous command");
        assert_eq!(debugger.machine.cpu.pc, Machine::LOAD_ADDRESS);
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
}
