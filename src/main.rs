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
  rave boot [--interactive] --firmware <fw_jump.bin> --kernel <Image> --dtb <rave.dtb> [--memory <size>] [--limit <instructions>]

sizes accept K, M, and G suffixes (default boot memory: 128M)";

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
    let mut device_tree = None;
    let mut memory_size = Machine::BOOT_MEMORY_SIZE;
    let mut instruction_limit = u64::MAX;
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
            "--dtb" => device_tree = Some(option_path(arguments, &mut index, argument)?),
            "--memory" => {
                let value = option_value(arguments, &mut index, argument)?;
                memory_size = parse_size(value)?;
            }
            "--limit" => {
                let value = option_value(arguments, &mut index, argument)?;
                instruction_limit = value
                    .parse()
                    .map_err(|_| format!("invalid instruction limit: {value}"))?;
            }
            other => return Err(format!("unknown boot option: {other}\n{USAGE}").into()),
        }
        index += 1;
    }

    let firmware = fs::read(firmware.ok_or("boot requires --firmware")?)?;
    let kernel = fs::read(kernel.ok_or("boot requires --kernel")?)?;
    let device_tree = fs::read(device_tree.ok_or("boot requires --dtb")?)?;
    if interactive {
        return tui::run_boot(&firmware, &kernel, &device_tree, memory_size);
    }
    let machine = Machine::from_boot(&firmware, &kernel, &device_tree, memory_size)?;
    run_headless(machine, instruction_limit)
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
        Some(b'M' | b'm') => (&value[..value.len() - 1], 1024usize.pow(2)),
        Some(b'G' | b'g') => (&value[..value.len() - 1], 1024usize.pow(3)),
        _ => (value, 1),
    };
    number
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .filter(|size| *size > 0)
        .ok_or_else(|| format!("invalid memory size: {value}").into())
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
}
