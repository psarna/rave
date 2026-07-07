mod tui;

use rave::Machine;
use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    process,
};

const USAGE: &str = "usage: rave [--interactive] <guest.bin>";

fn main() {
    if let Err(error) = run() {
        eprintln!("rave: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut interactive = false;
    let mut path = None;
    for argument in env::args().skip(1) {
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
    let mut machine = Machine::from_raw(&image, Machine::LOAD_ADDRESS, Machine::MEMORY_SIZE)?;
    if !io::stdin().is_terminal() {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        machine.bus.push_uart_input(&input);
    }
    let reason = machine.run(Machine::INSTRUCTION_LIMIT)?;
    io::stdout().write_all(machine.bus.uart_output())?;
    println!("{reason:?}");
    Ok(())
}
