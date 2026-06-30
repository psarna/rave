use crate::{Bus, BusError};
use std::fmt;

const REGISTER_COUNT: usize = 32;
const ZERO_REGISTER: usize = 0;
const RETURN_VALUE_REGISTER: usize = 10;
const COMPRESSED_INSTRUCTION_SIZE: u64 = 2;
const INSTRUCTION_SIZE: u64 = 4;
const INSTRUCTION_BITS: u32 = 32;
const XLEN_BITS: u32 = 64;

const RD_SHIFT: u32 = 7;
const RS1_SHIFT: u32 = 15;
const RS2_SHIFT: u32 = 20;
const FUNCT3_SHIFT: u32 = 12;
const FUNCT7_SHIFT: u32 = 25;
const CSR_SHIFT: u32 = 20;
const SHIFT_PREFIX_SHIFT: u32 = 26;
const REGISTER_FIELD_BITS: u32 = 5;
const CSR_BITS: u32 = 12;
const FUNCT3_BITS: u32 = 3;
const FUNCT7_BITS: u32 = 7;

const I_IMMEDIATE_SHIFT: u32 = 20;
const I_IMMEDIATE_BITS: u32 = 12;
const S_IMMEDIATE_LOW_SHIFT: u32 = 7;
const S_IMMEDIATE_HIGH_SHIFT: u32 = 25;
const S_IMMEDIATE_LOW_BITS: u32 = 5;
const S_IMMEDIATE_HIGH_BITS: u32 = 7;
const B_IMMEDIATE_BITS: u32 = 13;
const J_IMMEDIATE_BITS: u32 = 21;
const U_IMMEDIATE_BITS: u32 = 32;

const OPCODE_MASK: u32 = 0x7f;
const OPCODE_LOAD: u32 = 0x03;
const OPCODE_MISC_MEM: u32 = 0x0f;
const OPCODE_OP_IMM: u32 = 0x13;
const OPCODE_AUIPC: u32 = 0x17;
const OPCODE_OP_IMM_32: u32 = 0x1b;
const OPCODE_STORE: u32 = 0x23;
const OPCODE_AMO: u32 = 0x2f;
const OPCODE_OP: u32 = 0x33;
const OPCODE_LUI: u32 = 0x37;
const OPCODE_OP_32: u32 = 0x3b;
const OPCODE_BRANCH: u32 = 0x63;
const OPCODE_JALR: u32 = 0x67;
const OPCODE_JAL: u32 = 0x6f;
const OPCODE_SYSTEM: u32 = 0x73;

const FUNCT_ADD: u32 = 0;
const FUNCT_SHIFT_LEFT: u32 = 1;
const FUNCT_SET_LESS_THAN: u32 = 2;
const FUNCT_SET_LESS_THAN_UNSIGNED: u32 = 3;
const FUNCT_XOR: u32 = 4;
const FUNCT_SHIFT_RIGHT: u32 = 5;
const FUNCT_OR: u32 = 6;
const FUNCT_AND: u32 = 7;

const FUNCT_BRANCH_EQUAL: u32 = 0;
const FUNCT_BRANCH_NOT_EQUAL: u32 = 1;
const FUNCT_BRANCH_LESS_THAN: u32 = 4;
const FUNCT_BRANCH_GREATER_OR_EQUAL: u32 = 5;
const FUNCT_BRANCH_LESS_THAN_UNSIGNED: u32 = 6;
const FUNCT_BRANCH_GREATER_OR_EQUAL_UNSIGNED: u32 = 7;

const FUNCT_LOAD_BYTE: u32 = 0;
const FUNCT_LOAD_HALF: u32 = 1;
const FUNCT_LOAD_WORD: u32 = 2;
const FUNCT_LOAD_DOUBLE: u32 = 3;
const FUNCT_LOAD_BYTE_UNSIGNED: u32 = 4;
const FUNCT_LOAD_HALF_UNSIGNED: u32 = 5;
const FUNCT_LOAD_WORD_UNSIGNED: u32 = 6;

const FUNCT_STORE_BYTE: u32 = 0;
const FUNCT_STORE_HALF: u32 = 1;
const FUNCT_STORE_WORD: u32 = 2;
const FUNCT_STORE_DOUBLE: u32 = 3;

const FUNCT_AMO_WORD: u32 = 2;
const FUNCT_AMO_DOUBLE: u32 = 3;

const FUNCT_SYSTEM_PRIVILEGED: u32 = 0;
const FUNCT_CSRRW: u32 = 1;
const FUNCT_CSRRS: u32 = 2;
const FUNCT_CSRRC: u32 = 3;
const FUNCT_CSRRWI: u32 = 5;
const FUNCT_CSRRSI: u32 = 6;
const FUNCT_CSRRCI: u32 = 7;

const FUNCT7_BASE: u32 = 0;
const FUNCT7_MULTIPLY: u32 = 1;
const FUNCT7_ALTERNATE: u32 = 0x20;
const AMO_FUNCT_LOAD_RESERVED: u32 = 0b00010;
const AMO_FUNCT_STORE_CONDITIONAL: u32 = 0b00011;
const AMO_FUNCT_SWAP: u32 = 0b00001;
const AMO_FUNCT_ADD: u32 = 0b00000;
const AMO_FUNCT_XOR: u32 = 0b00100;
const AMO_FUNCT_AND: u32 = 0b01100;
const AMO_FUNCT_OR: u32 = 0b01000;
const AMO_FUNCT_MIN: u32 = 0b10000;
const AMO_FUNCT_MAX: u32 = 0b10100;
const AMO_FUNCT_MIN_UNSIGNED: u32 = 0b11000;
const AMO_FUNCT_MAX_UNSIGNED: u32 = 0b11100;
const AMO_FUNCT_SHIFT: u32 = 27;
const AMO_FUNCT_BITS: u32 = 5;
const SHIFT64_LOGICAL_PREFIX: u32 = 0;
const SHIFT64_ARITHMETIC_PREFIX: u32 = 0x10;
const SHIFT64_MASK: u32 = 0x3f;
const SHIFT32_MASK: u32 = 0x1f;

const INSTRUCTION_ECALL: u32 = 0x0000_0073;
const INSTRUCTION_EBREAK: u32 = 0x0010_0073;
const INSTRUCTION_FENCE_I: u32 = 0x0000_100f;
const UPPER_IMMEDIATE_MASK: u32 = 0xffff_f000;
const JALR_ALIGNMENT_MASK: u64 = !1;

const CSR_MVENDORID: u16 = 0xf11;
const CSR_MARCHID: u16 = 0xf12;
const CSR_MIMPID: u16 = 0xf13;
const CSR_MHARTID: u16 = 0xf14;
const CSR_MSTATUS: u16 = 0x300;
const CSR_MISA: u16 = 0x301;
const CSR_MIE: u16 = 0x304;
const CSR_MTVEC: u16 = 0x305;
const CSR_MSCRATCH: u16 = 0x340;
const CSR_MEPC: u16 = 0x341;
const CSR_MCAUSE: u16 = 0x342;
const CSR_MTVAL: u16 = 0x343;
const CSR_MIP: u16 = 0x344;
const CSR_CYCLE: u16 = 0xc00;
const CSR_TIME: u16 = 0xc01;
const CSR_INSTRET: u16 = 0xc02;

const MSTATUS_WRITABLE_MASK: u64 = (1 << 3) | (1 << 7) | (0b11 << 11);
const MIE_WRITABLE_MASK: u64 = (1 << 3) | (1 << 7) | (1 << 11);
const MIP_WRITABLE_MASK: u64 = 0;
const MTVEC_WRITABLE_MASK: u64 = !0b10;
const MEPC_WRITABLE_MASK: u64 = !1;
const MCAUSE_WRITABLE_MASK: u64 = u64::MAX;
const MTVAL_WRITABLE_MASK: u64 = u64::MAX;
const MSCRATCH_WRITABLE_MASK: u64 = u64::MAX;
const MISA_VALUE: u64 = (2 << 62) | (1 << 0) | (1 << 2) | (1 << 8) | (1 << 12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltReason {
    Breakpoint { code: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    Bus(BusError),
    IllegalInstruction { pc: u64, instruction: u32 },
    EnvironmentCall { pc: u64 },
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::IllegalInstruction { pc, instruction } => {
                write!(f, "illegal instruction {instruction:#010x} at {pc:#018x}")
            }
            Self::EnvironmentCall { pc } => write!(f, "ecall at {pc:#018x}"),
        }
    }
}

