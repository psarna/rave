mod bus;
mod cpu;
mod debugger;
mod machine;

pub use bus::{Bus, BusError, Region};
pub use cpu::{
    decode_compressed_instruction, encoded_instruction_size, AddressAccess, AddressTranslation,
    Cpu, HaltReason, PrivilegeMode, StepError,
};
pub use debugger::{
    parse_number as debugger_parse_number, Command, CommandError, Debugger, StopReason,
    REGISTER_NAMES,
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
