use rave::{HaltReason, Machine};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn compiles_and_runs_a_freestanding_rv64i_guest() {
    let result = compile_and_run_guest("guest", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"O");
}

#[test]
fn compiles_and_runs_a_uart_guest() {
    let result = compile_and_run_guest("uart", b"Ada\n", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"name?\noh hai Ada!\n");
}

#[test]
fn compiles_and_runs_an_rv64m_guest() {
    let result = compile_and_run_guest_with_march("rv64m", "rv64im", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"M");
}

#[test]
fn compiles_and_runs_an_rv64a_guest() {
    let result = compile_and_run_guest_with_march("rv64a", "rv64ima", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"A");
}

#[test]
fn compiles_and_runs_a_zicsr_guest() {
    let result = compile_and_run_guest_with_march("zicsr", "rv64im_zicsr", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"C");
}

#[test]
fn compiles_and_runs_an_rv64c_guest() {
    let result = compile_and_run_guest_with_march("rv64c", "rv64imac", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"C");
}

#[test]
fn compiles_and_runs_a_machine_trap_guest() {
    let result = compile_and_run_guest_with_march("mtrap", "rv64im_zicsr", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"TU");
}

#[test]
fn compiles_and_runs_a_machine_timer_interrupt_guest() {
    let result = compile_and_run_guest_with_march("clint", "rv64im_zicsr", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"I");
}

#[test]
fn compiles_and_runs_a_machine_software_interrupt_guest() {
    let result = compile_and_run_guest_with_march("msip", "rv64im_zicsr", b"", 100_000);
    assert_eq!(result.reason, HaltReason::Breakpoint { code: 0 });
    assert_eq!(result.uart_output, b"S");
}

struct GuestResult {
    reason: HaltReason,
    uart_output: Vec<u8>,
}

fn compile_and_run_guest(name: &str, uart_input: &[u8], instruction_limit: u64) -> GuestResult {
    compile_and_run_guest_with_march(name, "rv64i", uart_input, instruction_limit)
}

fn compile_and_run_guest_with_march(
    name: &str,
    march: &str,
    uart_input: &[u8],
    instruction_limit: u64,
) -> GuestResult {
    let binary = compile_guest(name, march);
    let image = std::fs::read(binary).unwrap();
    let mut machine =
        Machine::from_raw(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE).unwrap();
    machine.bus.push_uart_input(uart_input);
    let reason = machine.run(instruction_limit).unwrap();
    GuestResult {
        reason,
        uart_output: machine.bus.uart_output().to_vec(),
    }
}

fn compile_guest(name: &str, march: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::temp_dir().join(format!("rave-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let object = out.join(format!("{name}.o"));
    let elf = out.join(format!("{name}.elf"));
    let binary = out.join(format!("{name}.bin"));

    run(
        Command::new("clang")
            .args([
                "--target=riscv64-unknown-elf",
                &format!("-march={march}"),
                "-mabi=lp64",
                "-mcmodel=medany",
                "-ffreestanding",
                "-fno-builtin",
                "-nostdlib",
                "-O1",
                "-c",
            ])
            .arg(root.join(format!("tests/fixtures/{name}.c")))
            .arg("-o")
            .arg(&object),
        "compile RISC-V guest",
    );

    let linker = rust_lld();
    run(
        Command::new(linker)
            .args(["-flavor", "gnu", "-m", "elf64lriscv", "-T"])
            .arg(root.join("tests/fixtures/link.ld"))
            .arg(&object)
            .arg("-o")
            .arg(&elf),
        "link RISC-V guest",
    );

    run(
        Command::new("llvm-objcopy")
            .args(["-O", "binary"])
            .arg(&elf)
            .arg(&binary),
        "convert guest to raw binary",
    );

    binary
}

fn run(command: &mut Command, description: &str) {
    let output = command.output().unwrap_or_else(|error| {
        panic!("failed to start {description}: {error}");
    });
    assert!(
        output.status.success(),
        "{description} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rust_lld() -> PathBuf {
    let output = Command::new("rustc")
        .args(["--print", "target-libdir"])
        .output()
        .expect("rustc must be installed");
    assert!(
        output.status.success(),
        "rustc --print target-libdir failed"
    );
    let target_libdir = Path::new(std::str::from_utf8(&output.stdout).unwrap().trim());
    target_libdir
        .parent()
        .expect("target libdir must have a rustlib parent")
        .join("bin/rust-lld")
}
