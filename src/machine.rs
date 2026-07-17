use crate::bus::DRAM_START;
use crate::{Bus, BusError, Cpu, HaltReason, StepError};
use std::fmt;

#[derive(Debug)]
pub enum MachineError {
    Bus(BusError),
    Step(StepError),
    InstructionLimit(u64),
    BootImageOverlap {
        first: &'static str,
        second: &'static str,
    },
}

impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::Step(error) => error.fmt(f),
            Self::InstructionLimit(limit) => write!(f, "guest exceeded {limit} instructions"),
            Self::BootImageOverlap { first, second } => {
                write!(
                    f,
                    "boot images {first} and {second} overlap in guest memory"
                )
            }
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
    pub const BOOT_MEMORY_SIZE: usize = 128 * 1024 * 1024;
    pub const FIRMWARE_ADDRESS: u64 = DRAM_START;
    pub const KERNEL_ADDRESS: u64 = DRAM_START + 0x0020_0000;
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

    /// Constructs a conventional single-hart RISC-V firmware boot layout.
    ///
    /// OpenSBI starts at the beginning of DRAM, the kernel follows at the
    /// standard 2 MiB offset, and the device tree is placed near the top of
    /// RAM. Register `a0` contains hart ID zero and `a1` points at the DTB.
    pub fn from_boot(
        firmware: &[u8],
        kernel: &[u8],
        device_tree: &[u8],
        memory_size: usize,
    ) -> Result<Self, MachineError> {
        let memory_end = DRAM_START
            .checked_add(memory_size as u64)
            .ok_or(BusError::Unmapped {
                address: DRAM_START,
                size: memory_size,
            })
            .map_err(MachineError::Bus)?;
        let dtb_address = memory_end
            .checked_sub(device_tree.len() as u64)
            .map(|address| address & !7)
            .ok_or(BusError::Unmapped {
                address: DRAM_START,
                size: device_tree.len(),
            })
            .map_err(MachineError::Bus)?;

        let images = [
            ("firmware", Self::FIRMWARE_ADDRESS, firmware.len()),
            ("kernel", Self::KERNEL_ADDRESS, kernel.len()),
            ("device tree", dtb_address, device_tree.len()),
        ];
        for (index, (first, first_address, first_size)) in images.iter().enumerate() {
            for (second, second_address, second_size) in images.iter().skip(index + 1) {
                if ranges_overlap(*first_address, *first_size, *second_address, *second_size) {
                    return Err(MachineError::BootImageOverlap { first, second });
                }
            }
        }

        let mut bus = Bus::new(memory_size);
        bus.load_dram(Self::FIRMWARE_ADDRESS, firmware)
            .map_err(MachineError::Bus)?;
        bus.load_dram(Self::KERNEL_ADDRESS, kernel)
            .map_err(MachineError::Bus)?;
        bus.load_dram(dtb_address, device_tree)
            .map_err(MachineError::Bus)?;

        let mut cpu = Cpu::new(Self::FIRMWARE_ADDRESS);
        cpu.set_host_ebreak_exit(false);
        cpu.set_register(2, memory_end & !0xf);
        cpu.set_register(10, 0);
        cpu.set_register(11, dtb_address);
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

fn ranges_overlap(
    first_address: u64,
    first_size: usize,
    second_address: u64,
    second_size: usize,
) -> bool {
    if first_size == 0 || second_size == 0 {
        return false;
    }
    let first_end = first_address.saturating_add(first_size as u64);
    let second_end = second_address.saturating_add(second_size as u64);
    first_address < second_end && second_address < first_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_layout_loads_images_and_sets_firmware_arguments() {
        let firmware = [1, 2, 3, 4];
        let kernel = [5, 6, 7, 8];
        let device_tree = [9, 10, 11, 12];
        let mut machine =
            Machine::from_boot(&firmware, &kernel, &device_tree, 4 * 1024 * 1024).unwrap();

        assert_eq!(machine.cpu.pc, Machine::FIRMWARE_ADDRESS);
        assert_eq!(machine.cpu.register(10), 0);
        assert_eq!(machine.cpu.register(11) & 7, 0);
        assert_eq!(
            machine.bus.read_u32(Machine::FIRMWARE_ADDRESS),
            Ok(0x0403_0201)
        );
        assert_eq!(
            machine.bus.read_u32(Machine::KERNEL_ADDRESS),
            Ok(0x0807_0605)
        );
        assert_eq!(
            machine.bus.read_u32(machine.cpu.register(11)),
            Ok(0x0c0b_0a09)
        );
    }

    #[test]
    fn boot_layout_rejects_images_that_overlap() {
        let firmware = vec![0; 0x0020_0001];
        let error = Machine::from_boot(&firmware, &[0], &[], 4 * 1024 * 1024)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            MachineError::BootImageOverlap {
                first: "firmware",
                second: "kernel"
            }
        ));
    }
}
