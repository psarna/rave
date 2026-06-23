use crate::bus::DRAM_START;
use crate::{Bus, BusError, Cpu, HaltReason, StepError};
use std::fmt;

#[derive(Debug)]
pub enum MachineError {
    Bus(BusError),
    Step(StepError),
    InstructionLimit(u64),
}

impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::Step(error) => error.fmt(f),
            Self::InstructionLimit(limit) => write!(f, "guest exceeded {limit} instructions"),
        }
    }
}

impl std::error::Error for MachineError {}

pub struct Machine {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Machine {
    pub const LOAD_ADDRESS: u64 = DRAM_START;
    pub const MEMORY_SIZE: usize = 16 * 1024 * 1024;
    pub const INSTRUCTION_LIMIT: u64 = 10_000_000;

    pub fn from_raw(
        image: &[u8],
        load_address: u64,
        memory_size: usize,
    ) -> Result<Self, MachineError> {
        let mut bus = Bus::new(memory_size);
        bus.load_dram(load_address, image)
            .map_err(MachineError::Bus)?;
        let mut cpu = Cpu::new(load_address);
        cpu.set_register(2, (DRAM_START + memory_size as u64) & !0xf);
        Ok(Self { cpu, bus })
    }

    pub fn run(&mut self, instruction_limit: u64) -> Result<HaltReason, MachineError> {
        for _ in 0..instruction_limit {
            if let Some(reason) = self.step()? {
                return Ok(reason);
            }
        }
        Err(MachineError::InstructionLimit(instruction_limit))
    }

    pub fn step(&mut self) -> Result<Option<HaltReason>, MachineError> {
        self.cpu.step(&mut self.bus).map_err(MachineError::Step)
    }
}