impl std::error::Error for StepError {}

impl From<BusError> for StepError {
    fn from(value: BusError) -> Self {
        Self::Bus(value)
    }
}

#[derive(Debug, Clone)]
pub struct Cpu {
    registers: [u64; REGISTER_COUNT],
    csrs: CsrFile,
    reservation: Option<u64>,
    pub pc: u64,
}

#[derive(Debug, Clone, Default)]
struct CsrFile {
    mstatus: u64,
    mie: u64,
    mtvec: u64,
    mscratch: u64,
    mepc: u64,
    mcause: u64,
    mtval: u64,
    mip: u64,
    cycle: u64,
    instret: u64,
}

#[derive(Clone, Copy)]
struct Decoded {
    raw: u32,
    opcode: u32,
    rd: usize,
    funct3: u32,
    rs1: usize,
    rs2: usize,
    funct7: u32,
}

impl Decoded {
    fn new(raw: u32) -> Self {
        Self {
            raw,
            opcode: raw & OPCODE_MASK,
            rd: register_field(raw, RD_SHIFT),
            funct3: bits(raw, FUNCT3_SHIFT, FUNCT3_BITS),
            rs1: register_field(raw, RS1_SHIFT),
            rs2: register_field(raw, RS2_SHIFT),
            funct7: bits(raw, FUNCT7_SHIFT, FUNCT7_BITS),
        }
    }
}

#[derive(Clone, Copy)]
struct Fetched {
    raw: u32,
    size: u64,
}

enum Execution {
    Continue { next_pc: u64 },
    Halt(HaltReason),
}

impl Cpu {
    pub fn new(pc: u64) -> Self {
        Self {
            registers: [0; REGISTER_COUNT],
            csrs: CsrFile::default(),
            reservation: None,
            pc,
        }
    }

    pub fn register(&self, index: usize) -> u64 {
        self.registers[index]
    }

    pub fn set_register(&mut self, index: usize, value: u64) {
        if index != ZERO_REGISTER {
            self.registers[index] = value;
        }
    }

    pub fn csr(&self, address: u16) -> u64 {
        self.csrs.read(address).unwrap_or(0)
    }

