mod tui;

use rave::Machine;
use std::{env, fs, process};

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
    println!("{:?}", machine.run(Machine::INSTRUCTION_LIMIT)?);
    Ok(())
}
