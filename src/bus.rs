use std::{collections::VecDeque, fmt};

pub const ROM_START: u64 = 0x0000_1000;
pub const ROM_SIZE: u64 = 0x0000_f000;
pub const CLINT_START: u64 = 0x0200_0000;
pub const CLINT_SIZE: u64 = 0x0001_0000;
pub const PLIC_START: u64 = 0x0c00_0000;
pub const PLIC_SIZE: u64 = 0x0400_0000;
pub const UART_START: u64 = 0x1000_0000;
pub const UART_SIZE: u64 = 0x0000_0100;
pub const VIRTIO_START: u64 = 0x1000_1000;
pub const VIRTIO_SIZE: u64 = 0x0000_1000;
pub const DRAM_START: u64 = 0x8000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Rom,
    Clint,
    Plic,
    Uart,
    Virtio,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rom => "ROM",
            Self::Clint => "CLINT",
            Self::Plic => "PLIC",
            Self::Uart => "UART",
            Self::Virtio => "virtio",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusError {
    Unmapped {
        address: u64,
        size: usize,
    },
    Stub {
        region: Region,
        address: u64,
        size: usize,
    },
}

impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmapped { address, size } => write!(
                f,
                "bus access at {address:#018x} ({size} bytes) is unmapped"
            ),
            Self::Stub {
                region,
                address,
                size,
            } => write!(
                f,
                "bus access at {address:#018x} ({size} bytes) reached unimplemented {region}"
            ),
        }
    }
}

impl std::error::Error for BusError {}

pub struct Bus {
    dram: Vec<u8>,
    clint: Clint,
    uart: Uart,
}

#[derive(Default)]
struct Clint {
    msip: u32,
    mtimecmp: u64,
    mtime: u64,
}

#[derive(Default)]
struct Uart {
    output: Vec<u8>,
    input: VecDeque<u8>,
    waiting_for_input: bool,
}

impl Bus {
    pub fn new(dram_size: usize) -> Self {
        Self {
            dram: vec![0; dram_size],
            clint: Clint {
                mtimecmp: u64::MAX,
                ..Clint::default()
            },
            uart: Uart::default(),
        }
    }

    pub fn dram_size(&self) -> usize {
        self.dram.len()
    }

    pub fn uart_output(&self) -> &[u8] {
        &self.uart.output
    }

    pub fn push_uart_input(&mut self, input: &[u8]) {
        self.uart.input.extend(input);
        if !input.is_empty() {
            self.uart.waiting_for_input = false;
        }
    }

    pub fn take_uart_input_wait(&mut self) -> bool {
        std::mem::replace(&mut self.uart.waiting_for_input, false)
    }

    pub fn load_dram(&mut self, address: u64, data: &[u8]) -> Result<(), BusError> {
        let range = self.dram_range(address, data.len())?;
        self.dram[range].copy_from_slice(data);
        Ok(())
    }

    pub fn tick(&mut self, cycles: u64) {
        self.clint.mtime = self.clint.mtime.wrapping_add(cycles);
    }

    pub fn machine_software_interrupt_pending(&self) -> bool {
        self.clint.msip & 1 != 0
    }

    pub fn machine_timer_interrupt_pending(&self) -> bool {
        self.clint.mtime >= self.clint.mtimecmp
    }

    pub fn msip(&self) -> u64 {
        u64::from(self.clint.msip)
    }

    pub fn set_msip(&mut self, value: u64) {
        self.clint.msip = value as u32;
    }

    pub fn mtime(&self) -> u64 {
        self.clint.mtime
    }

    pub fn set_mtime(&mut self, value: u64) {
        self.clint.mtime = value;
    }

    pub fn mtimecmp(&self) -> u64 {
        self.clint.mtimecmp
    }

    pub fn set_mtimecmp(&mut self, value: u64) {
        self.clint.mtimecmp = value;
    }

