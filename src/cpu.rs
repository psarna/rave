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
const INSTRUCTION_MRET: u32 = 0x3020_0073;
const INSTRUCTION_SRET: u32 = 0x1020_0073;
const INSTRUCTION_WFI: u32 = 0x1050_0073;
pub const INSTRUCTION_SFENCE_VMA: u32 = 0x1200_0073;
pub const INSTRUCTION_SFENCE_VMA_MASK: u32 = 0xfe00_7fff;
const INSTRUCTION_NOP: u32 = 0x0000_0013; // addi x0, x0, 0
const INSTRUCTION_FENCE_I: u32 = 0x0000_100f;
const UPPER_IMMEDIATE_MASK: u32 = 0xffff_f000;
const JALR_ALIGNMENT_MASK: u64 = !1;

const CSR_MVENDORID: u16 = 0xf11;
const CSR_MARCHID: u16 = 0xf12;
const CSR_MIMPID: u16 = 0xf13;
const CSR_MHARTID: u16 = 0xf14;
pub const CSR_MSTATUS: u16 = 0x300;
const CSR_MISA: u16 = 0x301;
const CSR_MEDELEG: u16 = 0x302;
const CSR_MIDELEG: u16 = 0x303;
const CSR_MIE: u16 = 0x304;
const CSR_MTVEC: u16 = 0x305;
pub const CSR_MCOUNTEREN: u16 = 0x306;
const CSR_MSCRATCH: u16 = 0x340;
const CSR_MEPC: u16 = 0x341;
const CSR_MCAUSE: u16 = 0x342;
const CSR_MTVAL: u16 = 0x343;
const CSR_MIP: u16 = 0x344;
const CSR_SSTATUS: u16 = 0x100;
const CSR_SIE: u16 = 0x104;
const CSR_STVEC: u16 = 0x105;
pub const CSR_SCOUNTEREN: u16 = 0x106;
const CSR_SSCRATCH: u16 = 0x140;
const CSR_SEPC: u16 = 0x141;
const CSR_SCAUSE: u16 = 0x142;
const CSR_STVAL: u16 = 0x143;
const CSR_SIP: u16 = 0x144;
pub const CSR_SATP: u16 = 0x180;
const CSR_CYCLE: u16 = 0xc00;
const CSR_TIME: u16 = 0xc01;
const CSR_INSTRET: u16 = 0xc02;

const MSTATUS_SIE: u64 = 1 << 1;
const MSTATUS_MIE: u64 = 1 << 3;
const MSTATUS_SPIE: u64 = 1 << 5;
const MSTATUS_MPIE: u64 = 1 << 7;
const MSTATUS_SPP: u64 = 1 << 8;
const MSTATUS_MPP_SHIFT: u64 = 11;
const MSTATUS_MPP_MASK: u64 = 0b11 << MSTATUS_MPP_SHIFT;
const MSTATUS_MPP_USER: u64 = 0b00 << MSTATUS_MPP_SHIFT;
const MSTATUS_MPP_SUPERVISOR: u64 = 0b01 << MSTATUS_MPP_SHIFT;
const MSTATUS_MPP_RESERVED: u64 = 0b10 << MSTATUS_MPP_SHIFT;
const MSTATUS_MPP_MACHINE: u64 = 0b11 << MSTATUS_MPP_SHIFT;
const MSTATUS_MPRV: u64 = 1 << 17;
const MSTATUS_SUM: u64 = 1 << 18;
const MSTATUS_MXR: u64 = 1 << 19;
const SSTATUS_WRITABLE_MASK: u64 =
    MSTATUS_SIE | MSTATUS_SPIE | MSTATUS_SPP | MSTATUS_SUM | MSTATUS_MXR;
const MSTATUS_WRITABLE_MASK: u64 =
    SSTATUS_WRITABLE_MASK | MSTATUS_MIE | MSTATUS_MPIE | MSTATUS_MPP_MASK | MSTATUS_MPRV;
const MIP_SSIP: u64 = 1 << 1;
const MIP_MSIP: u64 = 1 << 3;
const MIP_STIP: u64 = 1 << 5;
const MIP_MTIP: u64 = 1 << 7;
const MIP_SEIP: u64 = 1 << 9;
const MIP_MEIP: u64 = 1 << 11;
const S_INTERRUPT_MASK: u64 = MIP_SSIP | MIP_STIP | MIP_SEIP;
const M_INTERRUPT_MASK: u64 = MIP_MSIP | MIP_MTIP | MIP_MEIP;
const MIE_WRITABLE_MASK: u64 = S_INTERRUPT_MASK | M_INTERRUPT_MASK;
const MIP_WRITABLE_MASK: u64 = S_INTERRUPT_MASK;
const MTVEC_WRITABLE_MASK: u64 = !0b10;
const MEPC_WRITABLE_MASK: u64 = !1;
const MCAUSE_WRITABLE_MASK: u64 = u64::MAX;
const MTVAL_WRITABLE_MASK: u64 = u64::MAX;
const MSCRATCH_WRITABLE_MASK: u64 = u64::MAX;
const MEDELEG_WRITABLE_MASK: u64 = u64::MAX;
const MIDELEG_WRITABLE_MASK: u64 = S_INTERRUPT_MASK;
const MISA_VALUE: u64 =
    (2 << 62) | (1 << 0) | (1 << 2) | (1 << 8) | (1 << 12) | (1 << 18) | (1 << 20);
const COUNTEREN_CYCLE: u64 = 1 << 0;
const COUNTEREN_TIME: u64 = 1 << 1;
const COUNTEREN_INSTRET: u64 = 1 << 2;
const COUNTEREN_WRITABLE_MASK: u64 = COUNTEREN_CYCLE | COUNTEREN_TIME | COUNTEREN_INSTRET;

const TRAP_INSTRUCTION_ACCESS_FAULT: u64 = 1;
const TRAP_ILLEGAL_INSTRUCTION: u64 = 2;
const TRAP_BREAKPOINT: u64 = 3;
const TRAP_LOAD_ADDRESS_MISALIGNED: u64 = 4;
const TRAP_STORE_ADDRESS_MISALIGNED: u64 = 6;
const TRAP_LOAD_ACCESS_FAULT: u64 = 5;
const TRAP_STORE_ACCESS_FAULT: u64 = 7;
const TRAP_ECALL_FROM_USER: u64 = 8;
const TRAP_ECALL_FROM_SUPERVISOR: u64 = 9;
const TRAP_ECALL_FROM_MACHINE: u64 = 11;
const TRAP_INSTRUCTION_PAGE_FAULT: u64 = 12;
const TRAP_LOAD_PAGE_FAULT: u64 = 13;
const TRAP_STORE_PAGE_FAULT: u64 = 15;
const INTERRUPT_BIT: u64 = 1 << 63;
const INTERRUPT_SUPERVISOR_SOFTWARE: u64 = 1;
const INTERRUPT_MACHINE_SOFTWARE: u64 = 3;
const INTERRUPT_SUPERVISOR_TIMER: u64 = 5;
const INTERRUPT_MACHINE_TIMER: u64 = 7;
const INTERRUPT_SUPERVISOR_EXTERNAL: u64 = 9;
const INTERRUPT_MACHINE_EXTERNAL: u64 = 11;
const MTVEC_MODE_MASK: u64 = 0b11;
const SATP_MODE_SHIFT: u64 = 60;
const SATP_MODE_BARE: u64 = 0;
const SATP_MODE_SV39: u64 = 8;
const SATP_ASID_MASK: u64 = 0xffff << 44;
const SATP_PPN_MASK: u64 = (1 << 44) - 1;
const PAGE_SHIFT: u64 = 12;
const PAGE_SIZE: u64 = 1 << PAGE_SHIFT;
const PAGE_OFFSET_MASK: u64 = PAGE_SIZE - 1;
const PTE_SIZE: u64 = 8;
const PTE_V: u64 = 1 << 0;
const PTE_R: u64 = 1 << 1;
const PTE_W: u64 = 1 << 2;
const PTE_X: u64 = 1 << 3;
const PTE_U: u64 = 1 << 4;
const PTE_A: u64 = 1 << 6;
const PTE_D: u64 = 1 << 7;
const PTE_PPN_SHIFT: u64 = 10;
const PTE_PPN_MASK: u64 = (1 << 44) - 1;
const VPN_MASK: u64 = 0x1ff;
const SV39_VA_BITS: u32 = 39;
const SV39_LEVELS: usize = 3;
const SV39_TOP_LEVEL: usize = SV39_LEVELS - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeMode {
    User,
    Supervisor,
    Machine,
}

