mod bus;
mod cpu;
mod debugger;
mod machine;

pub use bus::{Bus, BusError, Region};
pub use cpu::{
    decode_compressed_instruction, encoded_instruction_size, AddressAccess, AddressTranslation,
    Cpu, HaltReason, PrivilegeMode, StepError, CSR_MCOUNTEREN, CSR_MSTATUS, CSR_SATP,
    CSR_SCOUNTEREN, INSTRUCTION_SFENCE_VMA, INSTRUCTION_SFENCE_VMA_MASK,
};
pub use debugger::{
    parse_number as debugger_parse_number, Command, CommandError, Debugger, StopReason,
    MCOUNTEREN_REGISTER_INDEX, MSIP_REGISTER_INDEX, MSTATUS_REGISTER_INDEX,
    MTIMECMP_REGISTER_INDEX, MTIME_REGISTER_INDEX, PC_REGISTER_INDEX,
    PLIC_MACHINE_CLAIM_REGISTER_INDEX, PLIC_MACHINE_ENABLE_REGISTER_INDEX,
    PLIC_MACHINE_THRESHOLD_REGISTER_INDEX, PLIC_PENDING_REGISTER_INDEX,
    PLIC_SUPERVISOR_CLAIM_REGISTER_INDEX, PLIC_SUPERVISOR_ENABLE_REGISTER_INDEX,
    PLIC_SUPERVISOR_THRESHOLD_REGISTER_INDEX, PLIC_UART_PRIORITY_REGISTER_INDEX, REGISTER_NAMES,
    SATP_REGISTER_INDEX, SCOUNTEREN_REGISTER_INDEX, UART_IER_REGISTER_INDEX,
};
pub use machine::{Machine, MachineError};

/// Shared instruction decoding/arithmetic helpers used by the TUI previews.
#[doc(hidden)]
pub mod decode_helpers {
    pub use crate::cpu::{
        b_immediate, div, divu, divuw, divw, i_immediate, j_immediate, mulh, mulhsu, mulhu, rem,
        remu, remuw, remw, s_immediate, sign_extend_word, upper_immediate,
    };
}