    pub fn read_u8(&mut self, address: u64) -> Result<u8, BusError> {
        if let Some(offset) = clint_offset(address, 1) {
            return Ok(self.clint.read(offset, 1)? as u8);
        }
        if let Some(offset) = uart_offset(address, 1) {
            return Ok(self.uart.read(offset));
        }
        Ok(self.dram[self.dram_range(address, 1)?.start])
    }

    /// Side-effect-free variant of [`Bus::read_u8`] for debugger previews:
    /// device reads (e.g. the UART receive buffer) do not consume state.
    pub fn peek_u8(&self, address: u64) -> Result<u8, BusError> {
        if let Some(offset) = clint_offset(address, 1) {
            return Ok(self.clint.read(offset, 1)? as u8);
        }
        if let Some(offset) = uart_offset(address, 1) {
            return Ok(self.uart.peek(offset));
        }
        Ok(self.dram[self.dram_range(address, 1)?.start])
    }

    pub fn read_u16(&self, address: u64) -> Result<u16, BusError> {
        if let Some(offset) = clint_offset(address, 2) {
            return Ok(self.clint.read(offset, 2)? as u16);
        }
        let range = self.dram_range(address, 2)?;
        Ok(u16::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u32(&self, address: u64) -> Result<u32, BusError> {
        if let Some(offset) = clint_offset(address, 4) {
            return Ok(self.clint.read(offset, 4)? as u32);
        }
        let range = self.dram_range(address, 4)?;
        Ok(u32::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u64(&self, address: u64) -> Result<u64, BusError> {
        if let Some(offset) = clint_offset(address, 8) {
            return self.clint.read(offset, 8);
        }
        let range = self.dram_range(address, 8)?;
        Ok(u64::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn write_u8(&mut self, address: u64, value: u8) -> Result<(), BusError> {
        if let Some(offset) = clint_offset(address, 1) {
            self.clint.write(offset, 1, u64::from(value))?;
            return Ok(());
        }
        if let Some(offset) = uart_offset(address, 1) {
            self.uart.write(offset, value);
            return Ok(());
        }
        let index = self.dram_range(address, 1)?.start;
        self.dram[index] = value;
        Ok(())
    }

    pub fn write_u16(&mut self, address: u64, value: u16) -> Result<(), BusError> {
        if let Some(offset) = clint_offset(address, 2) {
            self.clint.write(offset, 2, u64::from(value))?;
            return Ok(());
        }
        let range = self.dram_range(address, 2)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u32(&mut self, address: u64, value: u32) -> Result<(), BusError> {
        if let Some(offset) = clint_offset(address, 4) {
            self.clint.write(offset, 4, u64::from(value))?;
            return Ok(());
        }
        let range = self.dram_range(address, 4)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u64(&mut self, address: u64, value: u64) -> Result<(), BusError> {
        if let Some(offset) = clint_offset(address, 8) {
            self.clint.write(offset, 8, value)?;
            return Ok(());
        }
        let range = self.dram_range(address, 8)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn dram_range(&self, address: u64, size: usize) -> Result<std::ops::Range<usize>, BusError> {
        if let Some(region) = stub_region(address, size) {
            return Err(BusError::Stub {
                region,
                address,
                size,
            });
        }

        let offset = address
            .checked_sub(DRAM_START)
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or(BusError::Unmapped { address, size })?;
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= self.dram.len())
            .ok_or(BusError::Unmapped { address, size })?;
        Ok(offset..end)
    }
}

impl Clint {
    const MSIP: u64 = 0x0000;
    const MSIP_SIZE: u64 = 4;
    const MTIMECMP: u64 = 0x4000;
    const MTIMECMP_SIZE: u64 = 8;
    const MTIME: u64 = 0xbff8;
    const MTIME_SIZE: u64 = 8;

    fn read(&self, offset: u64, size: usize) -> Result<u64, BusError> {
        if register_contains(offset, size, Self::MSIP, Self::MSIP_SIZE) {
            return Ok(read_le_field(
                u64::from(self.msip),
                offset - Self::MSIP,
                size,
            ));
        }
        if register_contains(offset, size, Self::MTIMECMP, Self::MTIMECMP_SIZE) {
            return Ok(read_le_field(self.mtimecmp, offset - Self::MTIMECMP, size));
        }
        if register_contains(offset, size, Self::MTIME, Self::MTIME_SIZE) {
            return Ok(read_le_field(self.mtime, offset - Self::MTIME, size));
        }
        Err(BusError::Unmapped {
            address: CLINT_START + offset,
            size,
        })
    }

    fn write(&mut self, offset: u64, size: usize, value: u64) -> Result<(), BusError> {
        if register_contains(offset, size, Self::MSIP, Self::MSIP_SIZE) {
            let next = write_le_field(u64::from(self.msip), offset - Self::MSIP, size, value);
            self.msip = next as u32;
            return Ok(());
        }
        if register_contains(offset, size, Self::MTIMECMP, Self::MTIMECMP_SIZE) {
            self.mtimecmp = write_le_field(self.mtimecmp, offset - Self::MTIMECMP, size, value);
            return Ok(());
        }
        if register_contains(offset, size, Self::MTIME, Self::MTIME_SIZE) {
            self.mtime = write_le_field(self.mtime, offset - Self::MTIME, size, value);
            return Ok(());
        }
        Err(BusError::Unmapped {
            address: CLINT_START + offset,
            size,
        })
    }
}

impl Uart {
    const RECEIVER_BUFFER_OR_TRANSMIT_HOLDING: u64 = 0;
    const LINE_STATUS: u64 = 5;
    const LINE_STATUS_DATA_READY: u8 = 1;
    const LINE_STATUS_TRANSMIT_HOLDING_EMPTY: u8 = 1 << 5;
    const LINE_STATUS_TRANSMITTER_EMPTY: u8 = 1 << 6;

    fn read(&mut self, offset: u64) -> u8 {
        match offset {
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => {
                let value = self.input.pop_front();
                self.waiting_for_input = value.is_none();
                value.unwrap_or(0)
            }
            Self::LINE_STATUS => {
                let has_input = !self.input.is_empty();
                self.waiting_for_input = !has_input;
                Self::line_status(has_input)
            }
            _ => 0,
        }
    }

    fn peek(&self, offset: u64) -> u8 {
        match offset {
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => {
                self.input.front().copied().unwrap_or(0)
            }
            Self::LINE_STATUS => Self::line_status(!self.input.is_empty()),
            _ => 0,
        }
    }

    fn line_status(has_input: bool) -> u8 {
        let mut status =
            Self::LINE_STATUS_TRANSMIT_HOLDING_EMPTY | Self::LINE_STATUS_TRANSMITTER_EMPTY;
        if has_input {
            status |= Self::LINE_STATUS_DATA_READY;
        }
        status
    }

    fn write(&mut self, offset: u64, value: u8) {
        if offset == Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING {
            self.output.push(value);
        }
    }
}

fn clint_offset(address: u64, size: usize) -> Option<u64> {
    let end = address.checked_add(size as u64)?;
    if address >= CLINT_START && end <= CLINT_START + CLINT_SIZE {
        Some(address - CLINT_START)
    } else {
        None
    }
}

fn uart_offset(address: u64, size: usize) -> Option<u64> {
    let end = address.checked_add(size as u64)?;
    if address >= UART_START && end <= UART_START + UART_SIZE {
        Some(address - UART_START)
    } else {
        None
    }
}

fn register_contains(offset: u64, size: usize, register: u64, register_size: u64) -> bool {
    let Some(end) = offset.checked_add(size as u64) else {
        return false;
    };
    offset >= register && end <= register + register_size
}

fn read_le_field(register: u64, offset: u64, size: usize) -> u64 {
    (register >> (offset * 8)) & byte_mask(size)
}

fn write_le_field(register: u64, offset: u64, size: usize, value: u64) -> u64 {
    let shift = offset * 8;
    let mask = byte_mask(size) << shift;
    (register & !mask) | ((value << shift) & mask)
}

fn byte_mask(size: usize) -> u64 {
    match size {
        8 => u64::MAX,
        _ => (1u64 << (size * 8)) - 1,
    }
}

fn stub_region(address: u64, size: usize) -> Option<Region> {
    let end = address.checked_add(size as u64)?;
    [
        (ROM_START, ROM_SIZE, Region::Rom),
        (PLIC_START, PLIC_SIZE, Region::Plic),
        (VIRTIO_START, VIRTIO_SIZE, Region::Virtio),
    ]
    .into_iter()
    .find_map(|(start, length, region)| {
        (address >= start && end <= start + length).then_some(region)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dram_uses_physical_addresses() {
        let mut bus = Bus::new(16);
        bus.write_u64(DRAM_START + 8, 0x1122_3344_5566_7788)
            .unwrap();
        assert_eq!(bus.read_u64(DRAM_START + 8).unwrap(), 0x1122_3344_5566_7788);
    }

    #[test]
    fn device_windows_are_explicit_stubs() {
        assert_eq!(
            Bus::new(16).read_u8(VIRTIO_START),
            Err(BusError::Stub {
                region: Region::Virtio,
                address: VIRTIO_START,
                size: 1,
            })
        );
    }

    #[test]
    fn uart_transmit_holding_register_captures_output() {
        let mut bus = Bus::new(16);
        bus.write_u8(UART_START, b'O').unwrap();
        bus.write_u8(UART_START, b'K').unwrap();
        assert_eq!(bus.uart_output(), b"OK");
    }

    #[test]
    fn uart_line_status_reports_transmitter_ready() {
        assert_eq!(Bus::new(16).read_u8(UART_START + 5), Ok(0x60));
    }

    #[test]
    fn clint_exposes_timer_registers_and_pending_state() {
        let mut bus = Bus::new(16);
        assert_eq!(bus.read_u64(CLINT_START + 0xbff8), Ok(0));
        assert!(!bus.machine_timer_interrupt_pending());

        bus.write_u64(CLINT_START + 0x4000, 3).unwrap();
        assert_eq!(bus.mtimecmp(), 3);
        bus.tick(2);
        assert_eq!(bus.read_u64(CLINT_START + 0xbff8), Ok(2));
        assert!(!bus.machine_timer_interrupt_pending());
        bus.tick(1);
        assert!(bus.machine_timer_interrupt_pending());

        bus.write_u32(CLINT_START, 1).unwrap();
        assert!(bus.machine_software_interrupt_pending());
        bus.write_u8(CLINT_START, 0).unwrap();
        assert!(!bus.machine_software_interrupt_pending());
    }

    #[test]
    fn clint_supports_subword_timer_accesses() {
        let mut bus = Bus::new(16);
        bus.write_u32(CLINT_START + 0xbff8, 0x5566_7788).unwrap();
        bus.write_u32(CLINT_START + 0xbffc, 0x1122_3344).unwrap();
        assert_eq!(bus.mtime(), 0x1122_3344_5566_7788);
        assert_eq!(bus.read_u16(CLINT_START + 0xbffa), Ok(0x5566));
    }

    #[test]
    fn uart_peek_does_not_consume_input() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"A");
        assert_eq!(bus.peek_u8(UART_START), Ok(b'A'));
        assert_eq!(bus.peek_u8(UART_START + 5), Ok(0x61));
        assert_eq!(bus.read_u8(UART_START), Ok(b'A'));
        assert_eq!(bus.peek_u8(UART_START + 5), Ok(0x60));
    }

    #[test]
    fn uart_receiver_buffer_pops_input_bytes() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"AZ");
        assert_eq!(bus.read_u8(UART_START + 5), Ok(0x61));
        assert_eq!(bus.read_u8(UART_START), Ok(b'A'));
        assert_eq!(bus.read_u8(UART_START), Ok(b'Z'));
        assert_eq!(bus.read_u8(UART_START + 5), Ok(0x60));
        assert_eq!(bus.read_u8(UART_START), Ok(0));
    }
}
