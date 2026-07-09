use crate::cpu::{decode_compressed_instruction, encoded_instruction_size};
use crate::{AddressAccess, HaltReason, Machine, MachineError};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

pub const PC_REGISTER_INDEX: usize = 32;
pub const MSIP_REGISTER_INDEX: usize = 33;
pub const MTIME_REGISTER_INDEX: usize = 34;
pub const MTIMECMP_REGISTER_INDEX: usize = 35;
pub const UART_IER_REGISTER_INDEX: usize = 36;
pub const PLIC_UART_PRIORITY_REGISTER_INDEX: usize = 37;
pub const PLIC_PENDING_REGISTER_INDEX: usize = 38;
pub const PLIC_MACHINE_ENABLE_REGISTER_INDEX: usize = 39;
pub const PLIC_SUPERVISOR_ENABLE_REGISTER_INDEX: usize = 40;
pub const PLIC_MACHINE_THRESHOLD_REGISTER_INDEX: usize = 41;
pub const PLIC_SUPERVISOR_THRESHOLD_REGISTER_INDEX: usize = 42;
pub const PLIC_MACHINE_CLAIM_REGISTER_INDEX: usize = 43;
pub const PLIC_SUPERVISOR_CLAIM_REGISTER_INDEX: usize = 44;
pub const SATP_REGISTER_INDEX: usize = 45;

pub const REGISTER_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Start,
    Step,
    Next,
    Break(u64),
    Continue,
    UartInput(String),
    SetRegister { index: usize, value: u64 },
    Undo,
    Help,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError(pub String);

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CommandError {}

impl FromStr for Command {
    type Err = CommandError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input.trim();
        if let Some(rest) = trimmed
            .strip_prefix("uart ")
            .or_else(|| trimmed.strip_prefix("input "))
        {
            return Ok(Self::UartInput(rest.to_string()));
        }
        if trimmed == "uart" || trimmed == "input" {
            return Err(CommandError("missing UART input".into()));
        }
        let mut words = input.split_whitespace();
        let name = words
            .next()
            .ok_or_else(|| CommandError("empty command".into()))?;
        match name {
            "start" | "run" | "r" => no_args(words, Self::Start),
            "step" | "stepi" | "s" | "si" => no_args(words, Self::Step),
            "next" | "nexti" | "n" | "ni" => no_args(words, Self::Next),
            "continue" | "cont" | "c" => no_args(words, Self::Continue),
            "help" | "h" | "?" => no_args(words, Self::Help),
            "undo" | "u" => no_args(words, Self::Undo),
            "exit" | "quit" | "q" => no_args(words, Self::Quit),
            "break" | "b" => {
                let address = parse_number(required(&mut words, "address")?)?;
                no_args(words, Self::Break(address))
            }
            "set" => {
                let index = parse_register(required(&mut words, "register")?)?;
                let value = parse_number(required(&mut words, "value")?)?;
                no_args(words, Self::SetRegister { index, value })
            }
            _ => Err(CommandError(format!("unknown command: {name}"))),
        }
    }
}

fn required<'a>(
    words: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<&'a str, CommandError> {
    words
        .next()
        .ok_or_else(|| CommandError(format!("missing {name}")))
}

fn no_args<'a>(
    mut words: impl Iterator<Item = &'a str>,
    command: Command,
) -> Result<Command, CommandError> {
    if let Some(extra) = words.next() {
        Err(CommandError(format!("unexpected argument: {extra}")))
    } else {
        Ok(command)
    }
}

pub fn parse_number(value: &str) -> Result<u64, CommandError> {
    let value = value.replace('_', "");
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    }
    .map_err(|_| CommandError(format!("invalid number: {value}")))
}

