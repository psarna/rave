use crate::{HaltReason, Machine, MachineError};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

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
            "quit" | "q" => no_args(words, Self::Quit),
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
    Halted(HaltReason),
}

pub struct Debugger {
    pub machine: Machine,
    image: Vec<u8>,
    load_address: u64,
    memory_size: usize,
    breakpoints: BTreeSet<u64>,
    skip_current_breakpoint: bool,
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
        })
    }

    pub fn breakpoints(&self) -> &BTreeSet<u64> {
        &self.breakpoints
    }

    pub fn start(&mut self) -> Result<StopReason, MachineError> {
        self.machine = Machine::from_raw(&self.image, self.load_address, self.memory_size)?;
        self.skip_current_breakpoint = false;
        Ok(StopReason::Started)
    }

    pub fn step(&mut self) -> Result<StopReason, MachineError> {
        self.skip_current_breakpoint = false;
        match self.machine.step()? {
            Some(reason) => Ok(StopReason::Halted(reason)),
            None => Ok(StopReason::Stepped),
        }
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
        }
        Err(MachineError::InstructionLimit(instruction_limit))
    }

    pub fn execute(
        &mut self,
        command: Command,
        instruction_limit: u64,
    ) -> Result<Option<StopReason>, MachineError> {
        match command {
            Command::Start => self.start().map(Some),
            Command::Step | Command::Next => self.step().map(Some),
            Command::Break(address) => {
                self.breakpoints.insert(address);
                Ok(None)
            }
            Command::Continue => self.continue_execution(instruction_limit).map(Some),
            Command::SetRegister { index, value } => {
                self.machine.cpu.set_register(index, value);
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
