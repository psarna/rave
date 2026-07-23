mod tui;

use rave::Machine;
use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    process,
    sync::mpsc::{self, Receiver},
    thread,
};

const USAGE: &str = "usage:
  rave [--interactive] <guest.bin>
  rave boot [--interactive] --firmware <fw_jump.bin> --kernel <Image> [--initrd <rootfs.cpio>] --dtb <rave.dtb> [--memory <size>] [--limit <instructions>]

sizes accept K, M, and G suffixes (default boot memory: 128M)
boot runs until the guest halts by default; --limit is headless-only";

fn main() {
    if let Err(error) = run() {
        eprintln!("rave: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().is_some_and(|argument| argument == "boot") {
        return run_boot(&arguments[1..]);
    }
    run_raw(&arguments)
}

fn run_raw(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut interactive = false;
    let mut path = None;
    for argument in arguments {
        match argument.as_str() {
            "--interactive" | "-i" => interactive = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}\n{USAGE}").into())
            }
            _ if path.is_none() => path = Some(argument),
            _ => return Err(USAGE.into()),
        }
    }
    let path = path.ok_or(USAGE)?;
    let image = fs::read(path)?;
    if interactive {
        return tui::run(&image);
    }
    let machine = Machine::from_raw(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE)?;
    run_headless(machine, Machine::INSTRUCTION_LIMIT)
}

fn run_boot(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut firmware = None;
    let mut kernel = None;
    let mut initrd = None;
    let mut device_tree = None;
    let mut memory_size = Machine::BOOT_MEMORY_SIZE;
    let mut instruction_limit = None;
    let mut interactive = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        match argument.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--interactive" | "-i" => interactive = true,
            "--firmware" => firmware = Some(option_path(arguments, &mut index, argument)?),
            "--kernel" => kernel = Some(option_path(arguments, &mut index, argument)?),
            "--initrd" => initrd = Some(option_path(arguments, &mut index, argument)?),
            "--dtb" => device_tree = Some(option_path(arguments, &mut index, argument)?),
            "--memory" => {
                let value = option_value(arguments, &mut index, argument)?;
                memory_size = parse_size(value)?;
            }
            "--limit" => {
                let value = option_value(arguments, &mut index, argument)?;
                instruction_limit = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid instruction limit: {value}"))?,
                );
            }
            other => return Err(format!("unknown boot option: {other}\n{USAGE}").into()),
        }
        index += 1;
    }

    let firmware = fs::read(firmware.ok_or("boot requires --firmware")?)?;
    let kernel = fs::read(kernel.ok_or("boot requires --kernel")?)?;
    let initrd = initrd.map(fs::read).transpose()?;
    let device_tree = fs::read(device_tree.ok_or("boot requires --dtb")?)?;
    validate_dtb_memory(&device_tree, memory_size)?;
    if interactive {
        if instruction_limit.is_some() {
            return Err("--limit is only supported for headless boot".into());
        }
        return tui::run_boot_with_initrd(
            &firmware,
            &kernel,
            initrd.as_deref(),
            &device_tree,
            memory_size,
        );
    }
    let machine = Machine::from_boot_with_initrd(
        &firmware,
        &kernel,
        initrd.as_deref(),
        &device_tree,
        memory_size,
    )?;
    run_headless(machine, instruction_limit.unwrap_or(u64::MAX))
}

fn option_path(
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(option_value(arguments, index, option)?))
}

fn option_value<'a>(
    arguments: &'a [String],
    index: &mut usize,
    option: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    *index += 1;
    arguments
        .get(*index)
        .map(String::as_str)
        .ok_or_else(|| format!("missing value for {option}").into())
}

fn parse_size(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let (number, multiplier) = match value.as_bytes().last().copied() {
        Some(b'K' | b'k') => (&value[..value.len() - 1], 1024usize),
        Some(b'M' | b'm') => (&value[..value.len() - 1], 1024usize * 1024),
        Some(b'G' | b'g') => (&value[..value.len() - 1], 1024usize * 1024 * 1024),
        _ => (value, 1),
    };
    number
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .filter(|size| *size > 0)
        .ok_or_else(|| format!("invalid memory size: {value}").into())
}

fn validate_dtb_memory(
    device_tree: &[u8],
    memory_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let tree = fdt::Fdt::new(device_tree).map_err(|error| format!("invalid DTB: {error}"))?;
    let memory = tree
        .find_node("/memory")
        .ok_or("DTB does not contain a memory node")?;
    let mut regions = memory.reg().ok_or("DTB memory node has no reg property")?;
    let region = regions.next().ok_or("DTB memory node has no regions")?;
    if regions.next().is_some() {
        return Err(
            "DTB describes multiple memory regions; rave requires one contiguous region".into(),
        );
    }

    let dtb_start = region.starting_address as usize as u64;
    let dtb_size = region
        .size
        .ok_or("DTB memory region does not specify a size")?;
    if dtb_start != Machine::LOAD_ADDRESS || dtb_size != memory_size {
        return Err(format!(
            "DTB memory region is {dtb_start:#x} + {dtb_size:#x} bytes, but rave is configured for {:#x} + {memory_size:#x} bytes; update the DTB or --memory",
            Machine::LOAD_ADDRESS
        )
        .into());
    }
    Ok(())
}

fn run_headless(
    mut machine: Machine,
    instruction_limit: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let terminal_input = if io::stdin().is_terminal() {
        Some(spawn_terminal_input())
    } else {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        machine.bus.push_uart_input(&input);
        None
    };

    let mut written = 0;
    let mut stdout = io::stdout().lock();
    for _ in 0..instruction_limit {
        if let Some(input) = &terminal_input {
            if let Ok(first) = input.try_recv() {
                let mut bytes = vec![first];
                bytes.extend(input.try_iter());
                machine.bus.push_uart_input(&bytes);
            }
        }
        let reason = machine.step()?;
        let output = machine.bus.uart_output();
        if written < output.len() {
            stdout.write_all(&output[written..])?;
            stdout.flush()?;
            written = output.len();
        }
        if let Some(reason) = reason {
            writeln!(stdout, "{reason:?}")?;
            return Ok(());
        }
    }
    Err(format!("guest exceeded {instruction_limit} instructions").into())
}

fn spawn_terminal_input() -> Receiver<u8> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        for byte in stdin.lock().bytes() {
            match byte {
                Ok(byte) if sender.send(byte).is_ok() => {}
                _ => break,
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sizes_accept_binary_suffixes() {
        assert_eq!(parse_size("128M").unwrap(), 128 * 1024 * 1024);
        assert_eq!(parse_size("4k").unwrap(), 4096);
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert!(parse_size("0").is_err());
        assert!(parse_size("many").is_err());
    }

    #[test]
    fn bundled_dtb_matches_default_boot_memory() {
        validate_dtb_memory(
            include_bytes!("../demo/rave.dtb"),
            Machine::BOOT_MEMORY_SIZE,
        )
        .unwrap();
    }

    #[test]
    fn dtb_memory_mismatch_is_rejected() {
        let error =
            validate_dtb_memory(include_bytes!("../demo/rave.dtb"), 64 * 1024 * 1024).unwrap_err();
        assert!(error.to_string().contains("update the DTB or --memory"));
    }

    #[test]
    fn invalid_dtb_is_rejected() {
        let error = validate_dtb_memory(b"not a DTB", Machine::BOOT_MEMORY_SIZE).unwrap_err();
        assert!(error.to_string().starts_with("invalid DTB:"));
    }
}
