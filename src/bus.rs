use std::{cell::RefCell, collections::VecDeque, fmt};

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
    uart: Uart,
}

#[derive(Default)]
struct Uart {
    output: Vec<u8>,
    input: RefCell<VecDeque<u8>>,
    waiting_for_input: RefCell<bool>,
}

impl Bus {
    pub fn new(dram_size: usize) -> Self {
        Self {
            dram: vec![0; dram_size],
            uart: Uart::default(),
        }
    }

    pub fn dram_size(&self) -> usize {
        self.dram.len()
    }

    pub fn uart_output(&self) -> &[u8] {
        &self.uart.output
    }

    pub fn push_uart_input(&self, input: &[u8]) {
        self.uart.input.borrow_mut().extend(input);
        if !input.is_empty() {
            self.uart.waiting_for_input.replace(false);
        }
    }

    pub fn take_uart_input_wait(&self) -> bool {
        self.uart.waiting_for_input.replace(false)
    }

    pub fn load_dram(&mut self, address: u64, data: &[u8]) -> Result<(), BusError> {
        let range = self.dram_range(address, data.len())?;
        self.dram[range].copy_from_slice(data);
        Ok(())
    }

    pub fn read_u8(&self, address: u64) -> Result<u8, BusError> {
        if let Some(offset) = uart_offset(address, 1) {
            return Ok(self.uart.read(offset));
        }
        Ok(self.dram[self.dram_range(address, 1)?.start])
    }

    pub fn read_u16(&self, address: u64) -> Result<u16, BusError> {
        let range = self.dram_range(address, 2)?;
        Ok(u16::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u32(&self, address: u64) -> Result<u32, BusError> {
        let range = self.dram_range(address, 4)?;
        Ok(u32::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u64(&self, address: u64) -> Result<u64, BusError> {
        let range = self.dram_range(address, 8)?;
        Ok(u64::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn write_u8(&mut self, address: u64, value: u8) -> Result<(), BusError> {
        if let Some(offset) = uart_offset(address, 1) {
            self.uart.write(offset, value);
            return Ok(());
        }
        let index = self.dram_range(address, 1)?.start;
        self.dram[index] = value;
        Ok(())
    }

    pub fn write_u16(&mut self, address: u64, value: u16) -> Result<(), BusError> {
        let range = self.dram_range(address, 2)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u32(&mut self, address: u64, value: u32) -> Result<(), BusError> {
        let range = self.dram_range(address, 4)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u64(&mut self, address: u64, value: u64) -> Result<(), BusError> {
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

impl Uart {
    const RECEIVER_BUFFER_OR_TRANSMIT_HOLDING: u64 = 0;
    const LINE_STATUS: u64 = 5;
    const LINE_STATUS_DATA_READY: u8 = 1;
    const LINE_STATUS_TRANSMIT_HOLDING_EMPTY: u8 = 1 << 5;
    const LINE_STATUS_TRANSMITTER_EMPTY: u8 = 1 << 6;

    fn read(&self, offset: u64) -> u8 {
        match offset {
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => {
                let value = self.input.borrow_mut().pop_front();
                self.waiting_for_input.replace(value.is_none());
                value.unwrap_or(0)
            }
            Self::LINE_STATUS => {
                let mut status =
                    Self::LINE_STATUS_TRANSMIT_HOLDING_EMPTY | Self::LINE_STATUS_TRANSMITTER_EMPTY;
                let has_input = !self.input.borrow().is_empty();
                if has_input {
                    status |= Self::LINE_STATUS_DATA_READY;
                }
                self.waiting_for_input.replace(!has_input);
                status
            }
            _ => 0,
        }
    }

    fn write(&mut self, offset: u64, value: u8) {
        if offset == Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING {
            self.output.push(value);
        }
    }
}

fn uart_offset(address: u64, size: usize) -> Option<u64> {
    let end = address.checked_add(size as u64)?;
    (address >= UART_START && end <= UART_START + UART_SIZE).then_some(address - UART_START)
}

fn stub_region(address: u64, size: usize) -> Option<Region> {
    let end = address.checked_add(size as u64)?;
    [
        (ROM_START, ROM_SIZE, Region::Rom),
        (CLINT_START, CLINT_SIZE, Region::Clint),
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
    fn uart_receiver_buffer_pops_input_bytes() {
        let bus = Bus::new(16);
        bus.push_uart_input(b"AZ");
        assert_eq!(bus.read_u8(UART_START + 5), Ok(0x61));
        assert_eq!(bus.read_u8(UART_START), Ok(b'A'));
        assert_eq!(bus.read_u8(UART_START), Ok(b'Z'));
        assert_eq!(bus.read_u8(UART_START + 5), Ok(0x60));
        assert_eq!(bus.read_u8(UART_START), Ok(0));
    }
}