    pub fn reservation_matches(&self, address: u64) -> bool {
        self.reservation == Some(address)
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<Option<HaltReason>, StepError> {
        let pc = self.pc;
        let fetched = fetch_instruction(bus, pc)?;
        let instruction = Decoded::new(fetched.raw);
        let sequential_pc = pc.wrapping_add(fetched.size);

        let execution = match instruction.opcode {
            OPCODE_LUI => self.execute_lui(instruction, sequential_pc),
            OPCODE_AUIPC => self.execute_auipc(instruction, pc, sequential_pc),
            OPCODE_JAL => self.execute_jal(instruction, pc, sequential_pc),
            OPCODE_JALR => self.execute_jalr(instruction, sequential_pc)?,
            OPCODE_BRANCH => self.execute_branch(instruction, pc, sequential_pc)?,
            OPCODE_LOAD => self.execute_load(instruction, bus, sequential_pc)?,
            OPCODE_STORE => self.execute_store(instruction, bus, sequential_pc)?,
            OPCODE_AMO => self.execute_amo(instruction, bus, sequential_pc)?,
            OPCODE_OP_IMM => self.execute_op_immediate(instruction, sequential_pc)?,
            OPCODE_OP_IMM_32 => self.execute_op_immediate_word(instruction, sequential_pc)?,
            OPCODE_OP => self.execute_op(instruction, sequential_pc)?,
            OPCODE_OP_32 => self.execute_op_word(instruction, sequential_pc)?,
            OPCODE_MISC_MEM => self.execute_misc_mem(instruction, sequential_pc)?,
            OPCODE_SYSTEM => self.execute_system(instruction, pc, sequential_pc)?,
            _ => return Err(illegal(pc, instruction.raw)),
        };

        match execution {
            Execution::Continue { next_pc } => {
                self.csrs.retire_instruction();
                self.pc = next_pc;
                self.registers[ZERO_REGISTER] = 0;
                Ok(None)
            }
            Execution::Halt(reason) => Ok(Some(reason)),
        }
    }

    fn execute_lui(&mut self, instruction: Decoded, next_pc: u64) -> Execution {
        self.set_register(instruction.rd, upper_immediate(instruction.raw));
        Execution::Continue { next_pc }
    }

    fn execute_auipc(&mut self, instruction: Decoded, pc: u64, next_pc: u64) -> Execution {
        self.set_register(
            instruction.rd,
            pc.wrapping_add(upper_immediate(instruction.raw)),
        );
        Execution::Continue { next_pc }
    }

    fn execute_jal(&mut self, instruction: Decoded, pc: u64, return_pc: u64) -> Execution {
        self.set_register(instruction.rd, return_pc);
        Execution::Continue {
            next_pc: pc.wrapping_add(j_immediate(instruction.raw) as u64),
        }
    }

    fn execute_jalr(
        &mut self,
        instruction: Decoded,
        return_pc: u64,
    ) -> Result<Execution, StepError> {
        if instruction.funct3 != FUNCT_ADD {
            return Err(illegal(self.pc, instruction.raw));
        }
        let target = self.registers[instruction.rs1]
            .wrapping_add(i_immediate(instruction.raw) as u64)
            & JALR_ALIGNMENT_MASK;
        self.set_register(instruction.rd, return_pc);
        Ok(Execution::Continue { next_pc: target })
    }

    fn execute_branch(
        &self,
        instruction: Decoded,
        pc: u64,
        sequential_pc: u64,
    ) -> Result<Execution, StepError> {
        let lhs = self.registers[instruction.rs1];
        let rhs = self.registers[instruction.rs2];
        let taken = match instruction.funct3 {
            FUNCT_BRANCH_EQUAL => lhs == rhs,
            FUNCT_BRANCH_NOT_EQUAL => lhs != rhs,
            FUNCT_BRANCH_LESS_THAN => (lhs as i64) < (rhs as i64),
            FUNCT_BRANCH_GREATER_OR_EQUAL => (lhs as i64) >= (rhs as i64),
            FUNCT_BRANCH_LESS_THAN_UNSIGNED => lhs < rhs,
            FUNCT_BRANCH_GREATER_OR_EQUAL_UNSIGNED => lhs >= rhs,
            _ => return Err(illegal(pc, instruction.raw)),
        };
        let next_pc = if taken {
            pc.wrapping_add(b_immediate(instruction.raw) as u64)
        } else {
            sequential_pc
        };
        Ok(Execution::Continue { next_pc })
    }

    fn execute_load(
        &mut self,
        instruction: Decoded,
        bus: &Bus,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address = self.effective_i_address(instruction);
        let value = match instruction.funct3 {
            FUNCT_LOAD_BYTE => bus.read_u8(address)? as i8 as i64 as u64,
            FUNCT_LOAD_HALF => bus.read_u16(address)? as i16 as i64 as u64,
            FUNCT_LOAD_WORD => bus.read_u32(address)? as i32 as i64 as u64,
            FUNCT_LOAD_DOUBLE => bus.read_u64(address)?,
            FUNCT_LOAD_BYTE_UNSIGNED => bus.read_u8(address)? as u64,
            FUNCT_LOAD_HALF_UNSIGNED => bus.read_u16(address)? as u64,
            FUNCT_LOAD_WORD_UNSIGNED => bus.read_u32(address)? as u64,
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, value);
        Ok(Execution::Continue { next_pc })
    }

    fn execute_store(
        &mut self,
        instruction: Decoded,
        bus: &mut Bus,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address =
            self.registers[instruction.rs1].wrapping_add(s_immediate(instruction.raw) as u64);
        let value = self.registers[instruction.rs2];
        match instruction.funct3 {
            FUNCT_STORE_BYTE => bus.write_u8(address, value as u8)?,
            FUNCT_STORE_HALF => bus.write_u16(address, value as u16)?,
            FUNCT_STORE_WORD => bus.write_u32(address, value as u32)?,
            FUNCT_STORE_DOUBLE => bus.write_u64(address, value)?,
            _ => return Err(illegal(self.pc, instruction.raw)),
        }
        self.clear_reservation_for_store(address);
        Ok(Execution::Continue { next_pc })
    }

    fn execute_amo(
        &mut self,
        instruction: Decoded,
        bus: &mut Bus,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address = self.registers[instruction.rs1];
        let funct5 = bits(instruction.raw, AMO_FUNCT_SHIFT, AMO_FUNCT_BITS);
        match instruction.funct3 {
            FUNCT_AMO_WORD => self.execute_amo_word(instruction, bus, address, funct5)?,
            FUNCT_AMO_DOUBLE => self.execute_amo_double(instruction, bus, address, funct5)?,
            _ => return Err(illegal(self.pc, instruction.raw)),
        }
        Ok(Execution::Continue { next_pc })
    }

    fn execute_amo_word(
        &mut self,
        instruction: Decoded,
        bus: &mut Bus,
        address: u64,
        funct5: u32,
    ) -> Result<(), StepError> {
        if funct5 == AMO_FUNCT_LOAD_RESERVED {
            if instruction.rs2 != ZERO_REGISTER {
                return Err(illegal(self.pc, instruction.raw));
            }
            let old = bus.read_u32(address)?;
            self.reservation = Some(address);
            self.set_register(instruction.rd, sign_extend_word(old));
            return Ok(());
        }

        if funct5 == AMO_FUNCT_STORE_CONDITIONAL {
            let success = self.reservation == Some(address);
            self.reservation = None;
            if success {
                bus.write_u32(address, self.registers[instruction.rs2] as u32)?;
            }
            self.set_register(instruction.rd, (!success) as u64);
            return Ok(());
        }

        let old = bus.read_u32(address)?;
        let rhs = self.registers[instruction.rs2] as u32;
        let new = match funct5 {
            AMO_FUNCT_SWAP => rhs,
            AMO_FUNCT_ADD => old.wrapping_add(rhs),
            AMO_FUNCT_XOR => old ^ rhs,
            AMO_FUNCT_AND => old & rhs,
            AMO_FUNCT_OR => old | rhs,
            AMO_FUNCT_MIN => ((old as i32).min(rhs as i32)) as u32,
            AMO_FUNCT_MAX => ((old as i32).max(rhs as i32)) as u32,
            AMO_FUNCT_MIN_UNSIGNED => old.min(rhs),
            AMO_FUNCT_MAX_UNSIGNED => old.max(rhs),
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        bus.write_u32(address, new)?;
        self.clear_reservation_for_store(address);
        self.set_register(instruction.rd, sign_extend_word(old));
        Ok(())
    }

    fn execute_amo_double(
        &mut self,
        instruction: Decoded,
        bus: &mut Bus,
        address: u64,
        funct5: u32,
    ) -> Result<(), StepError> {
        if funct5 == AMO_FUNCT_LOAD_RESERVED {
            if instruction.rs2 != ZERO_REGISTER {
                return Err(illegal(self.pc, instruction.raw));
            }
            let old = bus.read_u64(address)?;
            self.reservation = Some(address);
            self.set_register(instruction.rd, old);
            return Ok(());
        }

        if funct5 == AMO_FUNCT_STORE_CONDITIONAL {
            let success = self.reservation == Some(address);
            self.reservation = None;
            if success {
                bus.write_u64(address, self.registers[instruction.rs2])?;
            }
            self.set_register(instruction.rd, (!success) as u64);
            return Ok(());
        }

        let old = bus.read_u64(address)?;
        let rhs = self.registers[instruction.rs2];
        let new = match funct5 {
            AMO_FUNCT_SWAP => rhs,
            AMO_FUNCT_ADD => old.wrapping_add(rhs),
            AMO_FUNCT_XOR => old ^ rhs,
            AMO_FUNCT_AND => old & rhs,
            AMO_FUNCT_OR => old | rhs,
            AMO_FUNCT_MIN => ((old as i64).min(rhs as i64)) as u64,
            AMO_FUNCT_MAX => ((old as i64).max(rhs as i64)) as u64,
            AMO_FUNCT_MIN_UNSIGNED => old.min(rhs),
            AMO_FUNCT_MAX_UNSIGNED => old.max(rhs),
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        bus.write_u64(address, new)?;
        self.clear_reservation_for_store(address);
        self.set_register(instruction.rd, old);
        Ok(())
    }

    fn execute_op_immediate(
        &mut self,
        instruction: Decoded,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let lhs = self.registers[instruction.rs1];
        let immediate = i_immediate(instruction.raw);
        let shift = bits(instruction.raw, I_IMMEDIATE_SHIFT, REGISTER_FIELD_BITS + 1);
        let value = match instruction.funct3 {
            FUNCT_ADD => lhs.wrapping_add(immediate as u64),
            FUNCT_SET_LESS_THAN => ((lhs as i64) < immediate) as u64,
            FUNCT_SET_LESS_THAN_UNSIGNED => (lhs < immediate as u64) as u64,
            FUNCT_XOR => lhs ^ immediate as u64,
            FUNCT_OR => lhs | immediate as u64,
            FUNCT_AND => lhs & immediate as u64,
            FUNCT_SHIFT_LEFT if instruction.raw >> SHIFT_PREFIX_SHIFT == SHIFT64_LOGICAL_PREFIX => {
                lhs << shift
            }
            FUNCT_SHIFT_RIGHT
                if instruction.raw >> SHIFT_PREFIX_SHIFT == SHIFT64_LOGICAL_PREFIX =>
            {
                lhs >> shift
            }
            FUNCT_SHIFT_RIGHT
                if instruction.raw >> SHIFT_PREFIX_SHIFT == SHIFT64_ARITHMETIC_PREFIX =>
            {
                ((lhs as i64) >> shift) as u64
            }
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, value);
        Ok(Execution::Continue { next_pc })
    }

    fn execute_op_immediate_word(
        &mut self,
        instruction: Decoded,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let lhs = self.registers[instruction.rs1];
        let shift = bits(instruction.raw, I_IMMEDIATE_SHIFT, REGISTER_FIELD_BITS);
        let word = match (instruction.funct3, instruction.funct7) {
            (FUNCT_ADD, _) => lhs.wrapping_add(i_immediate(instruction.raw) as u64) as u32,
            (FUNCT_SHIFT_LEFT, FUNCT7_BASE) => (lhs as u32) << shift,
            (FUNCT_SHIFT_RIGHT, FUNCT7_BASE) => (lhs as u32) >> shift,
            (FUNCT_SHIFT_RIGHT, FUNCT7_ALTERNATE) => ((lhs as i32) >> shift) as u32,
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, sign_extend_word(word));
        Ok(Execution::Continue { next_pc })
    }

    fn execute_op(&mut self, instruction: Decoded, next_pc: u64) -> Result<Execution, StepError> {
        let lhs = self.registers[instruction.rs1];
        let rhs = self.registers[instruction.rs2];
        let value = match (instruction.funct3, instruction.funct7) {
            (FUNCT_ADD, FUNCT7_BASE) => lhs.wrapping_add(rhs),
            (FUNCT_ADD, FUNCT7_ALTERNATE) => lhs.wrapping_sub(rhs),
            (FUNCT_ADD, FUNCT7_MULTIPLY) => lhs.wrapping_mul(rhs),
            (FUNCT_SHIFT_LEFT, FUNCT7_BASE) => lhs << (rhs & u64::from(SHIFT64_MASK)),
            (FUNCT_SHIFT_LEFT, FUNCT7_MULTIPLY) => mulh(lhs, rhs),
            (FUNCT_SET_LESS_THAN, FUNCT7_BASE) => ((lhs as i64) < (rhs as i64)) as u64,
            (FUNCT_SET_LESS_THAN, FUNCT7_MULTIPLY) => mulhsu(lhs, rhs),
            (FUNCT_SET_LESS_THAN_UNSIGNED, FUNCT7_BASE) => (lhs < rhs) as u64,
            (FUNCT_SET_LESS_THAN_UNSIGNED, FUNCT7_MULTIPLY) => mulhu(lhs, rhs),
            (FUNCT_XOR, FUNCT7_BASE) => lhs ^ rhs,
            (FUNCT_XOR, FUNCT7_MULTIPLY) => div(lhs, rhs),
            (FUNCT_SHIFT_RIGHT, FUNCT7_BASE) => lhs >> (rhs & u64::from(SHIFT64_MASK)),
            (FUNCT_SHIFT_RIGHT, FUNCT7_ALTERNATE) => {
                ((lhs as i64) >> (rhs & u64::from(SHIFT64_MASK))) as u64
            }
            (FUNCT_SHIFT_RIGHT, FUNCT7_MULTIPLY) => divu(lhs, rhs),
            (FUNCT_OR, FUNCT7_BASE) => lhs | rhs,
            (FUNCT_OR, FUNCT7_MULTIPLY) => rem(lhs, rhs),
            (FUNCT_AND, FUNCT7_BASE) => lhs & rhs,
            (FUNCT_AND, FUNCT7_MULTIPLY) => remu(lhs, rhs),
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, value);
        Ok(Execution::Continue { next_pc })
    }

    fn execute_op_word(
        &mut self,
        instruction: Decoded,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let lhs = self.registers[instruction.rs1] as u32;
        let rhs = self.registers[instruction.rs2];
        let word = match (instruction.funct3, instruction.funct7) {
            (FUNCT_ADD, FUNCT7_BASE) => lhs.wrapping_add(rhs as u32),
            (FUNCT_ADD, FUNCT7_ALTERNATE) => lhs.wrapping_sub(rhs as u32),
            (FUNCT_ADD, FUNCT7_MULTIPLY) => lhs.wrapping_mul(rhs as u32),
            (FUNCT_SHIFT_LEFT, FUNCT7_BASE) => lhs << (rhs & u64::from(SHIFT32_MASK)),
            (FUNCT_XOR, FUNCT7_MULTIPLY) => divw(lhs, rhs as u32),
            (FUNCT_SHIFT_RIGHT, FUNCT7_BASE) => lhs >> (rhs & u64::from(SHIFT32_MASK)),
            (FUNCT_SHIFT_RIGHT, FUNCT7_ALTERNATE) => {
                ((lhs as i32) >> (rhs & u64::from(SHIFT32_MASK))) as u32
            }
            (FUNCT_SHIFT_RIGHT, FUNCT7_MULTIPLY) => divuw(lhs, rhs as u32),
            (FUNCT_OR, FUNCT7_MULTIPLY) => remw(lhs, rhs as u32),
            (FUNCT_AND, FUNCT7_MULTIPLY) => remuw(lhs, rhs as u32),
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, sign_extend_word(word));
        Ok(Execution::Continue { next_pc })
    }

    fn execute_misc_mem(&self, instruction: Decoded, next_pc: u64) -> Result<Execution, StepError> {
        if instruction.funct3 == FUNCT_ADD || instruction.raw == INSTRUCTION_FENCE_I {
            Ok(Execution::Continue { next_pc })
        } else {
            Err(illegal(self.pc, instruction.raw))
        }
    }

    fn execute_system(
        &mut self,
        instruction: Decoded,
        pc: u64,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        match instruction.raw {
            INSTRUCTION_EBREAK => Ok(Execution::Halt(HaltReason::Breakpoint {
                code: self.registers[RETURN_VALUE_REGISTER],
            })),
            INSTRUCTION_ECALL => Err(StepError::EnvironmentCall { pc }),
            _ if instruction.funct3 == FUNCT_SYSTEM_PRIVILEGED => Err(illegal(pc, instruction.raw)),
            _ => self.execute_csr(instruction, pc, next_pc),
        }
    }

    fn execute_csr(
        &mut self,
        instruction: Decoded,
        pc: u64,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address = csr_address(instruction.raw);
        let old_value = self
            .csrs
            .read(address)
            .ok_or_else(|| illegal(pc, instruction.raw))?;
        let register_operand = self.registers[instruction.rs1];
        let immediate_operand = instruction.rs1 as u64;

        let (write, new_value) = match instruction.funct3 {
            FUNCT_CSRRW => (true, register_operand),
            FUNCT_CSRRS => (
                instruction.rs1 != ZERO_REGISTER,
                old_value | register_operand,
            ),
            FUNCT_CSRRC => (
                instruction.rs1 != ZERO_REGISTER,
                old_value & !register_operand,
            ),
            FUNCT_CSRRWI => (true, immediate_operand),
            FUNCT_CSRRSI => (
                instruction.rs1 != ZERO_REGISTER,
                old_value | immediate_operand,
            ),
            FUNCT_CSRRCI => (
                instruction.rs1 != ZERO_REGISTER,
                old_value & !immediate_operand,
            ),
            _ => return Err(illegal(pc, instruction.raw)),
        };

        if write {
            if self.csrs.is_read_only(address) {
                return Err(illegal(pc, instruction.raw));
            }
            self.csrs
                .write(address, new_value)
                .ok_or_else(|| illegal(pc, instruction.raw))?;
        }
        self.set_register(instruction.rd, old_value);
        Ok(Execution::Continue { next_pc })
    }

    fn effective_i_address(&self, instruction: Decoded) -> u64 {
        self.registers[instruction.rs1].wrapping_add(i_immediate(instruction.raw) as u64)
    }

    fn clear_reservation_for_store(&mut self, address: u64) {
        if self.reservation == Some(address) {
            self.reservation = None;
        }
    }
}

fn fetch_instruction(bus: &Bus, pc: u64) -> Result<Fetched, StepError> {
    let half = bus.read_u16(pc)?;
    if half & 0b11 == 0b11 {
        Ok(Fetched {
            raw: bus.read_u32(pc)?,
            size: INSTRUCTION_SIZE,
        })
    } else {
        let raw = decompress(half)
            .map_err(|instruction| StepError::IllegalInstruction { pc, instruction })?;
        Ok(Fetched {
            raw,
            size: COMPRESSED_INSTRUCTION_SIZE,
        })
    }
}

pub fn decode_compressed_instruction(instruction: u16) -> Option<u32> {
    decompress(instruction).ok()
}

pub fn encoded_instruction_size(first_half: u16) -> u64 {
    if first_half & 0b11 == 0b11 {
        INSTRUCTION_SIZE
    } else {
        COMPRESSED_INSTRUCTION_SIZE
    }
}

fn decompress(instruction: u16) -> Result<u32, u32> {
    let raw = u32::from(instruction);
    let quadrant = raw & 0b11;
    let funct3 = (raw >> 13) & 0b111;
    match (quadrant, funct3) {
        (0b00, 0b000) => {
            let immediate = c_addi4spn_immediate(raw);
            if immediate == 0 {
                Err(raw)
            } else {
                Ok(encode_i(
                    immediate,
                    2,
                    FUNCT_ADD,
                    compressed_rd(raw),
                    OPCODE_OP_IMM,
                ))
            }
        }
        (0b00, 0b010) => Ok(encode_i(
            c_lw_immediate(raw),
            compressed_rs1(raw),
            FUNCT_LOAD_WORD,
            compressed_rd(raw),
            OPCODE_LOAD,
        )),
        (0b00, 0b011) => Ok(encode_i(
            c_ld_immediate(raw),
            compressed_rs1(raw),
            FUNCT_LOAD_DOUBLE,
            compressed_rd(raw),
            OPCODE_LOAD,
        )),
        (0b00, 0b110) => Ok(encode_s(
            c_lw_immediate(raw),
            compressed_rs2(raw),
            compressed_rs1(raw),
            FUNCT_STORE_WORD,
        )),
        (0b00, 0b111) => Ok(encode_s(
            c_ld_immediate(raw),
            compressed_rs2(raw),
            compressed_rs1(raw),
            FUNCT_STORE_DOUBLE,
        )),
        (0b01, 0b000) => Ok(encode_i(
            c_i_immediate(raw) as u32,
            c_rd_rs1(raw),
            FUNCT_ADD,
            c_rd_rs1(raw),
            OPCODE_OP_IMM,
        )),
        (0b01, 0b001) => {
            let rd = c_rd_rs1(raw);
            if rd == 0 {
                Err(raw)
            } else {
                Ok(encode_i(
                    c_i_immediate(raw) as u32,
                    rd,
                    FUNCT_ADD,
                    rd,
                    OPCODE_OP_IMM_32,
                ))
            }
        }
        (0b01, 0b010) => Ok(encode_i(
            c_i_immediate(raw) as u32,
            0,
            FUNCT_ADD,
            c_rd_rs1(raw),
            OPCODE_OP_IMM,
        )),
        (0b01, 0b011) => decompress_lui_addi16sp(raw),
        (0b01, 0b100) => decompress_misc_alu(raw),
        (0b01, 0b101) => Ok(encode_j(c_j_immediate(raw) as u32, 0)),
        (0b01, 0b110) => Ok(encode_b(
            c_b_immediate(raw) as u32,
            0,
            compressed_rs1(raw),
            FUNCT_BRANCH_EQUAL,
        )),
        (0b01, 0b111) => Ok(encode_b(
            c_b_immediate(raw) as u32,
            0,
            compressed_rs1(raw),
            FUNCT_BRANCH_NOT_EQUAL,
        )),
        (0b10, 0b000) => {
            let rd = c_rd_rs1(raw);
            if rd == 0 {
                Err(raw)
            } else {
                Ok(encode_i(
                    c_shift_amount(raw),
                    rd,
                    FUNCT_SHIFT_LEFT,
                    rd,
                    OPCODE_OP_IMM,
                ))
            }
        }
        (0b10, 0b010) => {
            let rd = c_rd_rs1(raw);
            if rd == 0 {
                Err(raw)
            } else {
                Ok(encode_i(
                    c_lwsp_immediate(raw),
                    2,
                    FUNCT_LOAD_WORD,
                    rd,
                    OPCODE_LOAD,
                ))
            }
        }
        (0b10, 0b011) => {
            let rd = c_rd_rs1(raw);
            if rd == 0 {
                Err(raw)
            } else {
                Ok(encode_i(
                    c_ldsp_immediate(raw),
                    2,
                    FUNCT_LOAD_DOUBLE,
                    rd,
                    OPCODE_LOAD,
                ))
            }
        }
        (0b10, 0b100) => decompress_jr_mv_ebreak_jalr_add(raw),
        (0b10, 0b110) => Ok(encode_s(
            c_swsp_immediate(raw),
            c_rs2(raw),
            2,
            FUNCT_STORE_WORD,
        )),
        (0b10, 0b111) => Ok(encode_s(
            c_sdsp_immediate(raw),
            c_rs2(raw),
            2,
            FUNCT_STORE_DOUBLE,
        )),
        _ => Err(raw),
    }
}

fn decompress_lui_addi16sp(raw: u32) -> Result<u32, u32> {
    let rd = c_rd_rs1(raw);
    if rd == 2 {
        let immediate = c_addi16sp_immediate(raw);
        if immediate == 0 {
            Err(raw)
        } else {
            Ok(encode_i(immediate as u32, 2, FUNCT_ADD, 2, OPCODE_OP_IMM))
        }
    } else if rd == 0 {
        Err(raw)
    } else {
        let immediate = c_lui_immediate(raw);
        if immediate == 0 {
            Err(raw)
        } else {
            Ok(encode_u(immediate as u32, rd, OPCODE_LUI))
        }
    }
}

fn decompress_misc_alu(raw: u32) -> Result<u32, u32> {
    let op = (raw >> 10) & 0b11;
    let bit12 = (raw >> 12) & 1;
    let rd = compressed_rs1(raw);
    match op {
        0b00 => Ok(encode_i(
            c_shift_amount(raw),
            rd,
            FUNCT_SHIFT_RIGHT,
            rd,
            OPCODE_OP_IMM,
        )),
        0b01 => Ok(encode_i(
            0x400 | c_shift_amount(raw),
            rd,
            FUNCT_SHIFT_RIGHT,
            rd,
            OPCODE_OP_IMM,
        )),
        0b10 => Ok(encode_i(
            c_i_immediate(raw) as u32,
            rd,
            FUNCT_AND,
            rd,
            OPCODE_OP_IMM,
        )),
        0b11 if bit12 == 0 => decompress_register_alu(raw, OPCODE_OP),
        0b11 => decompress_register_alu(raw, OPCODE_OP_32),
        _ => Err(raw),
    }
}

fn decompress_register_alu(raw: u32, opcode: u32) -> Result<u32, u32> {
    let rd = compressed_rs1(raw);
    let rs2 = compressed_rs2(raw);
    let op = (raw >> 5) & 0b11;
    let (funct7, funct3) = match (opcode, op) {
        (OPCODE_OP, 0b00) => (FUNCT7_ALTERNATE, FUNCT_ADD),
        (OPCODE_OP, 0b01) => (FUNCT7_BASE, FUNCT_XOR),
        (OPCODE_OP, 0b10) => (FUNCT7_BASE, FUNCT_OR),
        (OPCODE_OP, 0b11) => (FUNCT7_BASE, FUNCT_AND),
        (OPCODE_OP_32, 0b00) => (FUNCT7_ALTERNATE, FUNCT_ADD),
        (OPCODE_OP_32, 0b01) => (FUNCT7_BASE, FUNCT_ADD),
        _ => return Err(raw),
    };
    Ok(encode_r(funct7, rs2, rd, funct3, rd, opcode))
}

fn decompress_jr_mv_ebreak_jalr_add(raw: u32) -> Result<u32, u32> {
    let rd_rs1 = c_rd_rs1(raw);
    let rs2 = c_rs2(raw);
    let bit12 = (raw >> 12) & 1;
    match (bit12, rd_rs1, rs2) {
        (0, 0, _) => Err(raw),
        (0, _, 0) => Ok(encode_i(0, rd_rs1, FUNCT_ADD, 0, OPCODE_JALR)),
        (0, _, _) => Ok(encode_r(FUNCT7_BASE, rs2, 0, FUNCT_ADD, rd_rs1, OPCODE_OP)),
        (1, 0, 0) => Ok(INSTRUCTION_EBREAK),
        (1, _, 0) => Ok(encode_i(0, rd_rs1, FUNCT_ADD, 1, OPCODE_JALR)),
        (1, _, _) => Ok(encode_r(
            FUNCT7_BASE,
            rs2,
            rd_rs1,
            FUNCT_ADD,
            rd_rs1,
            OPCODE_OP,
        )),
        _ => Err(raw),
    }
}

fn c_rd_rs1(raw: u32) -> u32 {
    (raw >> 7) & 0x1f
}

fn c_rs2(raw: u32) -> u32 {
    (raw >> 2) & 0x1f
}

fn compressed_rd(raw: u32) -> u32 {
    8 + ((raw >> 2) & 0x7)
}

fn compressed_rs1(raw: u32) -> u32 {
    8 + ((raw >> 7) & 0x7)
}

fn compressed_rs2(raw: u32) -> u32 {
    8 + ((raw >> 2) & 0x7)
}

fn c_i_immediate(raw: u32) -> i64 {
    sign_extend(((raw >> 2) & 0x1f) | (((raw >> 12) & 1) << 5), 6)
}

fn c_shift_amount(raw: u32) -> u32 {
    ((raw >> 2) & 0x1f) | (((raw >> 12) & 1) << 5)
}

fn c_addi4spn_immediate(raw: u32) -> u32 {
    (((raw >> 7) & 0xf) << 6)
        | (((raw >> 11) & 0x3) << 4)
        | (((raw >> 5) & 1) << 3)
        | (((raw >> 6) & 1) << 2)
}

fn c_lw_immediate(raw: u32) -> u32 {
    (((raw >> 10) & 0x7) << 3) | (((raw >> 6) & 1) << 2)
}

fn c_ld_immediate(raw: u32) -> u32 {
    (((raw >> 10) & 0x7) << 3) | (((raw >> 5) & 0x3) << 6)
}

fn c_lwsp_immediate(raw: u32) -> u32 {
    (((raw >> 12) & 1) << 5) | (((raw >> 4) & 0x7) << 2) | (((raw >> 2) & 0x3) << 6)
}

fn c_ldsp_immediate(raw: u32) -> u32 {
    (((raw >> 12) & 1) << 5) | (((raw >> 5) & 0x3) << 3) | (((raw >> 2) & 0x7) << 6)
}

fn c_swsp_immediate(raw: u32) -> u32 {
    (((raw >> 9) & 0xf) << 2) | (((raw >> 7) & 0x3) << 6)
}

fn c_sdsp_immediate(raw: u32) -> u32 {
    (((raw >> 10) & 0x7) << 3) | (((raw >> 7) & 0x7) << 6)
}

fn c_addi16sp_immediate(raw: u32) -> i64 {
    let immediate = (((raw >> 12) & 1) << 9)
        | (((raw >> 6) & 1) << 4)
        | (((raw >> 5) & 1) << 6)
        | (((raw >> 3) & 0x3) << 7)
        | (((raw >> 2) & 1) << 5);
    sign_extend(immediate, 10)
}

fn c_lui_immediate(raw: u32) -> i64 {
    sign_extend(((raw >> 2) & 0x1f) | (((raw >> 12) & 1) << 5), 6) << 12
}

fn c_b_immediate(raw: u32) -> i64 {
    let immediate = (((raw >> 12) & 1) << 8)
        | (((raw >> 5) & 0x3) << 6)
        | (((raw >> 2) & 1) << 5)
        | (((raw >> 10) & 0x3) << 3)
        | (((raw >> 3) & 0x3) << 1);
    sign_extend(immediate, 9)
}

fn c_j_immediate(raw: u32) -> i64 {
    let immediate = (((raw >> 12) & 1) << 11)
        | (((raw >> 8) & 1) << 10)
        | (((raw >> 9) & 0x3) << 8)
        | (((raw >> 6) & 1) << 7)
        | (((raw >> 7) & 1) << 6)
        | (((raw >> 2) & 1) << 5)
        | (((raw >> 11) & 1) << 4)
        | (((raw >> 3) & 0x7) << 1);
    sign_extend(immediate, 12)
}

fn encode_r(funct7: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32, opcode: u32) -> u32 {
    (funct7 << FUNCT7_SHIFT)
        | (rs2 << RS2_SHIFT)
        | (rs1 << RS1_SHIFT)
        | (funct3 << FUNCT3_SHIFT)
        | (rd << RD_SHIFT)
        | opcode
}

fn encode_i(immediate: u32, rs1: u32, funct3: u32, rd: u32, opcode: u32) -> u32 {
    ((immediate & 0xfff) << I_IMMEDIATE_SHIFT)
        | (rs1 << RS1_SHIFT)
        | (funct3 << FUNCT3_SHIFT)
        | (rd << RD_SHIFT)
        | opcode
}

fn encode_s(immediate: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
    (((immediate >> 5) & 0x7f) << S_IMMEDIATE_HIGH_SHIFT)
        | (rs2 << RS2_SHIFT)
        | (rs1 << RS1_SHIFT)
        | (funct3 << FUNCT3_SHIFT)
        | ((immediate & 0x1f) << S_IMMEDIATE_LOW_SHIFT)
        | OPCODE_STORE
}

fn encode_b(immediate: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
    (((immediate >> 12) & 1) << 31)
        | (((immediate >> 5) & 0x3f) << 25)
        | (rs2 << RS2_SHIFT)
        | (rs1 << RS1_SHIFT)
        | (funct3 << FUNCT3_SHIFT)
        | (((immediate >> 1) & 0xf) << 8)
        | (((immediate >> 11) & 1) << 7)
        | OPCODE_BRANCH
}

fn encode_u(immediate: u32, rd: u32, opcode: u32) -> u32 {
    (immediate & UPPER_IMMEDIATE_MASK) | (rd << RD_SHIFT) | opcode
}

fn encode_j(immediate: u32, rd: u32) -> u32 {
    (((immediate >> 20) & 1) << 31)
        | (((immediate >> 1) & 0x3ff) << 21)
        | (((immediate >> 11) & 1) << 20)
        | (((immediate >> 12) & 0xff) << 12)
        | (rd << RD_SHIFT)
        | OPCODE_JAL
}

fn register_field(instruction: u32, shift: u32) -> usize {
    bits(instruction, shift, REGISTER_FIELD_BITS) as usize
}

fn bits(value: u32, shift: u32, width: u32) -> u32 {
    (value >> shift) & ((1 << width) - 1)
}

fn csr_address(instruction: u32) -> u16 {
    bits(instruction, CSR_SHIFT, CSR_BITS) as u16
}

impl CsrFile {
    fn read(&self, address: u16) -> Option<u64> {
        match address {
            CSR_MVENDORID | CSR_MARCHID | CSR_MIMPID | CSR_MHARTID => Some(0),
            CSR_MSTATUS => Some(self.mstatus),
            CSR_MISA => Some(MISA_VALUE),
            CSR_MIE => Some(self.mie),
            CSR_MTVEC => Some(self.mtvec),
            CSR_MSCRATCH => Some(self.mscratch),
            CSR_MEPC => Some(self.mepc),
            CSR_MCAUSE => Some(self.mcause),
            CSR_MTVAL => Some(self.mtval),
            CSR_MIP => Some(self.mip),
            CSR_CYCLE => Some(self.cycle),
            CSR_TIME => Some(self.cycle),
            CSR_INSTRET => Some(self.instret),
            _ => None,
        }
    }

    fn write(&mut self, address: u16, value: u64) -> Option<()> {
        match address {
            CSR_MSTATUS => self.mstatus = value & MSTATUS_WRITABLE_MASK,
            CSR_MIE => self.mie = value & MIE_WRITABLE_MASK,
            CSR_MTVEC => self.mtvec = value & MTVEC_WRITABLE_MASK,
            CSR_MSCRATCH => self.mscratch = value & MSCRATCH_WRITABLE_MASK,
            CSR_MEPC => self.mepc = value & MEPC_WRITABLE_MASK,
            CSR_MCAUSE => self.mcause = value & MCAUSE_WRITABLE_MASK,
            CSR_MTVAL => self.mtval = value & MTVAL_WRITABLE_MASK,
            CSR_MIP => self.mip = value & MIP_WRITABLE_MASK,
            _ => return None,
        }
        Some(())
    }

    fn is_read_only(&self, address: u16) -> bool {
        matches!(
            address,
            CSR_MVENDORID
                | CSR_MARCHID
                | CSR_MIMPID
                | CSR_MHARTID
                | CSR_MISA
                | CSR_CYCLE
                | CSR_TIME
                | CSR_INSTRET
        )
    }

    fn retire_instruction(&mut self) {
        self.cycle = self.cycle.wrapping_add(1);
        self.instret = self.instret.wrapping_add(1);
    }
}

fn illegal(pc: u64, instruction: u32) -> StepError {
    StepError::IllegalInstruction { pc, instruction }
}

fn sign_extend(value: u32, bits: u32) -> i64 {
    ((value << (INSTRUCTION_BITS - bits)) as i32 >> (INSTRUCTION_BITS - bits)) as i64
}

fn sign_extend_u64(value: u64, bits: u32) -> u64 {
    ((value << (XLEN_BITS - bits)) as i64 >> (XLEN_BITS - bits)) as u64
}

fn sign_extend_word(value: u32) -> u64 {
    value as i32 as i64 as u64
}

fn mulh(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as i64 as i128)) >> XLEN_BITS) as u64
}