impl PrivilegeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "U",
            Self::Supervisor => "S",
            Self::Machine => "M",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltReason {
    Breakpoint { code: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    Bus(BusError),
    InstructionAccessFault { pc: u64, address: u64 },
    LoadAccessFault { pc: u64, address: u64 },
    StoreAccessFault { pc: u64, address: u64 },
    LoadAddressMisaligned { pc: u64, address: u64 },
    StoreAddressMisaligned { pc: u64, address: u64 },
    InstructionPageFault { pc: u64, address: u64 },
    LoadPageFault { pc: u64, address: u64 },
    StorePageFault { pc: u64, address: u64 },
    IllegalInstruction { pc: u64, instruction: u32 },
    Breakpoint { pc: u64 },
    EnvironmentCall { pc: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressAccess {
    Fetch,
    Load,
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressTranslation {
    pub physical_address: u64,
    pub paging_active: bool,
    pte_update: Option<(u64, u64)>,
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => error.fmt(f),
            Self::InstructionAccessFault { pc, address } => {
                write!(
                    f,
                    "instruction access fault at {pc:#018x} reading {address:#018x}"
                )
            }
            Self::LoadAccessFault { pc, address } => {
                write!(f, "load access fault at {pc:#018x} reading {address:#018x}")
            }
            Self::StoreAccessFault { pc, address } => {
                write!(
                    f,
                    "store access fault at {pc:#018x} writing {address:#018x}"
                )
            }
            Self::LoadAddressMisaligned { pc, address } => {
                write!(f, "misaligned load at {pc:#018x} reading {address:#018x}")
            }
            Self::StoreAddressMisaligned { pc, address } => {
                write!(f, "misaligned store at {pc:#018x} writing {address:#018x}")
            }
            Self::InstructionPageFault { pc, address } => {
                write!(
                    f,
                    "instruction page fault at {pc:#018x} reading {address:#018x}"
                )
            }
            Self::LoadPageFault { pc, address } => {
                write!(f, "load page fault at {pc:#018x} reading {address:#018x}")
            }
            Self::StorePageFault { pc, address } => {
                write!(f, "store page fault at {pc:#018x} writing {address:#018x}")
            }
            Self::IllegalInstruction { pc, instruction } => {
                write!(f, "illegal instruction {instruction:#010x} at {pc:#018x}")
            }
            Self::Breakpoint { pc } => write!(f, "breakpoint at {pc:#018x}"),
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
    privilege_mode: PrivilegeMode,
    host_ebreak_exit: bool,
    pub pc: u64,
}

#[derive(Debug, Clone, Default)]
struct CsrFile {
    mstatus: u64,
    medeleg: u64,
    mideleg: u64,
    mie: u64,
    mtvec: u64,
    mcounteren: u64,
    mscratch: u64,
    mepc: u64,
    mcause: u64,
    mtval: u64,
    mip: u64,
    stvec: u64,
    scounteren: u64,
    sscratch: u64,
    sepc: u64,
    scause: u64,
    stval: u64,
    satp: u64,
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

#[derive(Clone, Copy)]
enum MemoryAccess {
    Fetch,
    Load,
    Store,
}

impl From<AddressAccess> for MemoryAccess {
    fn from(value: AddressAccess) -> Self {
        match value {
            AddressAccess::Fetch => Self::Fetch,
            AddressAccess::Load => Self::Load,
            AddressAccess::Store => Self::Store,
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
            csrs: CsrFile::default(),
            reservation: None,
            privilege_mode: PrivilegeMode::Machine,
            host_ebreak_exit: true,
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

    /// Writes a CSR through its normal WARL legalization for debugger edits.
    pub fn set_csr_for_debug(&mut self, address: u16, value: u64) -> bool {
        self.csrs.write(address, value).is_some()
    }

    pub fn privilege_mode(&self) -> PrivilegeMode {
        self.privilege_mode
    }

    pub fn set_host_ebreak_exit(&mut self, enabled: bool) {
        self.host_ebreak_exit = enabled;
    }

    pub fn host_ebreak_exit_enabled(&self) -> bool {
        self.host_ebreak_exit
    }

    pub fn reservation_matches(&self, address: u64) -> bool {
        self.reservation == Some(address)
    }

    pub fn translate_address_for_debug(
        &self,
        bus: &Bus,
        address: u64,
        access: AddressAccess,
    ) -> Result<AddressTranslation, StepError> {
        self.translate_address_inner(bus, address, access.into())
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<Option<HaltReason>, StepError> {
        let pc = self.pc;
        self.refresh_interrupt_pending(bus);
        if let Some(cause) = self.pending_interrupt() {
            self.enter_trap(cause, pc, 0, true);
            return Ok(None);
        }

        let fetched = match self.fetch_instruction(bus, pc) {
            Ok(fetched) => fetched,
            Err(error) => return self.trap_or_error(error),
        };
        let instruction = Decoded::new(fetched.raw);
        let sequential_pc = pc.wrapping_add(fetched.size);

        let execution = match self.execute_decoded(instruction, bus, pc, sequential_pc) {
            Ok(execution) => execution,
            Err(error) => return self.trap_or_error(error),
        };

        match execution {
            Execution::Continue { next_pc } => {
                self.csrs.retire_instruction();
                bus.tick(1);
                self.refresh_interrupt_pending(bus);
                self.pc = next_pc;
                self.registers[ZERO_REGISTER] = 0;
                Ok(None)
            }
            Execution::Halt(reason) => Ok(Some(reason)),
        }
    }

    fn execute_decoded(
        &mut self,
        instruction: Decoded,
        bus: &mut Bus,
        pc: u64,
        sequential_pc: u64,
    ) -> Result<Execution, StepError> {
        match instruction.opcode {
            OPCODE_LUI => Ok(self.execute_lui(instruction, sequential_pc)),
            OPCODE_AUIPC => Ok(self.execute_auipc(instruction, pc, sequential_pc)),
            OPCODE_JAL => Ok(self.execute_jal(instruction, pc, sequential_pc)),
            OPCODE_JALR => self.execute_jalr(instruction, sequential_pc),
            OPCODE_BRANCH => self.execute_branch(instruction, pc, sequential_pc),
            OPCODE_LOAD => self.execute_load(instruction, bus, sequential_pc),
            OPCODE_STORE => self.execute_store(instruction, bus, sequential_pc),
            OPCODE_AMO => self.execute_amo(instruction, bus, sequential_pc),
            OPCODE_OP_IMM => self.execute_op_immediate(instruction, sequential_pc),
            OPCODE_OP_IMM_32 => self.execute_op_immediate_word(instruction, sequential_pc),
            OPCODE_OP => self.execute_op(instruction, sequential_pc),
            OPCODE_OP_32 => self.execute_op_word(instruction, sequential_pc),
            OPCODE_MISC_MEM => self.execute_misc_mem(instruction, sequential_pc),
            OPCODE_SYSTEM => self.execute_system(instruction, pc, sequential_pc),
            _ => Err(illegal(pc, instruction.raw)),
        }
    }

    fn trap_or_error(&mut self, error: StepError) -> Result<Option<HaltReason>, StepError> {
        match error {
            StepError::InstructionAccessFault { pc, address } => {
                self.enter_trap(TRAP_INSTRUCTION_ACCESS_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::LoadAccessFault { pc, address } => {
                self.enter_trap(TRAP_LOAD_ACCESS_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::StoreAccessFault { pc, address } => {
                self.enter_trap(TRAP_STORE_ACCESS_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::LoadAddressMisaligned { pc, address } => {
                self.enter_trap(TRAP_LOAD_ADDRESS_MISALIGNED, pc, address, false);
                Ok(None)
            }
            StepError::StoreAddressMisaligned { pc, address } => {
                self.enter_trap(TRAP_STORE_ADDRESS_MISALIGNED, pc, address, false);
                Ok(None)
            }
            StepError::IllegalInstruction { pc, instruction } => {
                self.enter_trap(TRAP_ILLEGAL_INSTRUCTION, pc, u64::from(instruction), false);
                Ok(None)
            }
            StepError::Breakpoint { pc } => {
                self.enter_trap(TRAP_BREAKPOINT, pc, 0, false);
                Ok(None)
            }
            StepError::EnvironmentCall { pc } => {
                self.enter_trap(self.environment_call_cause(), pc, 0, false);
                Ok(None)
            }
            StepError::InstructionPageFault { pc, address } => {
                self.enter_trap(TRAP_INSTRUCTION_PAGE_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::LoadPageFault { pc, address } => {
                self.enter_trap(TRAP_LOAD_PAGE_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::StorePageFault { pc, address } => {
                self.enter_trap(TRAP_STORE_PAGE_FAULT, pc, address, false);
                Ok(None)
            }
            StepError::Bus(error) => Err(StepError::Bus(error)),
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
        bus: &mut Bus,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address = self.effective_i_address(instruction);
        let value = match instruction.funct3 {
            FUNCT_LOAD_BYTE => self.read_u8(bus, address, MemoryAccess::Load)? as i8 as i64 as u64,
            FUNCT_LOAD_HALF => {
                self.read_u16(bus, address, MemoryAccess::Load)? as i16 as i64 as u64
            }
            FUNCT_LOAD_WORD => {
                self.read_u32(bus, address, MemoryAccess::Load)? as i32 as i64 as u64
            }
            FUNCT_LOAD_DOUBLE => self.read_u64(bus, address, MemoryAccess::Load)?,
            FUNCT_LOAD_BYTE_UNSIGNED => self.read_u8(bus, address, MemoryAccess::Load)? as u64,
            FUNCT_LOAD_HALF_UNSIGNED => self.read_u16(bus, address, MemoryAccess::Load)? as u64,
            FUNCT_LOAD_WORD_UNSIGNED => self.read_u32(bus, address, MemoryAccess::Load)? as u64,
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
            FUNCT_STORE_BYTE => self.write_u8(bus, address, value as u8)?,
            FUNCT_STORE_HALF => self.write_u16(bus, address, value as u16)?,
            FUNCT_STORE_WORD => self.write_u32(bus, address, value as u32)?,
            FUNCT_STORE_DOUBLE => self.write_u64(bus, address, value)?,
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
        let alignment_mask = match instruction.funct3 {
            FUNCT_AMO_WORD => 0b011,
            FUNCT_AMO_DOUBLE => 0b111,
            _ => return Err(illegal(self.pc, instruction.raw)),
        };
        if address & alignment_mask != 0 {
            // Atomics require natural alignment (RISC-V A extension).
            return Err(if funct5 == AMO_FUNCT_LOAD_RESERVED {
                StepError::LoadAddressMisaligned {
                    pc: self.pc,
                    address,
                }
            } else {
                StepError::StoreAddressMisaligned {
                    pc: self.pc,
                    address,
                }
            });
        }
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
            let old = self.read_u32(bus, address, MemoryAccess::Load)?;
            self.reservation = Some(address);
            self.set_register(instruction.rd, sign_extend_word(old));
            return Ok(());
        }

        if funct5 == AMO_FUNCT_STORE_CONDITIONAL {
            let success = self.reservation == Some(address);
            self.reservation = None;
            if success {
                self.write_u32(bus, address, self.registers[instruction.rs2] as u32)?;
            }
            self.set_register(instruction.rd, (!success) as u64);
            return Ok(());
        }

        let old = self.read_u32(bus, address, MemoryAccess::Store)?;
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
        self.write_u32(bus, address, new)?;
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
            let old = self.read_u64(bus, address, MemoryAccess::Load)?;
            self.reservation = Some(address);
            self.set_register(instruction.rd, old);
            return Ok(());
        }

        if funct5 == AMO_FUNCT_STORE_CONDITIONAL {
            let success = self.reservation == Some(address);
            self.reservation = None;
            if success {
                self.write_u64(bus, address, self.registers[instruction.rs2])?;
            }
            self.set_register(instruction.rd, (!success) as u64);
            return Ok(());
        }

        let old = self.read_u64(bus, address, MemoryAccess::Store)?;
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
        self.write_u64(bus, address, new)?;
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
            INSTRUCTION_EBREAK if self.host_ebreak_exit => {
                Ok(Execution::Halt(HaltReason::Breakpoint {
                    code: self.registers[RETURN_VALUE_REGISTER],
                }))
            }
            INSTRUCTION_EBREAK => Err(StepError::Breakpoint { pc }),
            INSTRUCTION_ECALL => Err(StepError::EnvironmentCall { pc }),
            INSTRUCTION_MRET if self.privilege_mode == PrivilegeMode::Machine => {
                Ok(self.execute_mret())
            }
            INSTRUCTION_SRET if self.privilege_mode != PrivilegeMode::User => {
                Ok(self.execute_sret())
            }
            // The spec allows implementing wfi as a no-op; interrupts are
            // checked at the top of every step anyway.
            INSTRUCTION_WFI if self.privilege_mode != PrivilegeMode::User => {
                Ok(Execution::Continue { next_pc })
            }
            _ if instruction.raw & INSTRUCTION_SFENCE_VMA_MASK == INSTRUCTION_SFENCE_VMA
                && self.privilege_mode != PrivilegeMode::User =>
            {
                // There is no translation cache, so every page-table change is
                // already visible to the next access.
                Ok(Execution::Continue { next_pc })
            }
            _ if instruction.funct3 == FUNCT_SYSTEM_PRIVILEGED => Err(illegal(pc, instruction.raw)),
            _ => self.execute_csr(instruction, pc, next_pc),
        }
    }

    fn execute_mret(&mut self) -> Execution {
        let next_pc = self.csrs.mepc;
        let next_mode = privilege_mode_from_mpp(self.csrs.mstatus);
        let mpie = self.csrs.mstatus & MSTATUS_MPIE != 0;
        if mpie {
            self.csrs.mstatus |= MSTATUS_MIE;
        } else {
            self.csrs.mstatus &= !MSTATUS_MIE;
        }
        self.csrs.mstatus |= MSTATUS_MPIE;
        self.csrs.mstatus &= !MSTATUS_MPP_MASK;
        if next_mode != PrivilegeMode::Machine {
            self.csrs.mstatus &= !MSTATUS_MPRV;
        }
        self.privilege_mode = next_mode;
        Execution::Continue { next_pc }
    }

    fn execute_sret(&mut self) -> Execution {
        let next_pc = self.csrs.sepc;
        let next_mode = if self.csrs.mstatus & MSTATUS_SPP != 0 {
            PrivilegeMode::Supervisor
        } else {
            PrivilegeMode::User
        };
        let spie = self.csrs.mstatus & MSTATUS_SPIE != 0;
        if spie {
            self.csrs.mstatus |= MSTATUS_SIE;
        } else {
            self.csrs.mstatus &= !MSTATUS_SIE;
        }
        self.csrs.mstatus |= MSTATUS_SPIE;
        self.csrs.mstatus &= !MSTATUS_SPP;
        self.privilege_mode = next_mode;
        Execution::Continue { next_pc }
    }

    fn enter_trap(&mut self, cause: u64, epc: u64, tval: u64, interrupt: bool) {
        if self.trap_delegated_to_supervisor(cause, interrupt) {
            self.enter_supervisor_trap(cause, epc, tval, interrupt);
        } else {
            self.enter_machine_trap(cause, epc, tval, interrupt);
        }
    }

    fn trap_delegated_to_supervisor(&self, cause: u64, interrupt: bool) -> bool {
        if self.privilege_mode == PrivilegeMode::Machine {
            return false;
        }
        let delegation = if interrupt {
            self.csrs.mideleg
        } else {
            self.csrs.medeleg
        };
        delegation & (1 << cause) != 0
    }

    fn enter_machine_trap(&mut self, cause: u64, epc: u64, tval: u64, interrupt: bool) {
        self.csrs.mepc = epc & MEPC_WRITABLE_MASK;
        self.csrs.mcause = trap_cause_value(cause, interrupt);
        self.csrs.mtval = tval;

        let previous_mode = self.privilege_mode;
        let mie = self.csrs.mstatus & MSTATUS_MIE != 0;
        if mie {
            self.csrs.mstatus |= MSTATUS_MPIE;
        } else {
            self.csrs.mstatus &= !MSTATUS_MPIE;
        }
        self.csrs.mstatus &= !MSTATUS_MIE;
        self.csrs.mstatus =
            (self.csrs.mstatus & !MSTATUS_MPP_MASK) | mpp_from_privilege_mode(previous_mode);
        self.privilege_mode = PrivilegeMode::Machine;

        self.pc = trap_vector(self.csrs.mtvec, cause, interrupt);
        self.registers[ZERO_REGISTER] = 0;
    }

    fn enter_supervisor_trap(&mut self, cause: u64, epc: u64, tval: u64, interrupt: bool) {
        self.csrs.sepc = epc & MEPC_WRITABLE_MASK;
        self.csrs.scause = trap_cause_value(cause, interrupt);
        self.csrs.stval = tval;

        let previous_mode = self.privilege_mode;
        let sie = self.csrs.mstatus & MSTATUS_SIE != 0;
        if sie {
            self.csrs.mstatus |= MSTATUS_SPIE;
        } else {
            self.csrs.mstatus &= !MSTATUS_SPIE;
        }
        self.csrs.mstatus &= !MSTATUS_SIE;
        if previous_mode == PrivilegeMode::Supervisor {
            self.csrs.mstatus |= MSTATUS_SPP;
        } else {
            self.csrs.mstatus &= !MSTATUS_SPP;
        }
        self.privilege_mode = PrivilegeMode::Supervisor;

        self.pc = trap_vector(self.csrs.stvec, cause, interrupt);
        self.registers[ZERO_REGISTER] = 0;
    }

    fn refresh_interrupt_pending(&mut self, bus: &Bus) {
        self.csrs.mip &= !(MIP_MSIP | MIP_MTIP | MIP_MEIP);
        if bus.machine_software_interrupt_pending() {
            self.csrs.mip |= MIP_MSIP;
        }
        if bus.machine_timer_interrupt_pending() {
            self.csrs.mip |= MIP_MTIP;
        }
        if bus.machine_external_interrupt_pending() {
            self.csrs.mip |= MIP_MEIP;
        }
        if bus.supervisor_external_interrupt_pending() {
            self.csrs.mip |= MIP_SEIP;
        }
    }

    fn pending_interrupt(&self) -> Option<u64> {
        let pending = self.csrs.mie & self.csrs.mip;
        if self.machine_interrupts_enabled() {
            let machine_pending = pending & !self.csrs.mideleg;
            if machine_pending & MIP_MEIP != 0 {
                return Some(INTERRUPT_MACHINE_EXTERNAL);
            }
            if machine_pending & MIP_MSIP != 0 {
                return Some(INTERRUPT_MACHINE_SOFTWARE);
            }
            if machine_pending & MIP_MTIP != 0 {
                return Some(INTERRUPT_MACHINE_TIMER);
            }
        }
        if self.supervisor_interrupts_enabled() {
            let supervisor_pending = pending & self.csrs.mideleg;
            if supervisor_pending & MIP_SEIP != 0 {
                return Some(INTERRUPT_SUPERVISOR_EXTERNAL);
            }
            if supervisor_pending & MIP_SSIP != 0 {
                return Some(INTERRUPT_SUPERVISOR_SOFTWARE);
            }
            if supervisor_pending & MIP_STIP != 0 {
                return Some(INTERRUPT_SUPERVISOR_TIMER);
            }
        }
        None
    }

    fn machine_interrupts_enabled(&self) -> bool {
        self.privilege_mode != PrivilegeMode::Machine || self.csrs.mstatus & MSTATUS_MIE != 0
    }

    fn supervisor_interrupts_enabled(&self) -> bool {
        self.privilege_mode == PrivilegeMode::User
            || (self.privilege_mode == PrivilegeMode::Supervisor
                && self.csrs.mstatus & MSTATUS_SIE != 0)
    }

    fn execute_csr(
        &mut self,
        instruction: Decoded,
        pc: u64,
        next_pc: u64,
    ) -> Result<Execution, StepError> {
        let address = csr_address(instruction.raw);
        if !csr_accessible(address, self.privilege_mode) {
            return Err(illegal(pc, instruction.raw));
        }
        if let Some(counter) = counter_enable_bit(address) {
            let machine_enabled = self.csrs.mcounteren & counter != 0;
            let supervisor_enabled = self.csrs.scounteren & counter != 0;
            let enabled = match self.privilege_mode {
                PrivilegeMode::Machine => true,
                PrivilegeMode::Supervisor => machine_enabled,
                PrivilegeMode::User => machine_enabled && supervisor_enabled,
            };
            if !enabled {
                return Err(illegal(pc, instruction.raw));
            }
        }
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

    fn environment_call_cause(&self) -> u64 {
        match self.privilege_mode {
            PrivilegeMode::User => TRAP_ECALL_FROM_USER,
            PrivilegeMode::Supervisor => TRAP_ECALL_FROM_SUPERVISOR,
            PrivilegeMode::Machine => TRAP_ECALL_FROM_MACHINE,
        }
    }

    fn effective_i_address(&self, instruction: Decoded) -> u64 {
        self.registers[instruction.rs1].wrapping_add(i_immediate(instruction.raw) as u64)
    }

    fn fetch_instruction(&mut self, bus: &mut Bus, pc: u64) -> Result<Fetched, StepError> {
        let physical_pc = self.translate_address(bus, pc, MemoryAccess::Fetch)?;
        let half = bus
            .read_u16(physical_pc)
            .map_err(|error| instruction_access_fault(pc, error))?;
        if half & 0b11 == 0b11 {
            // The upper half may live on a different page, so translate it
            // separately instead of assuming physical contiguity.
            let upper_pc = pc.wrapping_add(COMPRESSED_INSTRUCTION_SIZE);
            let physical_upper = self.translate_address(bus, upper_pc, MemoryAccess::Fetch)?;
            let upper = bus
                .read_u16(physical_upper)
                .map_err(|error| instruction_access_fault(pc, error))?;
            Ok(Fetched {
                raw: u32::from(half) | (u32::from(upper) << 16),
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

    fn read_u8(
        &mut self,
        bus: &mut Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<u8, StepError> {
        let physical = self.translate_address(bus, address, access)?;
        bus.read_u8(physical)
            .map_err(|error| access_fault(self.pc, address, access, error))
    }

    fn read_u16(
        &mut self,
        bus: &mut Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<u16, StepError> {
        let physical = self.translate_address(bus, address, access)?;
        bus.read_u16(physical)
            .map_err(|error| access_fault(self.pc, address, access, error))
    }

    fn read_u32(
        &mut self,
        bus: &mut Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<u32, StepError> {
        let physical = self.translate_address(bus, address, access)?;
        bus.read_u32(physical)
            .map_err(|error| access_fault(self.pc, address, access, error))
    }

    fn read_u64(
        &mut self,
        bus: &mut Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<u64, StepError> {
        let physical = self.translate_address(bus, address, access)?;
        bus.read_u64(physical)
            .map_err(|error| access_fault(self.pc, address, access, error))
    }

    fn write_u8(&mut self, bus: &mut Bus, address: u64, value: u8) -> Result<(), StepError> {
        let physical = self.translate_address(bus, address, MemoryAccess::Store)?;
        bus.write_u8(physical, value)
            .map_err(|error| store_access_fault(self.pc, error))
    }

    fn write_u16(&mut self, bus: &mut Bus, address: u64, value: u16) -> Result<(), StepError> {
        let physical = self.translate_address(bus, address, MemoryAccess::Store)?;
        bus.write_u16(physical, value)
            .map_err(|error| store_access_fault(self.pc, error))
    }

    fn write_u32(&mut self, bus: &mut Bus, address: u64, value: u32) -> Result<(), StepError> {
        let physical = self.translate_address(bus, address, MemoryAccess::Store)?;
        bus.write_u32(physical, value)
            .map_err(|error| store_access_fault(self.pc, error))
    }

    fn write_u64(&mut self, bus: &mut Bus, address: u64, value: u64) -> Result<(), StepError> {
        let physical = self.translate_address(bus, address, MemoryAccess::Store)?;
        bus.write_u64(physical, value)
            .map_err(|error| store_access_fault(self.pc, error))
    }

    fn translate_address(
        &mut self,
        bus: &mut Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<u64, StepError> {
        let translation = self.translate_address_inner(bus, address, access)?;
        if let Some((pte_address, pte)) = translation.pte_update {
            bus.write_u64(pte_address, pte)
                .map_err(|error| access_fault(self.pc, address, access, error))?;
        }
        Ok(translation.physical_address)
    }

    fn translate_address_inner(
        &self,
        bus: &Bus,
        address: u64,
        access: MemoryAccess,
    ) -> Result<AddressTranslation, StepError> {
        let mode = self.csrs.satp >> SATP_MODE_SHIFT;
        let effective_mode = self.effective_privilege_mode(access);
        if effective_mode == PrivilegeMode::Machine || mode == SATP_MODE_BARE {
            return Ok(AddressTranslation {
                physical_address: address,
                paging_active: false,
                pte_update: None,
            });
        }
        if mode != SATP_MODE_SV39 || !sv39_canonical(address) {
            return Err(page_fault(self.pc, address, access));
        }

        let vpn = [
            (address >> 12) & VPN_MASK,
            (address >> 21) & VPN_MASK,
            (address >> 30) & VPN_MASK,
        ];
        let mut table = (self.csrs.satp & SATP_PPN_MASK) << PAGE_SHIFT;
        for level in (0..SV39_LEVELS).rev() {
            let pte_address = table.wrapping_add(vpn[level].wrapping_mul(PTE_SIZE));
            let pte = bus
                .peek_u64(pte_address)
                .map_err(|error| access_fault(self.pc, address, access, error))?;
            if pte & PTE_V == 0 || (pte & PTE_W != 0 && pte & PTE_R == 0) {
                return Err(page_fault(self.pc, address, access));
            }
            if pte & (PTE_R | PTE_X) == 0 {
                table = ((pte >> PTE_PPN_SHIFT) & PTE_PPN_MASK) << PAGE_SHIFT;
                continue;
            }
            if !pte_allows_access(pte, effective_mode, access, self.csrs.mstatus)
                || !superpage_aligned(pte, level)
            {
                return Err(page_fault(self.pc, address, access));
            }
            let accessed_dirty = PTE_A
                | if matches!(access, MemoryAccess::Store) {
                    PTE_D
                } else {
                    0
                };
            return Ok(AddressTranslation {
                physical_address: sv39_physical_address(pte, address, level),
                paging_active: true,
                pte_update: (pte & accessed_dirty != accessed_dirty)
                    .then_some((pte_address, pte | accessed_dirty)),
            });
        }
        Err(page_fault(self.pc, address, access))
    }

    fn effective_privilege_mode(&self, access: MemoryAccess) -> PrivilegeMode {
        if self.privilege_mode == PrivilegeMode::Machine
            && !matches!(access, MemoryAccess::Fetch)
            && self.csrs.mstatus & MSTATUS_MPRV != 0
        {
            privilege_mode_from_mpp(self.csrs.mstatus)
        } else {
            self.privilege_mode
        }
    }

    fn clear_reservation_for_store(&mut self, address: u64) {
        if self.reservation == Some(address) {
            self.reservation = None;
        }
    }
}

fn csr_accessible(address: u16, mode: PrivilegeMode) -> bool {
    privilege_mode_rank(mode) >= u8::try_from((address >> 8) & 0b11).unwrap()
}

fn counter_enable_bit(address: u16) -> Option<u64> {
    match address {
        CSR_CYCLE => Some(COUNTEREN_CYCLE),
        CSR_TIME => Some(COUNTEREN_TIME),
        CSR_INSTRET => Some(COUNTEREN_INSTRET),
        _ => None,
    }
}

fn privilege_mode_rank(mode: PrivilegeMode) -> u8 {
    match mode {
        PrivilegeMode::User => 0,
        PrivilegeMode::Supervisor => 1,
        PrivilegeMode::Machine => 3,
    }
}

fn privilege_mode_from_mpp(mstatus: u64) -> PrivilegeMode {
    match mstatus & MSTATUS_MPP_MASK {
        MSTATUS_MPP_USER => PrivilegeMode::User,
        MSTATUS_MPP_SUPERVISOR => PrivilegeMode::Supervisor,
        _ => PrivilegeMode::Machine,
    }
}

fn mpp_from_privilege_mode(mode: PrivilegeMode) -> u64 {
    match mode {
        PrivilegeMode::User => MSTATUS_MPP_USER,
        PrivilegeMode::Supervisor => MSTATUS_MPP_SUPERVISOR,
        PrivilegeMode::Machine => MSTATUS_MPP_MACHINE,
    }
}

fn trap_cause_value(cause: u64, interrupt: bool) -> u64 {
    if interrupt {
        INTERRUPT_BIT | cause
    } else {
        cause
    }
}

fn trap_vector(mtvec: u64, cause: u64, interrupt: bool) -> u64 {
    let base = mtvec & !MTVEC_MODE_MASK;
    let mode = mtvec & MTVEC_MODE_MASK;
    if interrupt && mode == 1 {
        base.wrapping_add(cause.wrapping_mul(4))
    } else {
        base
    }
}

fn bus_error_address(error: &BusError) -> u64 {
    match error {
        BusError::Unmapped { address, .. } | BusError::Stub { address, .. } => *address,
    }
}

fn instruction_access_fault(pc: u64, error: BusError) -> StepError {
    StepError::InstructionAccessFault {
        pc,
        address: bus_error_address(&error),
    }
}

fn load_access_fault(pc: u64, error: BusError) -> StepError {
    StepError::LoadAccessFault {
        pc,
        address: bus_error_address(&error),
    }
}

fn store_access_fault(pc: u64, error: BusError) -> StepError {
    StepError::StoreAccessFault {
        pc,
        address: bus_error_address(&error),
    }
}

fn access_fault(pc: u64, _address: u64, access: MemoryAccess, error: BusError) -> StepError {
    match access {
        MemoryAccess::Fetch => instruction_access_fault(pc, error),
        MemoryAccess::Load => load_access_fault(pc, error),
        MemoryAccess::Store => store_access_fault(pc, error),
    }
}

fn page_fault(pc: u64, address: u64, access: MemoryAccess) -> StepError {
    match access {
        MemoryAccess::Fetch => StepError::InstructionPageFault { pc, address },
        MemoryAccess::Load => StepError::LoadPageFault { pc, address },
        MemoryAccess::Store => StepError::StorePageFault { pc, address },
    }
}

fn sv39_canonical(address: u64) -> bool {
    sign_extend_u64(address, SV39_VA_BITS) == address
}

fn pte_ppn(pte: u64, index: usize) -> u64 {
    (pte >> (PTE_PPN_SHIFT + index as u64 * 9)) & VPN_MASK
}

fn pte_allows_access(pte: u64, mode: PrivilegeMode, access: MemoryAccess, mstatus: u64) -> bool {
    if mode == PrivilegeMode::User && pte & PTE_U == 0 {
        return false;
    }
    if mode == PrivilegeMode::Supervisor
        && pte & PTE_U != 0
        && (matches!(access, MemoryAccess::Fetch) || mstatus & MSTATUS_SUM == 0)
    {
        return false;
    }
    match access {
        MemoryAccess::Fetch => pte & PTE_X != 0,
        MemoryAccess::Load => pte & PTE_R != 0 || (mstatus & MSTATUS_MXR != 0 && pte & PTE_X != 0),
        MemoryAccess::Store => pte & (PTE_R | PTE_W) == PTE_R | PTE_W,
    }
}

fn superpage_aligned(pte: u64, level: usize) -> bool {
    (0..level).all(|index| pte_ppn(pte, index) == 0)
}

fn sv39_physical_address(pte: u64, address: u64, level: usize) -> u64 {
    let ppn = (pte >> PTE_PPN_SHIFT) & PTE_PPN_MASK;
    let page_offset = address & PAGE_OFFSET_MASK;
    let vpn0 = (address >> 12) & VPN_MASK;
    let vpn1 = (address >> 21) & VPN_MASK;
    match level {
        0 => (ppn << PAGE_SHIFT) | page_offset,
        1 => (ppn & !VPN_MASK) << PAGE_SHIFT | (vpn0 << PAGE_SHIFT) | page_offset,
        SV39_TOP_LEVEL => {
            (ppn & !(VPN_MASK | (VPN_MASK << 9))) << PAGE_SHIFT
                | (vpn1 << 21)
                | (vpn0 << PAGE_SHIFT)
                | page_offset
        }
        _ => unreachable!(),
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
                // c.slli with rd=0 is a HINT; execute as a no-op.
                Ok(INSTRUCTION_NOP)
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
    } else {
        let immediate = c_lui_immediate(raw);
        if immediate == 0 {
            // c.lui with a zero immediate is reserved.
            Err(raw)
        } else if rd == 0 {
            // c.lui with rd=0 (and nonzero immediate) is a HINT; no-op.
            Ok(INSTRUCTION_NOP)
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
        // c.mv with rd=0 and rs2!=0 is a HINT; rs2=0 there is reserved.
        (0, 0, 0) => Err(raw),
        (0, 0, _) => Ok(INSTRUCTION_NOP),
        (0, _, 0) => Ok(encode_i(0, rd_rs1, FUNCT_ADD, 0, OPCODE_JALR)),
        (0, _, _) => Ok(encode_r(FUNCT7_BASE, rs2, 0, FUNCT_ADD, rd_rs1, OPCODE_OP)),
        (1, 0, 0) => Ok(INSTRUCTION_EBREAK),
        (1, _, 0) => Ok(encode_i(0, rd_rs1, FUNCT_ADD, 1, OPCODE_JALR)),
        // c.add with rd=0 and rs2!=0 is a HINT; execute as a no-op.
        (1, 0, _) => Ok(INSTRUCTION_NOP),
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
    (((raw >> 10) & 0x7) << 3) | (((raw >> 6) & 1) << 2) | (((raw >> 5) & 1) << 6)
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
            CSR_MEDELEG => Some(self.medeleg),
            CSR_MIDELEG => Some(self.mideleg),
            CSR_MIE => Some(self.mie),
            CSR_MTVEC => Some(self.mtvec),
            CSR_MCOUNTEREN => Some(self.mcounteren),
            CSR_MSCRATCH => Some(self.mscratch),
            CSR_MEPC => Some(self.mepc),
            CSR_MCAUSE => Some(self.mcause),
            CSR_MTVAL => Some(self.mtval),
            CSR_MIP => Some(self.mip),
            CSR_SSTATUS => Some(self.mstatus & SSTATUS_WRITABLE_MASK),
            CSR_SIE => Some(self.mie & S_INTERRUPT_MASK),
            CSR_STVEC => Some(self.stvec),
            CSR_SCOUNTEREN => Some(self.scounteren),
            CSR_SSCRATCH => Some(self.sscratch),
            CSR_SEPC => Some(self.sepc),
            CSR_SCAUSE => Some(self.scause),
            CSR_STVAL => Some(self.stval),
            CSR_SIP => Some(self.mip & S_INTERRUPT_MASK),
            CSR_SATP => Some(self.satp),
            CSR_CYCLE => Some(self.cycle),
            CSR_TIME => Some(self.cycle),
            CSR_INSTRET => Some(self.instret),
            _ => None,
        }
    }

    fn write(&mut self, address: u16, value: u64) -> Option<()> {
        match address {
            CSR_MSTATUS => self.mstatus = legalize_mstatus(self.mstatus, value),
            CSR_MEDELEG => self.medeleg = value & MEDELEG_WRITABLE_MASK,
            CSR_MIDELEG => self.mideleg = value & MIDELEG_WRITABLE_MASK,
            CSR_MIE => self.mie = value & MIE_WRITABLE_MASK,
            CSR_MTVEC => self.mtvec = value & MTVEC_WRITABLE_MASK,
            CSR_MCOUNTEREN => self.mcounteren = value & COUNTEREN_WRITABLE_MASK,
            CSR_MSCRATCH => self.mscratch = value & MSCRATCH_WRITABLE_MASK,
            CSR_MEPC => self.mepc = value & MEPC_WRITABLE_MASK,
            CSR_MCAUSE => self.mcause = value & MCAUSE_WRITABLE_MASK,
            CSR_MTVAL => self.mtval = value & MTVAL_WRITABLE_MASK,
            CSR_MIP => self.mip = value & MIP_WRITABLE_MASK,
            CSR_SSTATUS => {
                self.mstatus =
                    (self.mstatus & !SSTATUS_WRITABLE_MASK) | (value & SSTATUS_WRITABLE_MASK)
            }
            CSR_SIE => self.mie = (self.mie & !S_INTERRUPT_MASK) | (value & S_INTERRUPT_MASK),
            CSR_STVEC => self.stvec = value & MTVEC_WRITABLE_MASK,
            CSR_SCOUNTEREN => self.scounteren = value & COUNTEREN_WRITABLE_MASK,
            CSR_SSCRATCH => self.sscratch = value & MSCRATCH_WRITABLE_MASK,
            CSR_SEPC => self.sepc = value & MEPC_WRITABLE_MASK,
            CSR_SCAUSE => self.scause = value & MCAUSE_WRITABLE_MASK,
            CSR_STVAL => self.stval = value & MTVAL_WRITABLE_MASK,
            CSR_SIP => self.mip = (self.mip & !S_INTERRUPT_MASK) | (value & MIP_WRITABLE_MASK),
            CSR_SATP => self.satp = write_satp(value),
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

fn legalize_mstatus(previous: u64, value: u64) -> u64 {
    let mut next = value & MSTATUS_WRITABLE_MASK;
    // MPP is a WARL field; 0b10 is reserved, so retain the previous value.
    if next & MSTATUS_MPP_MASK == MSTATUS_MPP_RESERVED {
        next = (next & !MSTATUS_MPP_MASK) | (previous & MSTATUS_MPP_MASK);
    }
    next
}

fn write_satp(value: u64) -> u64 {
    match value >> SATP_MODE_SHIFT {
        SATP_MODE_BARE => 0,
        SATP_MODE_SV39 => {
            value & ((SATP_MODE_SV39 << SATP_MODE_SHIFT) | SATP_ASID_MASK | SATP_PPN_MASK)
        }
        _ => 0,
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

#[doc(hidden)]
pub fn sign_extend_word(value: u32) -> u64 {
    value as i32 as i64 as u64
}

#[doc(hidden)]
pub fn mulh(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as i64 as i128)) >> XLEN_BITS) as u64
}

#[doc(hidden)]
pub fn mulhsu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as u128 as i128)) >> XLEN_BITS) as u64
}

#[doc(hidden)]
pub fn mulhu(lhs: u64, rhs: u64) -> u64 {
    (((lhs as u128) * (rhs as u128)) >> XLEN_BITS) as u64
}

#[doc(hidden)]
pub fn div(lhs: u64, rhs: u64) -> u64 {
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

#[doc(hidden)]
pub fn divu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        u64::MAX
    } else {
        lhs / rhs
    }
}

#[doc(hidden)]
pub fn rem(lhs: u64, rhs: u64) -> u64 {
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

#[doc(hidden)]
pub fn remu(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

#[doc(hidden)]
pub fn divw(lhs: u32, rhs: u32) -> u32 {
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

#[doc(hidden)]
pub fn divuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        u32::MAX
    } else {
        lhs / rhs
    }
}

#[doc(hidden)]
pub fn remw(lhs: u32, rhs: u32) -> u32 {
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

#[doc(hidden)]
pub fn remuw(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

#[doc(hidden)]
pub fn upper_immediate(instruction: u32) -> u64 {
    sign_extend_u64(
        (instruction & UPPER_IMMEDIATE_MASK) as u64,
        U_IMMEDIATE_BITS,
    )
}

#[doc(hidden)]
pub fn i_immediate(instruction: u32) -> i64 {
    sign_extend(
        bits(instruction, I_IMMEDIATE_SHIFT, I_IMMEDIATE_BITS),
        I_IMMEDIATE_BITS,
    )
}

#[doc(hidden)]
pub fn s_immediate(instruction: u32) -> i64 {
    let low = bits(instruction, S_IMMEDIATE_LOW_SHIFT, S_IMMEDIATE_LOW_BITS);
    let high = bits(instruction, S_IMMEDIATE_HIGH_SHIFT, S_IMMEDIATE_HIGH_BITS);
    sign_extend((high << S_IMMEDIATE_LOW_BITS) | low, I_IMMEDIATE_BITS)
}

#[doc(hidden)]
pub fn b_immediate(instruction: u32) -> i64 {
    let value = ((instruction >> 31) << 12)
        | (((instruction >> 7) & 1) << 11)
        | (((instruction >> 25) & 0x3f) << 5)
        | (((instruction >> 8) & 0xf) << 1);
    sign_extend(value, B_IMMEDIATE_BITS)
}

#[doc(hidden)]
pub fn j_immediate(instruction: u32) -> i64 {
    let value = ((instruction >> 31) << 20)
        | (((instruction >> 12) & 0xff) << 12)
        | (((instruction >> 20) & 1) << 11)
        | (((instruction >> 21) & 0x3ff) << 1);
    sign_extend(value, J_IMMEDIATE_BITS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{DRAM_START, VIRTIO_START};

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

        assert_illegal_instruction_trap(&mut cpu, &mut bus, instruction);
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
    fn satp_accepts_sv39_and_rejects_unsupported_modes() {
        let mut cpu = Cpu::new(DRAM_START);
        let sv39 = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | 0x1234;

        cpu.csrs.write(CSR_SATP, sv39).unwrap();
        assert_eq!(cpu.csr(CSR_SATP), sv39);

        cpu.csrs.write(CSR_SATP, u64::MAX).unwrap();
        assert_eq!(cpu.csr(CSR_SATP), 0);
    }

    #[test]
    fn sv39_translates_supervisor_fetches_and_loads() {
        let mut cpu = Cpu::new(0x1000);
        let mut bus = Bus::new(0x8000);
        let root = DRAM_START + 0x1000;
        let level1 = DRAM_START + 0x2000;
        let level0 = DRAM_START + 0x3000;
        let data = DRAM_START + 0x4000;

        install_sv39_table(&mut bus, root, level1, level0);
        install_pte(&mut bus, level0, 1, DRAM_START, PTE_V | PTE_R | PTE_X);
        install_pte(&mut bus, level0, 2, data, PTE_V | PTE_R);
        bus.write_u32(
            DRAM_START,
            encode_i(0, 1, FUNCT_LOAD_DOUBLE, 5, OPCODE_LOAD),
        )
        .unwrap();
        bus.write_u64(data, 0xfeed_face_cafe_beef).unwrap();
        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.satp = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | (root >> PAGE_SHIFT);
        cpu.set_register(1, 0x2000);

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(5), 0xfeed_face_cafe_beef);
        assert_eq!(cpu.pc, 0x1000 + INSTRUCTION_SIZE);
        assert_ne!(bus.read_u64(level0 + PTE_SIZE).unwrap() & PTE_A, 0);
        assert_ne!(bus.read_u64(level0 + PTE_SIZE * 2).unwrap() & PTE_A, 0);
    }

    #[test]
    fn sv39_load_page_fault_enters_trap_with_virtual_address() {
        let mut cpu = Cpu::new(0x1000);
        let mut bus = Bus::new(0x8000);
        let root = DRAM_START + 0x1000;
        let level1 = DRAM_START + 0x2000;
        let level0 = DRAM_START + 0x3000;
        let vector = DRAM_START + 0x5000;

        install_sv39_table(&mut bus, root, level1, level0);
        install_pte(&mut bus, level0, 1, DRAM_START, PTE_V | PTE_R | PTE_X);
        bus.write_u32(
            DRAM_START,
            encode_i(0, 1, FUNCT_LOAD_DOUBLE, 5, OPCODE_LOAD),
        )
        .unwrap();
        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.satp = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | (root >> PAGE_SHIFT);
        cpu.csrs.mtvec = vector;
        cpu.set_register(1, 0x2000);

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.privilege_mode(), PrivilegeMode::Machine);
        assert_eq!(cpu.csr(CSR_MEPC), 0x1000);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_LOAD_PAGE_FAULT);
        assert_eq!(cpu.csr(CSR_MTVAL), 0x2000);
        assert_eq!(cpu.register(5), 0);
    }

    #[test]
    fn sv39_honors_sum_and_mxr_for_supervisor_loads() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(0x8000);
        let root = DRAM_START + 0x1000;
        let level1 = DRAM_START + 0x2000;
        let level0 = DRAM_START + 0x3000;
        let data = DRAM_START + 0x4000;
        install_sv39_table(&mut bus, root, level1, level0);
        install_pte(&mut bus, level0, 1, data, PTE_V | PTE_U | PTE_X);
        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.satp = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | (root >> PAGE_SHIFT);

        assert!(cpu
            .translate_address_inner(&bus, 0x1000, MemoryAccess::Load)
            .is_err());
        cpu.csrs.mstatus = MSTATUS_SUM | MSTATUS_MXR;
        assert_eq!(
            cpu.translate_address_inner(&bus, 0x1000, MemoryAccess::Load)
                .unwrap()
                .physical_address,
            data
        );
        assert!(cpu
            .translate_address_inner(&bus, 0x1000, MemoryAccess::Fetch)
            .is_err());
    }

    #[test]
    fn mprv_uses_mpp_for_machine_data_accesses_only() {
        let mut cpu = Cpu::new(0x1000);
        let mut bus = Bus::new(0x8000);
        let root = DRAM_START + 0x1000;
        let level1 = DRAM_START + 0x2000;
        let level0 = DRAM_START + 0x3000;
        let data = DRAM_START + 0x4000;
        install_sv39_table(&mut bus, root, level1, level0);
        install_pte(&mut bus, level0, 1, data, PTE_V | PTE_R);
        cpu.csrs.satp = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | (root >> PAGE_SHIFT);
        cpu.csrs.mstatus = MSTATUS_MPRV | MSTATUS_MPP_SUPERVISOR;

        assert_eq!(
            cpu.translate_address_inner(&bus, 0x1000, MemoryAccess::Load)
                .unwrap()
                .physical_address,
            data
        );
        assert_eq!(
            cpu.translate_address_inner(&bus, 0x1000, MemoryAccess::Fetch)
                .unwrap()
                .physical_address,
            0x1000
        );
    }

    #[test]
    fn rv64a_lr_requires_zero_rs2() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        let instruction = encode_amo(AMO_FUNCT_LOAD_RESERVED, 1, 0, FUNCT_AMO_WORD, 5);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_illegal_instruction_trap(&mut cpu, &mut bus, instruction);
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
    fn supervisor_csr_aliases_update_only_supervisor_state() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(32);
        cpu.set_register(1, u64::MAX);
        bus.write_u32(
            DRAM_START,
            encode_csr(u32::from(CSR_SSTATUS), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE,
            encode_csr(u32::from(CSR_SIE), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 2,
            encode_csr(u32::from(CSR_SIP), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();
        bus.write_u32(
            DRAM_START + INSTRUCTION_SIZE * 3,
            encode_csr(u32::from(CSR_SATP), FUNCT_CSRRW, 1, 0),
        )
        .unwrap();

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.csr(CSR_SSTATUS), SSTATUS_WRITABLE_MASK);
        assert_eq!(cpu.csr(CSR_MSTATUS) & MSTATUS_MIE, 0);
        assert_eq!(cpu.csr(CSR_SIE), S_INTERRUPT_MASK);
        assert_eq!(cpu.csr(CSR_MIE), S_INTERRUPT_MASK);
        assert_eq!(cpu.csr(CSR_SIP), S_INTERRUPT_MASK);
        assert_eq!(cpu.csr(CSR_MIP), S_INTERRUPT_MASK);
        assert_eq!(cpu.csr(CSR_SATP), 0);
    }

    #[test]
    fn delegated_user_ecall_enters_supervisor_trap_vector() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.privilege_mode = PrivilegeMode::User;
        cpu.csrs.stvec = vector;
        cpu.csrs.medeleg = 1 << TRAP_ECALL_FROM_USER;
        cpu.csrs.mstatus = MSTATUS_SIE;
        bus.write_u32(DRAM_START, INSTRUCTION_ECALL).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.privilege_mode(), PrivilegeMode::Supervisor);
        assert_eq!(cpu.csr(CSR_SEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_SCAUSE), TRAP_ECALL_FROM_USER);
        assert_eq!(cpu.csr(CSR_STVAL), 0);
        assert_eq!(cpu.csr(CSR_SSTATUS) & MSTATUS_SIE, 0);
        assert_ne!(cpu.csr(CSR_SSTATUS) & MSTATUS_SPIE, 0);
        assert_eq!(cpu.csr(CSR_SSTATUS) & MSTATUS_SPP, 0);
        assert_eq!(cpu.csr(CSR_MEPC), 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    #[test]
    fn sret_restores_sepc_interrupt_enable_and_privilege_mode() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.sepc = DRAM_START + 8;
        cpu.csrs.mstatus = MSTATUS_SPIE;
        bus.write_u32(DRAM_START, INSTRUCTION_SRET).unwrap();

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, DRAM_START + 8);
        assert_eq!(cpu.privilege_mode(), PrivilegeMode::User);
        assert_ne!(cpu.csr(CSR_SSTATUS) & MSTATUS_SIE, 0);
        assert_ne!(cpu.csr(CSR_SSTATUS) & MSTATUS_SPIE, 0);
        assert_eq!(cpu.csr(CSR_SSTATUS) & MSTATUS_SPP, 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 1);
    }

    #[test]
    fn lower_privilege_machine_csr_access_traps_as_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        let vector = DRAM_START + 8;
        let instruction = encode_csr(u32::from(CSR_MSCRATCH), FUNCT_CSRRS, 0, 5);
        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.mtvec = vector;
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.privilege_mode(), PrivilegeMode::Machine);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_ILLEGAL_INSTRUCTION);
        assert_eq!(cpu.csr(CSR_MTVAL), u64::from(instruction));
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
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
    fn compressed_lw_and_sw_support_offsets_of_64_and_above() {
        // c.lw a0, 64(a1): offset[6] comes from instruction bit 5.
        // funct3=010, imm[5:3]=000 (bits 12:10), rs1'=a1(3), imm[2]=0, imm[6]=1, rd'=a0(2)
        #[allow(clippy::unusual_byte_groupings)] // grouped by encoding fields
        let c_lw = 0b010_000_011_0_1_010_00;
        assert_eq!(
            decode_compressed_instruction(c_lw),
            Some(encode_i(64, 11, FUNCT_LOAD_WORD, 10, OPCODE_LOAD))
        );
        // c.sw a0, 64(a1)
        #[allow(clippy::unusual_byte_groupings)]
        let c_sw = 0b110_000_011_0_1_010_00;
        assert_eq!(
            decode_compressed_instruction(c_sw),
            Some(encode_s(64, 10, 11, FUNCT_STORE_WORD))
        );
    }

    #[test]
    fn fetch_translates_each_half_of_a_page_straddling_instruction() {
        // Map virtual pages 1 and 2 to non-contiguous physical pages, place a
        // 4-byte instruction across the boundary, and check both halves come
        // from their own mappings.
        let mut cpu = Cpu::new(0x1000 + PAGE_SIZE - 2);
        let mut bus = Bus::new(0x10000);
        let root = DRAM_START + 0x1000;
        let level1 = DRAM_START + 0x2000;
        let level0 = DRAM_START + 0x3000;
        let low_page = DRAM_START + 0x4000;
        let high_page = DRAM_START + 0x6000;

        install_sv39_table(&mut bus, root, level1, level0);
        install_pte(&mut bus, level0, 1, low_page, PTE_V | PTE_R | PTE_X);
        install_pte(&mut bus, level0, 2, high_page, PTE_V | PTE_R | PTE_X);

        let instruction = 0x0010_0093_u32; // addi x1, x0, 1
        bus.write_u16(low_page + PAGE_SIZE - 2, instruction as u16)
            .unwrap();
        bus.write_u16(high_page, (instruction >> 16) as u16)
            .unwrap();

        cpu.privilege_mode = PrivilegeMode::Supervisor;
        cpu.csrs.satp = (SATP_MODE_SV39 << SATP_MODE_SHIFT) | (root >> PAGE_SHIFT);

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.register(1), 1);
        assert_eq!(cpu.pc, 0x1000 + PAGE_SIZE + 2);
    }

    #[test]
    fn mstatus_rejects_reserved_mpp_encoding() {
        let mut cpu = Cpu::new(DRAM_START);
        cpu.csrs.write(CSR_MSTATUS, MSTATUS_MPP_SUPERVISOR).unwrap();
        cpu.csrs.write(CSR_MSTATUS, MSTATUS_MPP_RESERVED).unwrap();
        assert_eq!(
            cpu.csr(CSR_MSTATUS) & MSTATUS_MPP_MASK,
            MSTATUS_MPP_SUPERVISOR
        );
    }

    #[test]
    fn misaligned_amo_raises_store_address_misaligned_trap() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(64);
        let vector = DRAM_START + 32;
        cpu.csrs.mtvec = vector;
        cpu.set_register(1, DRAM_START + 41); // not 8-byte aligned
        bus.write_u32(
            DRAM_START,
            encode_amo(AMO_FUNCT_ADD, 2, 1, FUNCT_AMO_DOUBLE, 5),
        )
        .unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_STORE_ADDRESS_MISALIGNED);
        assert_eq!(cpu.csr(CSR_MTVAL), DRAM_START + 41);
    }

    #[test]
    fn misaligned_lr_raises_load_address_misaligned_trap() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(64);
        let vector = DRAM_START + 32;
        cpu.csrs.mtvec = vector;
        cpu.set_register(1, DRAM_START + 42); // not 4-byte aligned
        bus.write_u32(
            DRAM_START,
            encode_amo(AMO_FUNCT_LOAD_RESERVED, 0, 1, FUNCT_AMO_WORD, 5),
        )
        .unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_LOAD_ADDRESS_MISALIGNED);
        assert_eq!(cpu.csr(CSR_MTVAL), DRAM_START + 42);
    }

    #[test]
    fn wfi_is_a_noop_in_machine_mode() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        bus.write_u32(DRAM_START, INSTRUCTION_WFI).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));
        assert_eq!(cpu.pc, DRAM_START + INSTRUCTION_SIZE);
        assert_eq!(cpu.csr(CSR_INSTRET), 1);
    }

    #[test]
    fn wfi_in_user_mode_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        cpu.privilege_mode = PrivilegeMode::User;
        bus.write_u32(DRAM_START, INSTRUCTION_WFI).unwrap();

        assert_illegal_instruction_trap(&mut cpu, &mut bus, INSTRUCTION_WFI);
    }

    #[test]
    fn sfence_vma_is_a_validated_noop_without_a_tlb() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let sfence_with_operands = INSTRUCTION_SFENCE_VMA | (3 << RS1_SHIFT) | (4 << RS2_SHIFT);
        bus.write_u32(DRAM_START, sfence_with_operands).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));
        assert_eq!(cpu.pc, DRAM_START + INSTRUCTION_SIZE);
        assert_eq!(cpu.csr(CSR_INSTRET), 1);
    }

    #[test]
    fn sfence_vma_in_user_mode_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        cpu.privilege_mode = PrivilegeMode::User;
        bus.write_u32(DRAM_START, INSTRUCTION_SFENCE_VMA).unwrap();

        assert_illegal_instruction_trap(&mut cpu, &mut bus, INSTRUCTION_SFENCE_VMA);
    }

    #[test]
    fn compressed_hint_encodings_expand_to_nop() {
        // c.slli x0, 1
        assert_eq!(decode_compressed_instruction(0x0006), Some(INSTRUCTION_NOP));
        // c.lui x0, 1
        assert_eq!(decode_compressed_instruction(0x6005), Some(INSTRUCTION_NOP));
        // c.mv x0, x5
        assert_eq!(decode_compressed_instruction(0x8016), Some(INSTRUCTION_NOP));
        // c.add x0, x5
        assert_eq!(decode_compressed_instruction(0x9016), Some(INSTRUCTION_NOP));
        // Reserved encodings stay illegal: c.mv x0, x0 alias (jr x0) and c.lui x0, 0.
        assert_eq!(decode_compressed_instruction(0x8002), None);
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
    fn ecall_enters_machine_trap_vector() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.csrs.mtvec = vector;
        cpu.csrs.mstatus = MSTATUS_MIE;
        bus.write_u32(DRAM_START, INSTRUCTION_ECALL).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_ECALL_FROM_MACHINE);
        assert_eq!(cpu.csr(CSR_MTVAL), 0);
        assert_eq!(cpu.csr(CSR_MSTATUS) & MSTATUS_MIE, 0);
        assert_ne!(cpu.csr(CSR_MSTATUS) & MSTATUS_MPIE, 0);
        assert_eq!(cpu.csr(CSR_MSTATUS) & MSTATUS_MPP_MASK, MSTATUS_MPP_MACHINE);
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    #[test]
    fn ebreak_can_enter_architectural_breakpoint_trap() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.set_host_ebreak_exit(false);
        cpu.csrs.mtvec = vector;
        bus.write_u32(DRAM_START, INSTRUCTION_EBREAK).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));
        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_BREAKPOINT);
        assert_eq!(cpu.csr(CSR_MTVAL), 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    #[test]
    fn load_access_fault_enters_machine_trap_vector() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        let vector = DRAM_START + 8;
        cpu.csrs.mtvec = vector;
        cpu.set_register(1, VIRTIO_START);
        bus.write_u32(DRAM_START, encode_i(0, 1, FUNCT_LOAD_WORD, 5, OPCODE_LOAD))
            .unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_LOAD_ACCESS_FAULT);
        assert_eq!(cpu.csr(CSR_MTVAL), VIRTIO_START);
        assert_eq!(cpu.register(5), 0);
    }

    #[test]
    fn cpu_starts_in_machine_mode() {
        assert_eq!(
            Cpu::new(DRAM_START).privilege_mode(),
            PrivilegeMode::Machine
        );
    }

    #[test]
    fn mret_restores_mepc_interrupt_enable_and_privilege_mode() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(16);
        cpu.csrs.mepc = DRAM_START + 8;
        cpu.csrs.mstatus = MSTATUS_MPIE | MSTATUS_MPP_MACHINE;
        bus.write_u32(DRAM_START, INSTRUCTION_MRET).unwrap();

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, DRAM_START + 8);
        assert_eq!(cpu.privilege_mode(), PrivilegeMode::Machine);
        assert_ne!(cpu.csr(CSR_MSTATUS) & MSTATUS_MIE, 0);
        assert_ne!(cpu.csr(CSR_MSTATUS) & MSTATUS_MPIE, 0);
        assert_eq!(cpu.csr(CSR_MSTATUS) & MSTATUS_MPP_MASK, 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 1);
    }

    #[test]
    fn enabled_machine_timer_interrupt_enters_trap_before_fetch() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.csrs.mtvec = vector;
        cpu.csrs.mstatus = MSTATUS_MIE;
        cpu.csrs.mie = MIP_MTIP;
        bus.write_u64(crate::bus::CLINT_START + 0x4000, 0).unwrap();
        bus.write_u32(DRAM_START, 0x0010_8093).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), INTERRUPT_BIT | INTERRUPT_MACHINE_TIMER);
        assert_eq!(cpu.csr(CSR_MTVAL), 0);
        assert_eq!(cpu.csr(CSR_MIP) & MIP_MTIP, MIP_MTIP);
        assert_eq!(cpu.register(1), 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    #[test]
    fn enabled_machine_external_interrupt_enters_trap_before_fetch() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.csrs.mtvec = vector;
        cpu.csrs.mstatus = MSTATUS_MIE;
        cpu.csrs.mie = MIP_MEIP;
        bus.push_uart_input(b"X");
        bus.write_u8(crate::bus::UART_START + 1, 1).unwrap();
        bus.write_u32(crate::bus::PLIC_START + 10 * 4, 1).unwrap();
        bus.write_u32(crate::bus::PLIC_START + 0x2000, 1 << 10)
            .unwrap();
        bus.write_u32(DRAM_START, 0x0010_8093).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(
            cpu.csr(CSR_MCAUSE),
            INTERRUPT_BIT | INTERRUPT_MACHINE_EXTERNAL
        );
        assert_eq!(cpu.csr(CSR_MTVAL), 0);
        assert_eq!(cpu.csr(CSR_MIP) & MIP_MEIP, MIP_MEIP);
        assert_eq!(cpu.register(1), 0);
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    #[test]
    fn vectored_machine_interrupt_uses_cause_offset() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(128);
        let vector = DRAM_START + 64;
        cpu.csrs.mtvec = vector | 1;
        cpu.csrs.mstatus = MSTATUS_MIE;
        cpu.csrs.mie = MIP_MSIP;
        bus.write_u32(crate::bus::CLINT_START, 1).unwrap();

        assert_eq!(cpu.step(&mut bus), Ok(None));

        assert_eq!(cpu.pc, vector + INTERRUPT_MACHINE_SOFTWARE * 4);
        assert_eq!(
            cpu.csr(CSR_MCAUSE),
            INTERRUPT_BIT | INTERRUPT_MACHINE_SOFTWARE
        );
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
    fn counter_enable_csrs_gate_lower_privilege_counter_access() {
        let instruction = encode_csr(u32::from(CSR_TIME), FUNCT_CSRRS, 0, 5);

        let mut supervisor = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        supervisor.privilege_mode = PrivilegeMode::Supervisor;
        supervisor.csrs.mtvec = DRAM_START + 4;
        bus.write_u32(DRAM_START, instruction).unwrap();
        assert_illegal_instruction_trap(&mut supervisor, &mut bus, instruction);

        let mut user = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        user.privilege_mode = PrivilegeMode::User;
        user.csrs.mcounteren = COUNTEREN_TIME;
        user.csrs.scounteren = COUNTEREN_TIME;
        bus.write_u32(DRAM_START, instruction).unwrap();
        assert_eq!(user.step(&mut bus), Ok(None));
        assert_eq!(user.register(5), 0);
    }

    #[test]
    fn counter_enable_csrs_apply_warl_masks() {
        let mut csrs = CsrFile::default();
        csrs.write(CSR_MCOUNTEREN, u64::MAX).unwrap();
        csrs.write(CSR_SCOUNTEREN, u64::MAX).unwrap();
        assert_eq!(csrs.read(CSR_MCOUNTEREN), Some(COUNTEREN_WRITABLE_MASK));
        assert_eq!(csrs.read(CSR_SCOUNTEREN), Some(COUNTEREN_WRITABLE_MASK));
    }

    #[test]
    fn writing_read_only_csr_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let instruction = encode_csr(u32::from(CSR_CYCLE), FUNCT_CSRRW, 1, 0);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_illegal_instruction_trap(&mut cpu, &mut bus, instruction);
        assert_eq!(cpu.csr(CSR_CYCLE), 0);
    }

    #[test]
    fn unknown_csr_access_is_illegal() {
        let mut cpu = Cpu::new(DRAM_START);
        let mut bus = Bus::new(8);
        let instruction = encode_csr(0x1ff, FUNCT_CSRRS, 0, 5);
        bus.write_u32(DRAM_START, instruction).unwrap();

        assert_illegal_instruction_trap(&mut cpu, &mut bus, instruction);
    }

    fn assert_illegal_instruction_trap(cpu: &mut Cpu, bus: &mut Bus, instruction: u32) {
        let vector = DRAM_START + 4;
        cpu.csrs.mtvec = vector;

        assert_eq!(cpu.step(bus), Ok(None));
        assert_eq!(cpu.pc, vector);
        assert_eq!(cpu.csr(CSR_MEPC), DRAM_START);
        assert_eq!(cpu.csr(CSR_MCAUSE), TRAP_ILLEGAL_INSTRUCTION);
        assert_eq!(cpu.csr(CSR_MTVAL), u64::from(instruction));
        assert_eq!(cpu.csr(CSR_INSTRET), 0);
    }

    fn install_sv39_table(bus: &mut Bus, root: u64, level1: u64, level0: u64) {
        install_pte(bus, root, 0, level1, PTE_V);
        install_pte(bus, level1, 0, level0, PTE_V);
    }

    fn install_pte(bus: &mut Bus, table: u64, index: u64, physical: u64, flags: u64) {
        let pte = ((physical >> PAGE_SHIFT) << PTE_PPN_SHIFT) | flags;
        bus.write_u64(table + index * PTE_SIZE, pte).unwrap();
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
