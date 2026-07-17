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
    plic: Plic,
    uart: Uart,
}

#[derive(Default)]
struct Clint {
    msip: u32,
    mtimecmp: u64,
    mtime: u64,
}

#[derive(Default)]
struct Plic {
    priorities: [u32; Plic::SOURCE_COUNT + 1],
    enables: [u32; Plic::CONTEXT_COUNT],
    thresholds: [u32; Plic::CONTEXT_COUNT],
    uart_claimed: bool,
}

#[derive(Default)]
struct Uart {
    output: Vec<u8>,
    input: VecDeque<u8>,
    interrupt_enable: u8,
    line_control: u8,
    divisor_latch: u16,
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
            plic: Plic::default(),
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

    pub fn machine_external_interrupt_pending(&self) -> bool {
        self.plic
            .context_interrupt_pending(Plic::MACHINE_CONTEXT, &self.uart)
    }

    pub fn supervisor_external_interrupt_pending(&self) -> bool {
        self.plic
            .context_interrupt_pending(Plic::SUPERVISOR_CONTEXT, &self.uart)
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

    pub fn uart_interrupt_enable(&self) -> u64 {
        u64::from(self.uart.interrupt_enable)
    }

    pub fn set_uart_interrupt_enable(&mut self, value: u64) {
        self.uart.interrupt_enable = (value as u8) & Uart::INTERRUPT_ENABLE_RECEIVED_DATA_AVAILABLE;
    }

    pub fn plic_uart_priority(&self) -> u64 {
        u64::from(self.plic.priorities[Plic::UART_SOURCE as usize])
    }

    pub fn set_plic_uart_priority(&mut self, value: u64) {
        self.plic.priorities[Plic::UART_SOURCE as usize] = (value as u32) & 0x7;
    }

    pub fn plic_pending(&self) -> u64 {
        u64::from(self.plic.pending_bits(&self.uart))
    }

    pub fn plic_machine_enable(&self) -> u64 {
        u64::from(self.plic.enables[Plic::MACHINE_CONTEXT])
    }

    pub fn set_plic_machine_enable(&mut self, value: u64) {
        self.plic.enables[Plic::MACHINE_CONTEXT] = (value as u32) & Plic::UART_SOURCE_BIT;
    }

    pub fn plic_supervisor_enable(&self) -> u64 {
        u64::from(self.plic.enables[Plic::SUPERVISOR_CONTEXT])
    }

    pub fn set_plic_supervisor_enable(&mut self, value: u64) {
        self.plic.enables[Plic::SUPERVISOR_CONTEXT] = (value as u32) & Plic::UART_SOURCE_BIT;
    }

    pub fn plic_machine_threshold(&self) -> u64 {
        u64::from(self.plic.thresholds[Plic::MACHINE_CONTEXT])
    }

    pub fn set_plic_machine_threshold(&mut self, value: u64) {
        self.plic.thresholds[Plic::MACHINE_CONTEXT] = (value as u32) & 0x7;
    }

    pub fn plic_supervisor_threshold(&self) -> u64 {
        u64::from(self.plic.thresholds[Plic::SUPERVISOR_CONTEXT])
    }

    pub fn set_plic_supervisor_threshold(&mut self, value: u64) {
        self.plic.thresholds[Plic::SUPERVISOR_CONTEXT] = (value as u32) & 0x7;
    }

    pub fn plic_machine_claim(&self) -> u64 {
        u64::from(self.plic.peek_claim(Plic::MACHINE_CONTEXT, &self.uart))
    }

    pub fn complete_plic_machine_claim(&mut self, value: u64) {
        self.plic.complete(value as u32);
    }

    pub fn plic_supervisor_claim(&self) -> u64 {
        u64::from(self.plic.peek_claim(Plic::SUPERVISOR_CONTEXT, &self.uart))
    }

    pub fn complete_plic_supervisor_claim(&mut self, value: u64) {
        self.plic.complete(value as u32);
    }

    pub fn read_u8(&mut self, address: u64) -> Result<u8, BusError> {
        if let Some(offset) = clint_offset(address, 1) {
            return Ok(self.clint.read(offset, 1)? as u8);
        }
        if let Some(offset) = plic_offset(address, 1) {
            return Ok(self.plic.read(offset, 1, &self.uart)? as u8);
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
        if let Some(offset) = plic_offset(address, 1) {
            return Ok(self.plic.peek(offset, 1, &self.uart)? as u8);
        }
        if let Some(offset) = uart_offset(address, 1) {
            return Ok(self.uart.peek(offset));
        }
        Ok(self.dram[self.dram_range(address, 1)?.start])
    }

    pub fn read_u16(&mut self, address: u64) -> Result<u16, BusError> {
        if uart_offset(address, 2).is_some() {
            return Ok(self.read_uart_le(address, 2) as u16);
        }
        if let Some(offset) = clint_offset(address, 2) {
            return Ok(self.clint.read(offset, 2)? as u16);
        }
        if let Some(offset) = plic_offset(address, 2) {
            return Ok(self.plic.read(offset, 2, &self.uart)? as u16);
        }
        let range = self.dram_range(address, 2)?;
        Ok(u16::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u32(&mut self, address: u64) -> Result<u32, BusError> {
        if uart_offset(address, 4).is_some() {
            return Ok(self.read_uart_le(address, 4) as u32);
        }
        if let Some(offset) = clint_offset(address, 4) {
            return Ok(self.clint.read(offset, 4)? as u32);
        }
        if let Some(offset) = plic_offset(address, 4) {
            return Ok(self.plic.read(offset, 4, &self.uart)? as u32);
        }
        let range = self.dram_range(address, 4)?;
        Ok(u32::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn read_u64(&mut self, address: u64) -> Result<u64, BusError> {
        if uart_offset(address, 8).is_some() {
            return Ok(self.read_uart_le(address, 8));
        }
        if let Some(offset) = clint_offset(address, 8) {
            return self.clint.read(offset, 8);
        }
        if let Some(offset) = plic_offset(address, 8) {
            return self.plic.read(offset, 8, &self.uart);
        }
        let range = self.dram_range(address, 8)?;
        Ok(u64::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    /// Side-effect-free variants of the read methods for debugger previews.
    pub fn peek_u16(&self, address: u64) -> Result<u16, BusError> {
        if uart_offset(address, 2).is_some() {
            return Ok(self.peek_uart_le(address, 2) as u16);
        }
        if let Some(offset) = clint_offset(address, 2) {
            return Ok(self.clint.read(offset, 2)? as u16);
        }
        if let Some(offset) = plic_offset(address, 2) {
            return Ok(self.plic.peek(offset, 2, &self.uart)? as u16);
        }
        let range = self.dram_range(address, 2)?;
        Ok(u16::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn peek_u32(&self, address: u64) -> Result<u32, BusError> {
        if uart_offset(address, 4).is_some() {
            return Ok(self.peek_uart_le(address, 4) as u32);
        }
        if let Some(offset) = clint_offset(address, 4) {
            return Ok(self.clint.read(offset, 4)? as u32);
        }
        if let Some(offset) = plic_offset(address, 4) {
            return Ok(self.plic.peek(offset, 4, &self.uart)? as u32);
        }
        let range = self.dram_range(address, 4)?;
        Ok(u32::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn peek_u64(&self, address: u64) -> Result<u64, BusError> {
        if uart_offset(address, 8).is_some() {
            return Ok(self.peek_uart_le(address, 8));
        }
        if let Some(offset) = clint_offset(address, 8) {
            return self.clint.read(offset, 8);
        }
        if let Some(offset) = plic_offset(address, 8) {
            return self.plic.peek(offset, 8, &self.uart);
        }
        let range = self.dram_range(address, 8)?;
        Ok(u64::from_le_bytes(self.dram[range].try_into().unwrap()))
    }

    pub fn write_u8(&mut self, address: u64, value: u8) -> Result<(), BusError> {
        if let Some(offset) = clint_offset(address, 1) {
            self.clint.write(offset, 1, u64::from(value))?;
            return Ok(());
        }
        if let Some(offset) = plic_offset(address, 1) {
            self.plic.write(offset, 1, u64::from(value))?;
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
        if uart_offset(address, 2).is_some() {
            self.write_uart_le(address, u64::from(value), 2);
            return Ok(());
        }
        if let Some(offset) = clint_offset(address, 2) {
            self.clint.write(offset, 2, u64::from(value))?;
            return Ok(());
        }
        if let Some(offset) = plic_offset(address, 2) {
            self.plic.write(offset, 2, u64::from(value))?;
            return Ok(());
        }
        let range = self.dram_range(address, 2)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u32(&mut self, address: u64, value: u32) -> Result<(), BusError> {
        if uart_offset(address, 4).is_some() {
            self.write_uart_le(address, u64::from(value), 4);
            return Ok(());
        }
        if let Some(offset) = clint_offset(address, 4) {
            self.clint.write(offset, 4, u64::from(value))?;
            return Ok(());
        }
        if let Some(offset) = plic_offset(address, 4) {
            self.plic.write(offset, 4, u64::from(value))?;
            return Ok(());
        }
        let range = self.dram_range(address, 4)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u64(&mut self, address: u64, value: u64) -> Result<(), BusError> {
        if uart_offset(address, 8).is_some() {
            self.write_uart_le(address, value, 8);
            return Ok(());
        }
        if let Some(offset) = clint_offset(address, 8) {
            self.clint.write(offset, 8, value)?;
            return Ok(());
        }
        if let Some(offset) = plic_offset(address, 8) {
            self.plic.write(offset, 8, value)?;
            return Ok(());
        }
        let range = self.dram_range(address, 8)?;
        self.dram[range].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Reads a little-endian value from the UART window one byte at a time,
    /// so wide accesses see the same registers as byte accesses.
    fn read_uart_le(&mut self, address: u64, size: usize) -> u64 {
        let mut value = 0u64;
        for index in 0..size {
            let offset = address + index as u64 - UART_START;
            value |= u64::from(self.uart.read(offset)) << (index * 8);
        }
        value
    }

    fn peek_uart_le(&self, address: u64, size: usize) -> u64 {
        let mut value = 0u64;
        for index in 0..size {
            let offset = address + index as u64 - UART_START;
            value |= u64::from(self.uart.peek(offset)) << (index * 8);
        }
        value
    }

    fn write_uart_le(&mut self, address: u64, value: u64, size: usize) {
        for index in 0..size {
            let offset = address + index as u64 - UART_START;
            self.uart.write(offset, (value >> (index * 8)) as u8);
        }
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

impl Plic {
    const SOURCE_COUNT: usize = 10;
    const UART_SOURCE: u32 = 10;
    const UART_SOURCE_BIT: u32 = 1 << Self::UART_SOURCE;
    const CONTEXT_COUNT: usize = 2;
    const MACHINE_CONTEXT: usize = 0;
    const SUPERVISOR_CONTEXT: usize = 1;
    const PRIORITY_BASE: u64 = 0x0000;
    const PENDING_BASE: u64 = 0x1000;
    const ENABLE_BASE: u64 = 0x2000;
    const ENABLE_STRIDE: u64 = 0x80;
    const CONTEXT_BASE: u64 = 0x20_0000;
    const CONTEXT_STRIDE: u64 = 0x1000;
    const THRESHOLD_OFFSET: u64 = 0;
    const CLAIM_COMPLETE_OFFSET: u64 = 4;
    const REGISTER_SIZE: u64 = 4;

    fn context_interrupt_pending(&self, context: usize, uart: &Uart) -> bool {
        self.pending_source(uart)
            .is_some_and(|source| self.source_enabled(context, source))
    }

    fn read(&mut self, offset: u64, size: usize, uart: &Uart) -> Result<u64, BusError> {
        if let Some(context) = context_claim_complete(offset, size) {
            return Ok(read_le_field(
                u64::from(self.claim(context, uart)),
                offset - context_register(context, Self::CLAIM_COMPLETE_OFFSET),
                size,
            ));
        }
        self.peek(offset, size, uart)
    }

    fn peek(&self, offset: u64, size: usize, uart: &Uart) -> Result<u64, BusError> {
        if let Some(source) = priority_source(offset, size) {
            return Ok(read_le_field(
                u64::from(self.priorities[source]),
                offset - (Self::PRIORITY_BASE + source as u64 * Self::REGISTER_SIZE),
                size,
            ));
        }
        if register_contains(offset, size, Self::PENDING_BASE, Self::REGISTER_SIZE) {
            return Ok(read_le_field(
                u64::from(self.pending_bits(uart)),
                offset - Self::PENDING_BASE,
                size,
            ));
        }
        if let Some(context) = context_enable(offset, size) {
            return Ok(read_le_field(
                u64::from(self.enables[context]),
                offset - enable_register(context),
                size,
            ));
        }
        if let Some(context) = context_threshold(offset, size) {
            return Ok(read_le_field(
                u64::from(self.thresholds[context]),
                offset - context_register(context, Self::THRESHOLD_OFFSET),
                size,
            ));
        }
        if let Some(context) = context_claim_complete(offset, size) {
            let claim = self.peek_claim(context, uart);
            return Ok(read_le_field(
                u64::from(claim),
                offset - context_register(context, Self::CLAIM_COMPLETE_OFFSET),
                size,
            ));
        }
        Err(BusError::Unmapped {
            address: PLIC_START + offset,
            size,
        })
    }

    fn write(&mut self, offset: u64, size: usize, value: u64) -> Result<(), BusError> {
        if let Some(source) = priority_source(offset, size) {
            let next = write_le_field(
                u64::from(self.priorities[source]),
                offset - (Self::PRIORITY_BASE + source as u64 * Self::REGISTER_SIZE),
                size,
                value,
            );
            if source != 0 {
                self.priorities[source] = (next as u32) & 0x7;
            }
            return Ok(());
        }
        if let Some(context) = context_enable(offset, size) {
            let next = write_le_field(
                u64::from(self.enables[context]),
                offset - enable_register(context),
                size,
                value,
            );
            self.enables[context] = (next as u32) & Self::UART_SOURCE_BIT;
            return Ok(());
        }
        if let Some(context) = context_threshold(offset, size) {
            let next = write_le_field(
                u64::from(self.thresholds[context]),
                offset - context_register(context, Self::THRESHOLD_OFFSET),
                size,
                value,
            );
            self.thresholds[context] = (next as u32) & 0x7;
            return Ok(());
        }
        if context_claim_complete(offset, size).is_some() {
            let source = write_le_field(0, offset & 0b11, size, value) as u32;
            self.complete(source);
            return Ok(());
        }
        Err(BusError::Unmapped {
            address: PLIC_START + offset,
            size,
        })
    }

    fn peek_claim(&self, context: usize, uart: &Uart) -> u32 {
        self.pending_source(uart)
            .filter(|source| self.source_enabled(context, *source))
            .unwrap_or(0)
    }

    fn complete(&mut self, source: u32) {
        if source == Self::UART_SOURCE {
            self.uart_claimed = false;
        }
    }

    fn claim(&mut self, context: usize, uart: &Uart) -> u32 {
        if self
            .pending_source(uart)
            .is_some_and(|source| self.source_enabled(context, source))
        {
            self.uart_claimed = true;
            Self::UART_SOURCE
        } else {
            0
        }
    }

    fn pending_bits(&self, uart: &Uart) -> u32 {
        self.pending_source(uart)
            .map(|_| Self::UART_SOURCE_BIT)
            .unwrap_or(0)
    }

    fn pending_source(&self, uart: &Uart) -> Option<u32> {
        if !self.uart_claimed
            && self.priorities[Self::UART_SOURCE as usize] > 0
            && uart.receive_interrupt_pending()
        {
            Some(Self::UART_SOURCE)
        } else {
            None
        }
    }

    fn source_enabled(&self, context: usize, source: u32) -> bool {
        self.enables[context] & (1 << source) != 0
            && self.priorities[source as usize] > self.thresholds[context]
    }
}

fn priority_source(offset: u64, size: usize) -> Option<usize> {
    (0..=Plic::SOURCE_COUNT).find(|source| {
        register_contains(
            offset,
            size,
            Plic::PRIORITY_BASE + *source as u64 * Plic::REGISTER_SIZE,
            Plic::REGISTER_SIZE,
        )
    })
}

impl Uart {
    const RECEIVER_BUFFER_OR_TRANSMIT_HOLDING: u64 = 0;
    const INTERRUPT_ENABLE: u64 = 1;
    const INTERRUPT_IDENTIFICATION: u64 = 2;
    const LINE_CONTROL: u64 = 3;
    const LINE_STATUS: u64 = 5;
    const LINE_CONTROL_DIVISOR_LATCH_ACCESS: u8 = 1 << 7;
    const INTERRUPT_ENABLE_RECEIVED_DATA_AVAILABLE: u8 = 1;
    const INTERRUPT_IDENTIFICATION_NO_INTERRUPT: u8 = 1;
    const INTERRUPT_IDENTIFICATION_RECEIVED_DATA_AVAILABLE: u8 = 0x04;
    const LINE_STATUS_DATA_READY: u8 = 1;
    const LINE_STATUS_TRANSMIT_HOLDING_EMPTY: u8 = 1 << 5;
    const LINE_STATUS_TRANSMITTER_EMPTY: u8 = 1 << 6;

    fn read(&mut self, offset: u64) -> u8 {
        match offset {
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING if self.divisor_latch_access() => {
                self.divisor_latch as u8
            }
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => {
                let value = self.input.pop_front();
                self.waiting_for_input = value.is_none();
                value.unwrap_or(0)
            }
            Self::INTERRUPT_ENABLE if self.divisor_latch_access() => {
                (self.divisor_latch >> 8) as u8
            }
            Self::INTERRUPT_ENABLE => self.interrupt_enable,
            Self::INTERRUPT_IDENTIFICATION => self.interrupt_identification(),
            Self::LINE_CONTROL => self.line_control,
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
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING if self.divisor_latch_access() => {
                self.divisor_latch as u8
            }
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => self.input.front().copied().unwrap_or(0),
            Self::INTERRUPT_ENABLE if self.divisor_latch_access() => {
                (self.divisor_latch >> 8) as u8
            }
            Self::INTERRUPT_ENABLE => self.interrupt_enable,
            Self::INTERRUPT_IDENTIFICATION => self.interrupt_identification(),
            Self::LINE_CONTROL => self.line_control,
            Self::LINE_STATUS => Self::line_status(!self.input.is_empty()),
            _ => 0,
        }
    }

    fn receive_interrupt_pending(&self) -> bool {
        self.interrupt_enable & Self::INTERRUPT_ENABLE_RECEIVED_DATA_AVAILABLE != 0
            && !self.input.is_empty()
    }

    fn interrupt_identification(&self) -> u8 {
        if self.receive_interrupt_pending() {
            Self::INTERRUPT_IDENTIFICATION_RECEIVED_DATA_AVAILABLE
        } else {
            Self::INTERRUPT_IDENTIFICATION_NO_INTERRUPT
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
        match offset {
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING if self.divisor_latch_access() => {
                self.divisor_latch = (self.divisor_latch & 0xff00) | u16::from(value);
            }
            Self::RECEIVER_BUFFER_OR_TRANSMIT_HOLDING => self.output.push(value),
            Self::INTERRUPT_ENABLE if self.divisor_latch_access() => {
                self.divisor_latch = (self.divisor_latch & 0x00ff) | (u16::from(value) << 8);
            }
            Self::INTERRUPT_ENABLE => {
                self.interrupt_enable = value & Self::INTERRUPT_ENABLE_RECEIVED_DATA_AVAILABLE;
            }
            Self::LINE_CONTROL => self.line_control = value,
            _ => {}
        }
    }

    fn divisor_latch_access(&self) -> bool {
        self.line_control & Self::LINE_CONTROL_DIVISOR_LATCH_ACCESS != 0
    }
}

fn context_enable(offset: u64, size: usize) -> Option<usize> {
    (0..Plic::CONTEXT_COUNT).find(|context| {
        register_contains(offset, size, enable_register(*context), Plic::REGISTER_SIZE)
    })
}

fn context_threshold(offset: u64, size: usize) -> Option<usize> {
    (0..Plic::CONTEXT_COUNT).find(|context| {
        register_contains(
            offset,
            size,
            context_register(*context, Plic::THRESHOLD_OFFSET),
            Plic::REGISTER_SIZE,
        )
    })
}

fn context_claim_complete(offset: u64, size: usize) -> Option<usize> {
    (0..Plic::CONTEXT_COUNT).find(|context| {
        register_contains(
            offset,
            size,
            context_register(*context, Plic::CLAIM_COMPLETE_OFFSET),
            Plic::REGISTER_SIZE,
        )
    })
}

fn enable_register(context: usize) -> u64 {
    Plic::ENABLE_BASE + context as u64 * Plic::ENABLE_STRIDE
}

fn context_register(context: usize, register: u64) -> u64 {
    Plic::CONTEXT_BASE + context as u64 * Plic::CONTEXT_STRIDE + register
}

fn clint_offset(address: u64, size: usize) -> Option<u64> {
    let end = address.checked_add(size as u64)?;
    if address >= CLINT_START && end <= CLINT_START + CLINT_SIZE {
        Some(address - CLINT_START)
    } else {
        None
    }
}

fn plic_offset(address: u64, size: usize) -> Option<u64> {
    let end = address.checked_add(size as u64)?;
    if address >= PLIC_START && end <= PLIC_START + PLIC_SIZE {
        Some(address - PLIC_START)
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
    fn plic_routes_uart_receive_interrupt_to_machine_context() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"A");
        bus.write_u8(UART_START + 1, 1).unwrap();
        assert!(!bus.machine_external_interrupt_pending());

        bus.write_u32(PLIC_START + 10 * 4, 1).unwrap();
        bus.write_u32(PLIC_START + 0x2000, 1 << 10).unwrap();
        bus.write_u32(PLIC_START + 0x20_0000, 0).unwrap();

        assert!(bus.machine_external_interrupt_pending());
        assert_eq!(bus.read_u32(PLIC_START + 0x1000).unwrap(), 1 << 10);
        assert_eq!(bus.peek_u32(PLIC_START + 0x20_0004).unwrap(), 10);
        assert_eq!(bus.read_u32(PLIC_START + 0x20_0004).unwrap(), 10);
        assert!(!bus.machine_external_interrupt_pending());

        assert_eq!(bus.read_u8(UART_START).unwrap(), b'A');
        bus.write_u32(PLIC_START + 0x20_0004, 10).unwrap();
        assert!(!bus.machine_external_interrupt_pending());
    }

    #[test]
    fn plic_routes_uart_receive_interrupt_to_supervisor_context() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"B");
        bus.write_u8(UART_START + 1, 1).unwrap();
        bus.write_u32(PLIC_START + 10 * 4, 2).unwrap();
        bus.write_u32(PLIC_START + 0x2080, 1 << 10).unwrap();
        bus.write_u32(PLIC_START + 0x20_1000, 1).unwrap();

        assert!(bus.supervisor_external_interrupt_pending());
        assert!(!bus.machine_external_interrupt_pending());
        assert_eq!(bus.read_u32(PLIC_START + 0x20_1004).unwrap(), 10);
    }

    #[test]
    fn plic_exposes_all_advertised_priority_registers() {
        let mut bus = Bus::new(16);
        for source in 1..=Plic::SOURCE_COUNT {
            let address = PLIC_START + source as u64 * Plic::REGISTER_SIZE;
            bus.write_u32(address, source as u32).unwrap();
            assert_eq!(bus.read_u32(address), Ok((source as u32) & 0x7));
        }
        bus.write_u32(PLIC_START, 7).unwrap();
        assert_eq!(bus.read_u32(PLIC_START), Ok(0));
    }

    #[test]
    fn uart_interrupt_identification_tracks_receive_interrupt_enable() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"C");
        assert_eq!(bus.read_u8(UART_START + 2), Ok(1));
        bus.write_u8(UART_START + 1, 1).unwrap();
        assert_eq!(bus.read_u8(UART_START + 1), Ok(1));
        assert_eq!(bus.read_u8(UART_START + 2), Ok(4));
        assert_eq!(bus.read_u8(UART_START), Ok(b'C'));
        assert_eq!(bus.read_u8(UART_START + 2), Ok(1));
    }

    #[test]
    fn uart_transmit_holding_register_captures_output() {
        let mut bus = Bus::new(16);
        bus.write_u8(UART_START, b'O').unwrap();
        bus.write_u8(UART_START, b'K').unwrap();
        assert_eq!(bus.uart_output(), b"OK");
    }

    #[test]
    fn uart_divisor_latch_does_not_emit_output() {
        let mut bus = Bus::new(16);
        bus.write_u8(UART_START + 3, 0x80).unwrap();
        bus.write_u8(UART_START, 2).unwrap();
        bus.write_u8(UART_START + 1, 1).unwrap();
        assert_eq!(bus.read_u8(UART_START), Ok(2));
        assert_eq!(bus.read_u8(UART_START + 1), Ok(1));
        assert!(bus.uart_output().is_empty());

        bus.write_u8(UART_START + 3, 3).unwrap();
        bus.write_u8(UART_START, b'X').unwrap();
        assert_eq!(bus.uart_output(), b"X");
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
    fn uart_supports_wide_accesses() {
        let mut bus = Bus::new(16);
        bus.push_uart_input(b"AB");
        // 16-bit read at the receive buffer pops one byte per byte lane.
        assert_eq!(bus.read_u16(UART_START), Ok(u16::from_le_bytes([b'A', 0])));
        // 32-bit write to the transmit register emits the low byte.
        bus.write_u32(UART_START, u32::from(b'X')).unwrap();
        assert_eq!(&bus.uart_output()[bus.uart_output().len() - 1..], b"X");
        // Wide reads covering the line-status register report data-ready.
        let status = bus.peek_u64(UART_START).unwrap();
        assert_ne!((status >> 40) & 1, 0); // LSR data ready for pending 'B'
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