fn mulhsu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as u128 as i128)) >> XLEN_BITS) as u64
}

fn mulhu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as u128) * (rhs as u128)) >> XLEN_BITS) as u64
}

fn div(lhs: u64, rhs: u64) -> u64 {
    let dividend = lhs as i64;
    let divisor = rhs as i64;
    if divisor == 0 {
        u64::MAX
    } else if dividend == i64::MIN && divisor == -1 {
        lhs
    } else {
        dividend.wrapping_div(divisor) as u64
    }
}

fn divu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        u64::MAX
    } else {
        lhs / rhs
    }
}

fn rem(lhs: u64, rhs: u64) -> u64 {
    let dividend = lhs as i64;
    let divisor = rhs as i64;
    if divisor == 0 {
        lhs
    } else if dividend == i64::MIN && divisor == -1 {
        0
    } else {
        dividend.wrapping_rem(divisor) as u64
    }
}

fn remu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

fn divw(lhs: u32, rhs: u32) -> u32 {
    let dividend = lhs as i32;
    let divisor = rhs as i32;
    if divisor == 0 {
        u32::MAX
    } else if dividend == i32::MIN && divisor == -1 {
        lhs
    } else {
        dividend.wrapping_div(divisor) as u32
    }
}

fn divuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        u32::MAX
    } else {
        lhs / rhs
    }
}

