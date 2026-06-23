use std::fmt;

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
}

impl Bus {
    pub fn new(dram_size: usize) -> Self {
        Self {
            dram: vec![0; dram_size],
        }
    }

    pub fn dram_size(&self) -> usize {
        self.dram.len()
    }

    pub fn load_dram(&mut self, address: u64, data: &[u8]) -> Result<(), BusError> {
        let range = self.dram_range(address, data.len())?;
        self.dram[range].copy_from_slice(data);
        Ok(())
    }

    pub fn read_u8(&self, address: u64) -> Result<u8, BusError> {
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

fn stub_region(address: u64, size: usize) -> Option<Region> {
    let end = address.checked_add(size as u64)?;
    [
        (ROM_START, ROM_SIZE, Region::Rom),
        (CLINT_START, CLINT_SIZE, Region::Clint),
        (PLIC_START, PLIC_SIZE, Region::Plic),
        (UART_START, UART_SIZE, Region::Uart),
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
            Bus::new(16).read_u8(UART_START),
            Err(BusError::Stub {
                region: Region::Uart,
                address: UART_START,
                size: 1,
            })
        );
    }
}
