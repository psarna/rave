use crate::{Bus, BusError};
use std::fmt;

const REGISTER_COUNT: usize = 32;
const ZERO_REGISTER: usize = 0;
const RETURN_VALUE_REGISTER: usize = 10;
const INSTRUCTION_SIZE: u64 = 4;
const INSTRUCTION_BITS: u32 = 32;
const XLEN_BITS: u32 = 64;

const RD_SHIFT: u32 = 7;
const RS1_SHIFT: u32 = 15;
const RS2_SHIFT: u32 = 20;
const FUNCT3_SHIFT: u32 = 12;
const FUNCT7_SHIFT: u32 = 25;
const SHIFT_PREFIX_SHIFT: u32 = 26;
const REGISTER_FIELD_BITS: u32 = 5;
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

const FUNCT7_BASE: u32 = 0;
const FUNCT7_ALTERNATE: u32 = 0x20;
const SHIFT64_LOGICAL_PREFIX: u32 = 0;
const SHIFT64_ARITHMETIC_PREFIX: u32 = 0x10;
const SHIFT64_MASK: u32 = 0x3f;
const SHIFT32_MASK: u32 = 0x1f;

const INSTRUCTION_ECALL: u32 = 0x0000_0073;
const INSTRUCTION_EBREAK: u32 = 0x0010_0073;
const UPPER_IMMEDIATE_MASK: u32 = 0xffff_f000;
const JALR_ALIGNMENT_MASK: u64 = !1;

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
    pub pc: u64,
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

enum Execution {
    Continue { next_pc: u64 },
    Halt(HaltReason),
}

impl Cpu {
    pub fn new(pc: u64) -> Self {
        Self {
            registers: [0; REGISTER_COUNT],
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

    pub fn step(&mut self, bus: &mut Bus) -> Result<Option<HaltReason>, StepError> {
        let pc = self.pc;
        let instruction = Decoded::new(bus.read_u32(pc)?);
        let sequential_pc = pc.wrapping_add(INSTRUCTION_SIZE);

        let execution = match instruction.opcode {
            OPCODE_LUI => self.execute_lui(instruction, sequential_pc),
            OPCODE_AUIPC => self.execute_auipc(instruction, pc, sequential_pc),
            OPCODE_JAL => self.execute_jal(instruction, pc, sequential_pc),
            OPCODE_JALR => self.execute_jalr(instruction, sequential_pc)?,
            OPCODE_BRANCH => self.execute_branch(instruction, pc, sequential_pc)?,
            OPCODE_LOAD => self.execute_load(instruction, bus, sequential_pc)?,
            OPCODE_STORE => self.execute_store(instruction, bus, sequential_pc)?,
            OPCODE_OP_IMM => self.execute_op_immediate(instruction, sequential_pc)?,
            OPCODE_OP_IMM_32 => self.execute_op_immediate_word(instruction, sequential_pc)?,
            OPCODE_OP => self.execute_op(instruction, sequential_pc)?,
            OPCODE_OP_32 => self.execute_op_word(instruction, sequential_pc)?,
            OPCODE_MISC_MEM => self.execute_misc_mem(instruction, sequential_pc)?,
            OPCODE_SYSTEM => self.execute_system(instruction, pc)?,
            _ => return Err(illegal(pc, instruction.raw)),
        };

        match execution {
            Execution::Continue { next_pc } => {
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
        Ok(Execution::Continue { next_pc })
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
            (FUNCT_SHIFT_LEFT, FUNCT7_BASE) => lhs << (rhs & u64::from(SHIFT64_MASK)),
            (FUNCT_SET_LESS_THAN, FUNCT7_BASE) => ((lhs as i64) < (rhs as i64)) as u64,
            (FUNCT_SET_LESS_THAN_UNSIGNED, FUNCT7_BASE) => (lhs < rhs) as u64,
            (FUNCT_XOR, FUNCT7_BASE) => lhs ^ rhs,
            (FUNCT_SHIFT_RIGHT, FUNCT7_BASE) => lhs >> (rhs & u64::from(SHIFT64_MASK)),
            (FUNCT_SHIFT_RIGHT, FUNCT7_ALTERNATE) => {
                ((lhs as i64) >> (rhs & u64::from(SHIFT64_MASK))) as u64
            }
            (FUNCT_OR, FUNCT7_BASE) => lhs | rhs,
            (FUNCT_AND, FUNCT7_BASE) => lhs & rhs,
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
            (FUNCT_SHIFT_LEFT, FUNCT7_BASE) => lhs << (rhs & u64::from(SHIFT32_MASK)),
            (FUNCT_SHIFT_RIGHT, FUNCT7_BASE) => lhs >> (rhs & u64::from(SHIFT32_MASK)),
            (FUNCT_SHIFT_RIGHT, FUNCT7_ALTERNATE) => {
                ((lhs as i32) >> (rhs & u64::from(SHIFT32_MASK))) as u32
            }
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        self.set_register(instruction.rd, sign_extend_word(word));
        Ok(Execution::Continue { next_pc })
    }

    fn execute_misc_mem(&self, instruction: Decoded, next_pc: u64) -> Result<Execution, StepError> {
        if instruction.funct3 == FUNCT_ADD {
            Ok(Execution::Continue { next_pc })
        } else {
            Err(illegal(self.pc, instruction.raw))
        }
    }

    fn execute_system(&self, instruction: Decoded, pc: u64) -> Result<Execution, StepError> {
        match instruction.raw {
            INSTRUCTION_EBREAK => Ok(Execution::Halt(HaltReason::Breakpoint {
                code: self.registers[RETURN_VALUE_REGISTER],
            })),
            INSTRUCTION_ECALL => Err(StepError::EnvironmentCall { pc }),
            _ => Err(illegal(pc, instruction.raw)),
        }
    }

    fn effective_i_address(&self, instruction: Decoded) -> u64 {
        self.registers[instruction.rs1].wrapping_add(i_immediate(instruction.raw) as u64)
    }
}

fn register_field(instruction: u32, shift: u32) -> usize {
    bits(instruction, shift, REGISTER_FIELD_BITS) as usize
}

fn bits(value: u32, shift: u32, width: u32) -> u32 {
    (value >> shift) & ((1 << width) - 1)
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
}
