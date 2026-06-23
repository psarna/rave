use rave::{HaltReason, Machine};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn compiles_and_runs_a_freestanding_rv64i_guest() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::temp_dir().join(format!("rave-guest-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let object = out.join("guest.o");
    let elf = out.join("guest.elf");
    let binary = out.join("guest.bin");

    run(
        Command::new("clang")
            .args([
                "--target=riscv64-unknown-elf",
                "-march=rv64i",
                "-mabi=lp64",
                "-mcmodel=medany",
                "-ffreestanding",
                "-fno-builtin",
                "-nostdlib",
                "-O1",
                "-c",
            ])
            .arg(root.join("tests/fixtures/guest.c"))
            .arg("-o")
            .arg(&object),
        "compile RV64I guest",
    );

    let linker = rust_lld();
    run(
        Command::new(linker)
            .args(["-flavor", "gnu", "-m", "elf64lriscv", "-T"])
            .arg(root.join("tests/fixtures/link.ld"))
            .arg(&object)
            .arg("-o")
            .arg(&elf),
        "link RV64I guest",
    );

    run(
        Command::new("llvm-objcopy")
            .args(["-O", "binary"])
            .arg(&elf)
            .arg(&binary),
        "convert guest to raw binary",
    );

    let image = std::fs::read(binary).unwrap();
    let mut machine =
        Machine::from_raw(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE).unwrap();
    assert_eq!(
        machine.run(100_000).unwrap(),
        HaltReason::Breakpoint { code: 0 }
    );
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