fn remw(lhs: u32, rhs: u32) -> u32 {
    let dividend = lhs as i32;
    let divisor = rhs as i32;
    if divisor == 0 {
        lhs
    } else if dividend == i32::MIN && divisor == -1 {
        0
    } else {
        dividend.wrapping_rem(divisor) as u32
    }
}

fn remuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

fn upper_immediate(instruction: u32) -> u64 {
    sign_extend_u64(
        (instruction & UPPER_IMMEDIATE_MASK) as u64,
        U_IMMEDIATE_BITS,
    )
}

fn i_immediate(instruction: u32) -> i64 {
    sign_extend(
        bits(instruction, I_IMMEDIATE_SHIFT, I_IMMEDIATE_BITS),
        I_IMMEDIATE_BITS,
    )
}

fn s_immediate(instruction: u32) -> i64 {
    let low = bits(instruction, S_IMMEDIATE_LOW_SHIFT, S_IMMEDIATE_LOW_BITS);
    let high = bits(instruction, S_IMMEDIATE_HIGH_SHIFT, S_IMMEDIATE_HIGH_BITS);
    sign_extend((high << S_IMMEDIATE_LOW_BITS) | low, I_IMMEDIATE_BITS)
}

fn b_immediate(instruction: u32) -> i64 {
    let value = ((instruction >> 31) << 12)
        | (((instruction >> 7) & 1) << 11)
        | (((instruction >> 25) & 0x3f) << 5)
        | (((instruction >> 8) & 0xf) << 1);
    sign_extend(value, B_IMMEDIATE_BITS)
}

