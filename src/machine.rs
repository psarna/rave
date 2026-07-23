use crate::bus::DRAM_START;
use crate::{Bus, BusError, Cpu, HaltReason, StepError};
use std::fmt;

#[derive(Debug)]
pub enum MachineError {
    Bus(BusError),
    Step(StepError),
    InstructionLimit(u64),
    InvalidDeviceTree(&'static str),
    BootImageDoesNotFit(&'static str),
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
            Self::InvalidDeviceTree(message) => write!(f, "invalid device tree: {message}"),
            Self::BootImageDoesNotFit(image) => {
                write!(f, "boot image {image} does not fit in guest memory")
            }
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
        Self::from_boot_with_initrd(firmware, kernel, None, device_tree, memory_size)
    }

    /// Constructs a firmware boot layout with an optional external initrd.
    ///
    /// The initrd is page-aligned below the device tree, and its exact byte
    /// range is published through `/chosen/linux,initrd-start` and
    /// `/chosen/linux,initrd-end` in the device tree passed to firmware.
    pub fn from_boot_with_initrd(
        firmware: &[u8],
        kernel: &[u8],
        initrd: Option<&[u8]>,
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
        let placeholder_tree = if initrd.is_some() {
            device_tree_with_initrd(device_tree, 0, 0)?
        } else {
            device_tree.to_vec()
        };
        let dtb_address = memory_end
            .checked_sub(placeholder_tree.len() as u64)
            .map(|address| address & !7)
            .ok_or(BusError::Unmapped {
                address: DRAM_START,
                size: placeholder_tree.len(),
            })
            .map_err(MachineError::Bus)?;

        let initrd_layout = initrd
            .map(|image| {
                let limit = dtb_address & !0xfff;
                let start = limit
                    .checked_sub(image.len() as u64)
                    .map(|address| address & !0xfff)
                    .filter(|address| *address >= DRAM_START)
                    .ok_or(MachineError::BootImageDoesNotFit("initrd"))?;
                Ok((start, start + image.len() as u64))
            })
            .transpose()?;
        let device_tree = if let Some((start, end)) = initrd_layout {
            device_tree_with_initrd(device_tree, start, end)?
        } else {
            placeholder_tree
        };

        let mut images = vec![
            ("firmware", Self::FIRMWARE_ADDRESS, firmware.len()),
            ("kernel", Self::KERNEL_ADDRESS, kernel.len()),
            ("device tree", dtb_address, device_tree.len()),
        ];
        if let (Some(image), Some((start, _))) = (initrd, initrd_layout) {
            images.push(("initrd", start, image.len()));
        }
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
        bus.load_dram(dtb_address, &device_tree)
            .map_err(MachineError::Bus)?;
        if let (Some(image), Some((start, _))) = (initrd, initrd_layout) {
            bus.load_dram(start, image).map_err(MachineError::Bus)?;
        }

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

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_HEADER_SIZE: usize = 40;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

fn device_tree_with_initrd(
    device_tree: &[u8],
    initrd_start: u64,
    initrd_end: u64,
) -> Result<Vec<u8>, MachineError> {
    if read_be_u32(device_tree, 0)? != FDT_MAGIC {
        return Err(MachineError::InvalidDeviceTree("bad FDT magic"));
    }
    let structure_offset = read_be_u32(device_tree, 8)? as usize;
    let strings_offset = read_be_u32(device_tree, 12)? as usize;
    let strings_size = read_be_u32(device_tree, 32)? as usize;
    let structure_size = read_be_u32(device_tree, 36)? as usize;
    let structure_end = structure_offset
        .checked_add(structure_size)
        .filter(|end| *end <= device_tree.len())
        .ok_or(MachineError::InvalidDeviceTree(
            "structure block is out of bounds",
        ))?;
    let strings_end = strings_offset
        .checked_add(strings_size)
        .filter(|end| *end <= device_tree.len())
        .ok_or(MachineError::InvalidDeviceTree(
            "strings block is out of bounds",
        ))?;
    if structure_offset < FDT_HEADER_SIZE || strings_offset < structure_end {
        return Err(MachineError::InvalidDeviceTree(
            "unsupported FDT block layout",
        ));
    }

    let strings = &device_tree[strings_offset..strings_end];
    let mut next_strings = strings.to_vec();
    let start_name = string_offset_or_append(&mut next_strings, "linux,initrd-start")?;
    let end_name = string_offset_or_append(&mut next_strings, "linux,initrd-end")?;
    let structure = &device_tree[structure_offset..structure_end];
    let mut next_structure = Vec::with_capacity(structure.len() + 40);
    let mut cursor = 0;
    let mut depth = 0usize;
    let mut chosen_depth = None;
    let mut found_chosen = false;

    loop {
        let token_start = cursor;
        let token = read_be_u32(structure, cursor)?;
        cursor += 4;
        match token {
            FDT_BEGIN_NODE => {
                let name_end = structure
                    .get(cursor..)
                    .and_then(|bytes| bytes.iter().position(|byte| *byte == 0))
                    .map(|length| cursor + length)
                    .ok_or(MachineError::InvalidDeviceTree("unterminated node name"))?;
                let name = std::str::from_utf8(&structure[cursor..name_end])
                    .map_err(|_| MachineError::InvalidDeviceTree("node name is not UTF-8"))?;
                cursor = align_up(name_end + 1, 4)
                    .filter(|end| *end <= structure.len())
                    .ok_or(MachineError::InvalidDeviceTree(
                        "node extends past structure block",
                    ))?;
                depth += 1;
                if depth == 2 && name == "chosen" {
                    chosen_depth = Some(depth);
                    found_chosen = true;
                }
                next_structure.extend_from_slice(&structure[token_start..cursor]);
            }
            FDT_PROP => {
                let length = read_be_u32(structure, cursor)? as usize;
                let name_offset = read_be_u32(structure, cursor + 4)? as usize;
                cursor = cursor
                    .checked_add(8)
                    .and_then(|start| start.checked_add(length))
                    .and_then(|end| align_up(end, 4))
                    .filter(|end| *end <= structure.len())
                    .ok_or(MachineError::InvalidDeviceTree(
                        "property extends past structure block",
                    ))?;
                let name = fdt_string(strings, name_offset)?;
                if chosen_depth == Some(depth)
                    && matches!(name, "linux,initrd-start" | "linux,initrd-end")
                {
                    continue;
                }
                next_structure.extend_from_slice(&structure[token_start..cursor]);
            }
            FDT_END_NODE => {
                if chosen_depth == Some(depth) {
                    append_fdt_u64_property(&mut next_structure, start_name, initrd_start);
                    append_fdt_u64_property(&mut next_structure, end_name, initrd_end);
                    chosen_depth = None;
                }
                next_structure.extend_from_slice(&structure[token_start..cursor]);
                depth = depth
                    .checked_sub(1)
                    .ok_or(MachineError::InvalidDeviceTree("unbalanced node tokens"))?;
            }
            FDT_NOP => next_structure.extend_from_slice(&structure[token_start..cursor]),
            FDT_END => {
                next_structure.extend_from_slice(&structure[token_start..cursor]);
                break;
            }
            _ => return Err(MachineError::InvalidDeviceTree("unknown structure token")),
        }
    }
    if !found_chosen {
        return Err(MachineError::InvalidDeviceTree("missing /chosen node"));
    }

    let mut result = device_tree[..structure_offset].to_vec();
    result.extend_from_slice(&next_structure);
    let next_strings_offset = result.len();
    result.extend_from_slice(&next_strings);
    let total_size = result.len();
    write_be_u32(&mut result, 4, total_size)?;
    write_be_u32(&mut result, 12, next_strings_offset)?;
    write_be_u32(&mut result, 32, next_strings.len())?;
    write_be_u32(&mut result, 36, next_structure.len())?;
    Ok(result)
}

fn append_fdt_u64_property(structure: &mut Vec<u8>, name_offset: u32, value: u64) {
    structure.extend_from_slice(&FDT_PROP.to_be_bytes());
    structure.extend_from_slice(&8u32.to_be_bytes());
    structure.extend_from_slice(&name_offset.to_be_bytes());
    structure.extend_from_slice(&value.to_be_bytes());
}

fn string_offset_or_append(strings: &mut Vec<u8>, name: &str) -> Result<u32, MachineError> {
    if let Some(offset) = strings
        .windows(name.len() + 1)
        .position(|window| window[..name.len()] == *name.as_bytes() && window[name.len()] == 0)
    {
        return u32::try_from(offset)
            .map_err(|_| MachineError::InvalidDeviceTree("strings block is too large"));
    }
    let offset = u32::try_from(strings.len())
        .map_err(|_| MachineError::InvalidDeviceTree("strings block is too large"))?;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    Ok(offset)
}

fn fdt_string(strings: &[u8], offset: usize) -> Result<&str, MachineError> {
    let bytes = strings
        .get(offset..)
        .ok_or(MachineError::InvalidDeviceTree(
            "property name is out of bounds",
        ))?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(MachineError::InvalidDeviceTree(
            "unterminated property name",
        ))?;
    std::str::from_utf8(&bytes[..end])
        .map_err(|_| MachineError::InvalidDeviceTree("property name is not UTF-8"))
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Result<u32, MachineError> {
    let end = offset
        .checked_add(4)
        .ok_or(MachineError::InvalidDeviceTree("truncated FDT"))?;
    bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or(MachineError::InvalidDeviceTree("truncated FDT"))
}

fn write_be_u32(bytes: &mut [u8], offset: usize, value: usize) -> Result<(), MachineError> {
    let value =
        u32::try_from(value).map_err(|_| MachineError::InvalidDeviceTree("FDT is too large"))?;
    let destination = bytes
        .get_mut(offset..offset + 4)
        .ok_or(MachineError::InvalidDeviceTree("truncated FDT header"))?;
    destination.copy_from_slice(&value.to_be_bytes());
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
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

    #[test]
    fn boot_layout_loads_initrd_and_publishes_its_exact_range() {
        let device_tree = include_bytes!("../demo/rave.dtb");
        let initrd = vec![0xa5; 6001];
        let mut machine = Machine::from_boot_with_initrd(
            &[1, 2, 3, 4],
            &[5, 6, 7, 8],
            Some(&initrd),
            device_tree,
            Machine::BOOT_MEMORY_SIZE,
        )
        .unwrap();

        let dtb_address = machine.cpu.register(11);
        let header: Vec<_> = (0..FDT_HEADER_SIZE)
            .map(|offset| machine.bus.read_u8(dtb_address + offset as u64).unwrap())
            .collect();
        let total_size = read_be_u32(&header, 4).unwrap() as usize;
        let dtb: Vec<_> = (0..total_size)
            .map(|offset| machine.bus.read_u8(dtb_address + offset as u64).unwrap())
            .collect();
        let tree = fdt::Fdt::new(&dtb).unwrap();
        let chosen = tree.find_node("/chosen").unwrap();
        let start = u64::from_be_bytes(
            chosen
                .property("linux,initrd-start")
                .unwrap()
                .value
                .try_into()
                .unwrap(),
        );
        let end = u64::from_be_bytes(
            chosen
                .property("linux,initrd-end")
                .unwrap()
                .value
                .try_into()
                .unwrap(),
        );

        assert_eq!(start & 0xfff, 0);
        assert_eq!(end - start, initrd.len() as u64);
        assert!(end <= dtb_address);
        assert_eq!(machine.bus.read_u8(start), Ok(0xa5));
        assert_eq!(machine.bus.read_u8(end - 1), Ok(0xa5));
    }

    #[test]
    fn initrd_properties_replace_existing_values() {
        let original = include_bytes!("../demo/rave.dtb");
        let first = device_tree_with_initrd(original, 0x8100_0000, 0x8110_0000).unwrap();
        let second = device_tree_with_initrd(&first, 0x8200_0000, 0x8220_0000).unwrap();
        let tree = fdt::Fdt::new(&second).unwrap();
        let chosen = tree.find_node("/chosen").unwrap();
        let properties: Vec<_> = chosen
            .properties()
            .filter(|property| property.name.starts_with("linux,initrd-"))
            .collect();

        assert_eq!(properties.len(), 2);
        assert_eq!(
            properties
                .iter()
                .find(|property| property.name == "linux,initrd-start")
                .unwrap()
                .value,
            0x8200_0000u64.to_be_bytes()
        );
        assert_eq!(
            properties
                .iter()
                .find(|property| property.name == "linux,initrd-end")
                .unwrap()
                .value,
            0x8220_0000u64.to_be_bytes()
        );
    }
}