pub fn parse_register(name: &str) -> Result<usize, CommandError> {
    let name = name.trim_start_matches('$');
    if name == "fp" {
        return Ok(8);
    }
    match name {
        "pc" => return Ok(PC_REGISTER_INDEX),
        "msip" => return Ok(MSIP_REGISTER_INDEX),
        "mtime" => return Ok(MTIME_REGISTER_INDEX),
        "mtimecmp" => return Ok(MTIMECMP_REGISTER_INDEX),
        "uart_ier" | "ier" => return Ok(UART_IER_REGISTER_INDEX),
        "plic_prio" | "plic_priority" => return Ok(PLIC_UART_PRIORITY_REGISTER_INDEX),
        "plic_pending" => return Ok(PLIC_PENDING_REGISTER_INDEX),
        "plic_menable" | "plic_meie" => return Ok(PLIC_MACHINE_ENABLE_REGISTER_INDEX),
        "plic_senable" | "plic_seie" => return Ok(PLIC_SUPERVISOR_ENABLE_REGISTER_INDEX),
        "plic_mthresh" | "plic_mthreshold" => {
            return Ok(PLIC_MACHINE_THRESHOLD_REGISTER_INDEX);
        }
        "plic_sthresh" | "plic_sthreshold" => {
            return Ok(PLIC_SUPERVISOR_THRESHOLD_REGISTER_INDEX);
        }
        "plic_mclaim" => return Ok(PLIC_MACHINE_CLAIM_REGISTER_INDEX),
        "plic_sclaim" => return Ok(PLIC_SUPERVISOR_CLAIM_REGISTER_INDEX),
        "satp" => return Ok(SATP_REGISTER_INDEX),
        _ => {}
    }
    if let Some(index) = REGISTER_NAMES
        .iter()
        .position(|candidate| *candidate == name)
    {
        return Ok(index);
    }
    if let Some(index) = name
        .strip_prefix('x')
        .and_then(|value| value.parse::<usize>().ok())
    {
        if index < 32 {
            return Ok(index);
        }
    }
    Err(CommandError(format!("unknown register: {name}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Started,
    Stepped,
    Breakpoint(u64),
    UartInput,
    Halted(HaltReason),
}

pub struct Debugger {
    pub machine: Machine,
    image: Vec<u8>,
    load_address: u64,
    memory_size: usize,
    breakpoints: BTreeSet<u64>,
    skip_current_breakpoint: bool,
    uart_wait_output_len: Option<usize>,
}

impl Debugger {
    pub fn new(image: &[u8], load_address: u64, memory_size: usize) -> Result<Self, MachineError> {
        Ok(Self {
            machine: Machine::from_raw(image, load_address, memory_size)?,
            image: image.to_vec(),
            load_address,
            memory_size,
            breakpoints: BTreeSet::new(),
            skip_current_breakpoint: false,
            uart_wait_output_len: None,
        })
    }

    pub fn breakpoints(&self) -> &BTreeSet<u64> {
        &self.breakpoints
    }

    pub fn start(&mut self) -> Result<StopReason, MachineError> {
        self.machine = Machine::from_raw(&self.image, self.load_address, self.memory_size)?;
        self.skip_current_breakpoint = false;
        self.uart_wait_output_len = None;
        Ok(StopReason::Started)
    }

    pub fn step(&mut self) -> Result<StopReason, MachineError> {
        self.skip_current_breakpoint = false;
        match self.machine.step()? {
            Some(reason) => Ok(StopReason::Halted(reason)),
            None if self.should_stop_for_uart_input() => Ok(StopReason::UartInput),
            None => Ok(StopReason::Stepped),
        }
    }

    /// Steps one instruction, but treats calls (`jal`/`jalr` linking into
    /// `ra` or `t0`) as a single step by running until the call returns.
    pub fn next(&mut self, instruction_limit: u64) -> Result<StopReason, MachineError> {
        let Some(return_address) = self.call_return_address() else {
            return self.step();
        };
        let first = self.step()?;
        if first != StopReason::Stepped {
            return Ok(first);
        }
        for _ in 0..instruction_limit {
            let pc = self.machine.cpu.pc;
            if pc == return_address {
                return Ok(StopReason::Stepped);
            }
            if self.breakpoints.contains(&pc) {
                self.skip_current_breakpoint = true;
                return Ok(StopReason::Breakpoint(pc));
            }
            if let Some(reason) = self.machine.step()? {
                return Ok(StopReason::Halted(reason));
            }
            if self.should_stop_for_uart_input() {
                return Ok(StopReason::UartInput);
            }
        }
        Err(MachineError::InstructionLimit(instruction_limit))
    }

    /// Returns the sequential return address if the current instruction is a
    /// call, or `None` if it is any other instruction (or unreadable).
    fn call_return_address(&self) -> Option<u64> {
        const OPCODE_MASK: u32 = 0x7f;
        const OPCODE_JAL: u32 = 0x6f;
        const OPCODE_JALR: u32 = 0x67;
        const LINK_REGISTERS: [u32; 2] = [1, 5]; // ra, t0

        let pc = self.machine.cpu.pc;
        let physical = self
            .machine
            .cpu
            .translate_address_for_debug(&self.machine.bus, pc, AddressAccess::Fetch)
            .ok()?
            .physical_address;
        let half = self.machine.bus.peek_u16(physical).ok()?;
        let size = encoded_instruction_size(half);
        let instruction = if size == 4 {
            self.machine.bus.peek_u32(physical).ok()?
        } else {
            decode_compressed_instruction(half)?
        };
        let opcode = instruction & OPCODE_MASK;
        let rd = (instruction >> 7) & 0x1f;
        let is_call = matches!(opcode, OPCODE_JAL | OPCODE_JALR) && LINK_REGISTERS.contains(&rd);
        is_call.then(|| pc.wrapping_add(size))
    }

    pub fn continue_execution(
        &mut self,
        instruction_limit: u64,
    ) -> Result<StopReason, MachineError> {
        for index in 0..instruction_limit {
            let at_breakpoint = self.breakpoints.contains(&self.machine.cpu.pc);
            if at_breakpoint && !(index == 0 && self.skip_current_breakpoint) {
                self.skip_current_breakpoint = true;
                return Ok(StopReason::Breakpoint(self.machine.cpu.pc));
            }
            self.skip_current_breakpoint = false;
            if let Some(reason) = self.machine.step()? {
                return Ok(StopReason::Halted(reason));
            }
            if self.should_stop_for_uart_input() {
                return Ok(StopReason::UartInput);
            }
        }
        Err(MachineError::InstructionLimit(instruction_limit))
    }

    fn should_stop_for_uart_input(&mut self) -> bool {
        if !self.machine.bus.take_uart_input_wait() {
            return false;
        }

        let output_len = self.machine.bus.uart_output().len();
        if self.uart_wait_output_len == Some(output_len) {
            self.uart_wait_output_len = None;
            true
        } else {
            self.uart_wait_output_len = Some(output_len);
            false
        }
    }

    fn set_register_or_pseudo(&mut self, index: usize, value: u64) {
        match index {
            PC_REGISTER_INDEX => self.machine.cpu.pc = value,
            MSIP_REGISTER_INDEX => self.machine.bus.set_msip(value),
            MTIME_REGISTER_INDEX => self.machine.bus.set_mtime(value),
            MTIMECMP_REGISTER_INDEX => self.machine.bus.set_mtimecmp(value),
            UART_IER_REGISTER_INDEX => self.machine.bus.set_uart_interrupt_enable(value),
            PLIC_UART_PRIORITY_REGISTER_INDEX => self.machine.bus.set_plic_uart_priority(value),
            PLIC_PENDING_REGISTER_INDEX => {}
            PLIC_MACHINE_ENABLE_REGISTER_INDEX => self.machine.bus.set_plic_machine_enable(value),
            PLIC_SUPERVISOR_ENABLE_REGISTER_INDEX => {
                self.machine.bus.set_plic_supervisor_enable(value)
            }
            PLIC_MACHINE_THRESHOLD_REGISTER_INDEX => {
                self.machine.bus.set_plic_machine_threshold(value)
            }
            PLIC_SUPERVISOR_THRESHOLD_REGISTER_INDEX => {
                self.machine.bus.set_plic_supervisor_threshold(value);
            }
            PLIC_MACHINE_CLAIM_REGISTER_INDEX => {
                self.machine.bus.complete_plic_machine_claim(value)
            }
            PLIC_SUPERVISOR_CLAIM_REGISTER_INDEX => {
                self.machine.bus.complete_plic_supervisor_claim(value);
            }
            SATP_REGISTER_INDEX => {}
            _ if index < REGISTER_NAMES.len() => self.machine.cpu.set_register(index, value),
            _ => {}
        }
    }

    pub fn execute(
        &mut self,
        command: Command,
        instruction_limit: u64,
    ) -> Result<Option<StopReason>, MachineError> {
        match command {
            Command::Start => self.start().map(Some),
            Command::Step => self.step().map(Some),
            Command::Next => self.next(instruction_limit).map(Some),
            Command::Break(address) => {
                self.breakpoints.insert(address);
                Ok(None)
            }
            Command::Continue => self.continue_execution(instruction_limit).map(Some),
            Command::UartInput(input) => {
                let mut bytes = input.into_bytes();
                bytes.push(b'\n');
                self.machine.bus.push_uart_input(&bytes);
                self.uart_wait_output_len = None;
                Ok(None)
            }
            Command::SetRegister { index, value } => {
                self.set_register_or_pseudo(index, value);
                Ok(None)
            }
            Command::Undo | Command::Help | Command::Quit => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::DRAM_START;

    fn debugger() -> Debugger {
        let instructions = [0x0010_8093_u32, 0x0010_8093, 0x0010_0073];
        let image: Vec<u8> = instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        Debugger::new(&image, DRAM_START, 4096).unwrap()
    }

    #[test]
    fn parses_gdb_style_commands_and_register_names() {
        assert_eq!("break 0x80000010".parse(), Ok(Command::Break(0x8000_0010)));
        assert_eq!(
            "set a0 0xff".parse(),
            Ok(Command::SetRegister {
                index: 10,
                value: 255
            })
        );
        assert_eq!("next".parse(), Ok(Command::Next));
        assert_eq!("undo".parse(), Ok(Command::Undo));
        assert_eq!(
            "uart Ada Lovelace".parse(),
            Ok(Command::UartInput("Ada Lovelace".into()))
        );
        assert_eq!(
            "set plic_prio 1".parse(),
            Ok(Command::SetRegister {
                index: PLIC_UART_PRIORITY_REGISTER_INDEX,
                value: 1
            })
        );
    }

    #[test]
    fn set_command_updates_platform_pseudo_registers() {
        let mut debugger = debugger();

        debugger
            .execute(
                Command::SetRegister {
                    index: UART_IER_REGISTER_INDEX,
                    value: 1,
                },
                10,
            )
            .unwrap();
        debugger
            .execute(
                Command::SetRegister {
                    index: PLIC_MACHINE_ENABLE_REGISTER_INDEX,
                    value: 1 << 10,
                },
                10,
            )
            .unwrap();

        assert_eq!(debugger.machine.bus.uart_interrupt_enable(), 1);
        assert_eq!(debugger.machine.bus.plic_machine_enable(), 1 << 10);
    }

    #[test]
    fn continue_stops_before_breakpoint_and_can_resume() {
        let mut debugger = debugger();
        debugger.breakpoints.insert(DRAM_START + 4);
        assert_eq!(
            debugger.continue_execution(10).unwrap(),
            StopReason::Breakpoint(DRAM_START + 4)
        );
        assert_eq!(debugger.machine.cpu.register(1), 1);
        assert_eq!(
            debugger.continue_execution(10).unwrap(),
            StopReason::Halted(HaltReason::Breakpoint { code: 0 })
        );
        assert_eq!(debugger.machine.cpu.register(1), 2);
    }

    #[test]
    fn continue_stops_when_guest_waits_for_uart_input() {
        let image: Vec<u8> = [0x0050_c103_u32, 0xfe00_0ee3]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let mut debugger = Debugger::new(&image, DRAM_START, 4096).unwrap();
        debugger.machine.cpu.set_register(1, crate::bus::UART_START);
        assert_eq!(
            debugger.continue_execution(10).unwrap(),
            StopReason::UartInput
        );
    }

    #[test]
    fn next_steps_over_a_call_and_stops_after_it_returns() {
        // jal ra, +8; ebreak; addi x1,x1,1; jalr x0, 0(ra)
        let instructions = [
            0x0080_00ef_u32, // jal ra, 8
            0x0010_0073,     // ebreak
            0x0013_0313,     // addi x6, x6, 1
            0x0000_8067,     // ret
        ];
        let image: Vec<u8> = instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let mut debugger = Debugger::new(&image, DRAM_START, 4096).unwrap();
        assert_eq!(debugger.next(100).unwrap(), StopReason::Stepped);
        assert_eq!(debugger.machine.cpu.pc, DRAM_START + 4);
        assert_eq!(debugger.machine.cpu.register(6), 1);
    }

    #[test]
    fn next_behaves_like_step_on_non_call_instructions() {
        let mut debugger = debugger();
        assert_eq!(debugger.next(100).unwrap(), StopReason::Stepped);
        assert_eq!(debugger.machine.cpu.pc, DRAM_START + 4);
        assert_eq!(debugger.machine.cpu.register(1), 1);
    }

    #[test]
    fn start_restores_cpu_state() {
        let mut debugger = debugger();
        debugger.step().unwrap();
        debugger.machine.cpu.set_register(10, 99);
        debugger.start().unwrap();
        assert_eq!(debugger.machine.cpu.pc, DRAM_START);
        assert_eq!(debugger.machine.cpu.register(1), 0);
        assert_eq!(debugger.machine.cpu.register(10), 0);
    }
}