fn j_immediate(instruction: u32) -> i64 {
    let value = ((instruction >> 31) << 20)
        | (((instruction >> 12) & 0xff) << 12)
        | (((instruction >> 20) & 1) << 11)
        | (((instruction >> 21) & 0x3ff) << 1);
    sign_extend(value, J_IMMEDIATE_BITS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::DRAM_START;

    #[test]
    fn x0_cannot_be_changed() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        bus.write_u32(DRAM_START, 0x0050_0013).unwrap(); // addi x0, x0, 5
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(0), 0);
    }

    #[test]
    fn taken_branch_replaces_next_pc() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        bus.write_u32(DRAM_START, 0x0000_0463).unwrap(); // beq x0, x0, 8
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, DRAM_START + 8);
    }

    #[test]
    fn addiw_sign_extends_to_xlen() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        cpu.set_register(1, 0x7fff_ffff);
        bus.write_u32(DRAM_START, 0x0010_809b).unwrap(); // addiw x1, x1, 1
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(1), 0xffff_ffff_8000_0000);
    }

    #[test]
    fn fence_i_is_a_validated_noop() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        bus.write_u32(DRAM_START, INSTRUCTION_FENCE_I).unwrap();

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, DRAM_START + INSTRUCTION_SIZE);
    }

    #[test]
    fn non_canonical_fence_i_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let instruction = INSTRUCTION_FENCE_I | (1 << RD_SHIFT);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_eq!(
            cpu.step(&mut bus),
            Err(StepError::IllegalInstruction {
                pc: DRAM_START,
                instruction,
            })
        );
    }

    #[test]
    fn rv64m_multiplies_low_and_high_halves() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(32);
        cpu.set_register(1, 0xffff_ffff_ffff_fffe);
        cpu.set_register(2, 3);
        bus.write_u32(DRAM_START, encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_ADD, 5))
            .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_SHIFT_LEFT, 6),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_SET_LESS_THAN, 7),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 3,
            encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_SET_LESS_THAN_UNSIGNED, 8),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0xffff_ffff_ffff_fffa);
        assert_eq!(cpu.register(6), 0xffff_ffff_ffff_ffff);
        assert_eq!(cpu.register(7), 0xffff_ffff_ffff_ffff);
        assert_eq!(cpu.register(8), 2);
    }

    #[test]
    fn rv64m_division_and_remainder_follow_edge_case_rules() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(32);
        cpu.set_register(1, 0xffff_ffff_ffff_fff9);
        cpu.set_register(2, 3);
        cpu.set_register(3, 0);
        bus.write_u32(DRAM_START, encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_XOR, 5))
            .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_OR, 6),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_r(FUNCT7_MULTIPLY, 3, 1, FUNCT_XOR, 7),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 3,
            encode_r(FUNCT7_MULTIPLY, 3, 1, FUNCT_OR, 8),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), (-2_i64) as u64);
        assert_eq!(cpu.register(6), (-1_i64) as u64);
        assert_eq!(cpu.register(7), u64::MAX);
        assert_eq!(cpu.register(8), 0xffff_ffff_ffff_fff9);
    }

    #[test]
    fn rv64m_signed_division_overflow_returns_specified_values() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        cpu.set_register(1, i64::MIN as u64);
        cpu.set_register(2, u64::MAX);
        bus.write_u32(DRAM_START, encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_XOR, 5))
            .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_r(FUNCT7_MULTIPLY, 2, 1, FUNCT_OR, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), i64::MIN as u64);
        assert_eq!(cpu.register(6), 0);
    }

    #[test]
    fn rv64m_word_operations_sign_extend_results() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(32);
        cpu.set_register(1, 0x0000_0000_8000_0000);
        cpu.set_register(2, 0xffff_ffff_ffff_ffff);
        cpu.set_register(3, 0);
        bus.write_u32(DRAM_START, encode_r32(FUNCT7_MULTIPLY, 2, 1, FUNCT_ADD, 5))
            .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_r32(FUNCT7_MULTIPLY, 2, 1, FUNCT_XOR, 6),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_r32(FUNCT7_MULTIPLY, 2, 1, FUNCT_OR, 7),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 3,
            encode_r32(FUNCT7_MULTIPLY, 3, 1, FUNCT_SHIFT_RIGHT, 8),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0xffff_ffff_8000_0000);
        assert_eq!(cpu.register(6), 0xffff_ffff_8000_0000);
        assert_eq!(cpu.register(7), 0);
        assert_eq!(cpu.register(8), u64::MAX);
    }

    #[test]
    fn rv64a_atomic_memory_operations_return_old_values_and_store_new_values() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(64);
        let word_address = DRAM_START + 40;
        let double_address = DRAM_START + 48;
        cpu.set_register(1, word_address);
        cpu.set_register(2, 5);
        cpu.set_register(3, double_address);
        cpu.set_register(4, 7);
        bus.write_u32(word_address, 0xffff_fffe).unwrap();
        bus.write_u64(double_address, 10).unwrap();
        bus.write_u32(
            DRAM_START,
            encode_amo(AMO_FUNCT_ADD, 2, 1, FUNCT_AMO_WORD, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_amo(AMO_FUNCT_MAX_UNSIGNED, 4, 3, FUNCT_AMO_DOUBLE, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0xffff_ffff_ffff_fffe);
        assert_eq!(bus.read_u32(word_address).unwrap(), 3);
        assert_eq!(cpu.register(6), 10);
        assert_eq!(bus.read_u64(double_address).unwrap(), 10);
    }

    #[test]
    fn rv64a_load_reserved_and_store_conditional_report_success_or_failure() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(64);
        let address = DRAM_START + 40;
        cpu.set_register(1, address);
        cpu.set_register(2, 0x55aa);
        bus.write_u64(address, 0x1234).unwrap();
        bus.write_u32(
            DRAM_START,
            encode_amo(AMO_FUNCT_LOAD_RESERVED, 0, 1, FUNCT_AMO_DOUBLE, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_amo(AMO_FUNCT_STORE_CONDITIONAL, 2, 1, FUNCT_AMO_DOUBLE, 6),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_amo(AMO_FUNCT_STORE_CONDITIONAL, 2, 1, FUNCT_AMO_DOUBLE, 7),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0x1234);
        assert_eq!(cpu.register(6), 0);
        assert_eq!(cpu.register(7), 1);
        assert_eq!(bus.read_u64(address).unwrap(), 0x55aa);
    }

    #[test]
    fn rv64a_store_to_reserved_address_makes_store_conditional_fail() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(64);
        let address = DRAM_START + 40;
        cpu.set_register(1, address);
        cpu.set_register(2, 0x1234);
        cpu.set_register(3, 0x5678);
        bus.write_u64(address, 0).unwrap();
        bus.write_u32(
            DRAM_START,
            encode_amo(AMO_FUNCT_LOAD_RESERVED, 0, 1, FUNCT_AMO_DOUBLE, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_s(0, 2, 1, FUNCT_STORE_DOUBLE),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_amo(AMO_FUNCT_STORE_CONDITIONAL, 3, 1, FUNCT_AMO_DOUBLE, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(6), 1);
        assert_eq!(bus.read_u64(address).unwrap(), 0x1234);
    }

    #[test]
    fn rv64a_lr_requires_zero_rs2() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        let instruction = encode_amo(AMO_FUNCT_LOAD_RESERVED, 1, 0, FUNCT_AMO_WORD, 5);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_eq!(
            cpu.step(&mut bus),
            Err(StepError::IllegalInstruction {
                pc: DRAM_START,
                instruction,
            })
        );
    }

    #[test]
    fn csrrw_swaps_register_and_machine_csr_values() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        cpu.set_register(6, 0xaaaa);
        cpu.set_register(7, 0x5555);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRW, 6, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRW, 7, 5),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0xaaaa);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0x5555);
    }

    #[test]
    fn csrrs_and_csrrc_update_with_register_masks() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(24);
        cpu.set_register(5, 0b1010);
        cpu.set_register(6, 0b0101);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRW, 5, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRS, 6, 7),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRC, 6, 8),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(7), 0b1010);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0b1111);

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(8), 0b1111);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0b1010);
    }

    #[test]
    fn csr_zero_masks_read_without_writing() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(24);
        cpu.set_register(1, 0x1234);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRS, 0, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRSI, 0, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0x1234);
        assert_eq!(cpu.register(6), 0x1234);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0x1234);
    }

    #[test]
    fn csr_immediate_instructions_use_rs1_field_as_zimm() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(24);
        cpu.set_register(1, 0b1010);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRSI, 0b0011, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRCI, 0b0110, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(5), 0b1010);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0b1011);

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(6), 0b1011);
        assert_eq!(cpu.csr(CSR_MSCRATCH), 0b1001);
    }

    #[test]
    fn machine_csr_writes_apply_warl_masks() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(24);
        cpu.set_register(1, u64::MAX);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MSTATUS), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MTVEC), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_MEPC), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.csr(CSR_MSTATUS), MSTATUS_WRITABLE_MASK);
        assert_eq!(cpu.csr(CSR_MTVEC), u64::MAX & MTVEC_WRITABLE_MASK);
        assert_eq!(cpu.csr(CSR_MEPC), u64::MAX & MEPC_WRITABLE_MASK);
    }

    #[test]
    fn compressed_instructions_execute_and_advance_by_two() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        bus.write_u16(DRAM_START, 0x4085).unwrap();
        bus.write_u16(DRAM_START + 2, 0x0089).unwrap();
        bus.write_u16(DRAM_START + 4, 0x9002).unwrap();

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, DRAM_START + 2);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, DRAM_START + 4);
        assert_eq!(cpu.register(1), 3);
        assert_eq!(
            cpu.step(&mut bus),
            Ok(Some(HaltReason::Breakpoint { code: 0 }))
        );
    }

    #[test]
    fn compressed_alu_quadrant_distinguishes_shift_and_and_immediates() {
        assert_eq!(
            decode_compressed_instruction(0x9005),
            Some(encode_i(33, 8, FUNCT_SHIFT_RIGHT, 8, OPCODE_OP_IMM))
        );
        assert_eq!(
            decode_compressed_instruction(0x8405),
            Some(encode_i(0x401, 8, FUNCT_SHIFT_RIGHT, 8, OPCODE_OP_IMM))
        );
        assert_eq!(
            decode_compressed_instruction(0x987d),
            Some(encode_i(0xfff, 8, FUNCT_AND, 8, OPCODE_OP_IMM))
        );
    }

    #[test]
    fn compressed_branch_uses_halfword_pc_relative_offset() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        bus.write_u16(DRAM_START, 0xc011).unwrap();
        bus.write_u16(DRAM_START + 4, 0x4085).unwrap();

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, DRAM_START + 4);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.register(1), 1);
    }

    #[test]
    fn machine_identity_csrs_are_readable() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_MISA), FUNCT_CSRRS, 0, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_MHARTID), FUNCT_CSRRS, 0, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), MISA_VALUE);
        assert_eq!(cpu.register(6), 0);
    }

    #[test]
    fn counters_track_successfully_retired_instructions() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        bus.write_u32(DRAM_START, 0x0010_8093).unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_CYCLE), FUNCT_CSRRS, 0, 5),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_INSTRET), FUNCT_CSRRS, 0, 6),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 1);
        assert_eq!(cpu.register(6), 2);
        assert_eq!(cpu.csr(CSR_CYCLE), 3);
        assert_eq!(cpu.csr(CSR_TIME), 3);
    }

    #[test]
    fn writing_read_only_csr_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let instruction = encode_csr(u32::from(CSR_CYCLE), FUNCT_CSRRW, 1, 0);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_eq!(
            cpu.step(&mut bus),
            Err(StepError::IllegalInstruction {
                pc: DRAM_START,
                instruction,
            })
        );
        assert_eq!(cpu.csr(CSR_CYCLE), 0);
    }

    #[test]
    fn unknown_csr_access_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let instruction = encode_csr(0x100, FUNCT_CSRRS, 0, 5);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_eq!(
            cpu.step(&mut bus),
            Err(StepError::IllegalInstruction {
                pc: DRAM_START,
                instruction,
            })
        );
    }

    fn encode_r(funct7: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (funct7 << FUNCT7_SHIFT)
            | (rs2 << RS2_SHIFT)
            | (rs1 << RS1_SHIFT)
            | (funct3 << FUNCT3_SHIFT)
            | (rd << RD_SHIFT)
            | OPCODE_OP
    }

    fn encode_r32(funct7: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (funct7 << FUNCT7_SHIFT)
            | (rs2 << RS2_SHIFT)
            | (rs1 << RS1_SHIFT)
            | (funct3 << FUNCT3_SHIFT)
            | (rd << RD_SHIFT)
            | OPCODE_OP_32
    }

    fn encode_s(immediate: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
        (((immediate >> 5) & 0x7f) << S_IMMEDIATE_HIGH_SHIFT)
            | (rs2 << RS2_SHIFT)
            | (rs1 << RS1_SHIFT)
            | (funct3 << FUNCT3_SHIFT)
            | ((immediate & 0x1f) << S_IMMEDIATE_LOW_SHIFT)
            | OPCODE_STORE
    }

    fn encode_amo(funct5: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
        (funct5 << AMO_FUNCT_SHIFT)
            | (rs2 << RS2_SHIFT)
            | (rs1 << RS1_SHIFT)
            | (funct3 << FUNCT3_SHIFT)
            | (rd << RD_SHIFT)
            | OPCODE_AMO
    }

    fn encode_csr(csr: u32, funct3: u32, rs1: u32, rd: u32) -> u32 {
        (csr << CSR_SHIFT)
            | (rs1 << RS1_SHIFT)
            | (funct3 << FUNCT3_SHIFT)
            | (rd << RD_SHIFT)
            | OPCODE_SYSTEM
    }
}
