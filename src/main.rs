#[macro_use]
extern crate log;

use std::path::PathBuf;
use structopt::StructOpt;
use std::fs::File;
use std::io;
use std::num::ParseIntError;
use serialport::{SerialPortSettings, DataBits, FlowControl, Parity, StopBits, ClearBuffer};
use std::io::Read;
use std::process::exit;
use termion::color;
use crate::ihex::IntelHex;
use std::time::Duration;

mod ihex;

fn parse_hex(src: &str) -> Result<u32, ParseIntError> {
    if src.starts_with("0x") {
        u32::from_str_radix(&src[2..], 16)
    } else {
        u32::from_str_radix(src, 10)
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "args")]
struct Arguments {
    #[structopt(name = "serial-port", long, short = "p")]
    serial_port: Option<String>,

    #[structopt(long, parse(try_from_str = parse_hex), default_value = "0")]
    base: u32,

    /// File to be flashed onto device
    #[structopt(name = "file", parse(from_os_str))]
    file: PathBuf,

    #[structopt(long, short = "dry")]
    dry: bool,
}

fn main() -> io::Result<()> {
    // Initialize Logger
    env_logger::init();
    // Arg parse
    let args = Arguments::from_args();
    debug!("Parsed {:?}", args);

    let mut target_file = File::open(args.file)?;
    debug!("file opened: {:?}", target_file);
    let meta = target_file.metadata()?;
    debug!("File size: {} bytes", meta.len());


    let mut commands: Vec<IntelHex> = Vec::new();

    let mut buffer: [u8; 255] = [0; 255];
    let mut current_base: u32 = args.base & 0xFF00;
    let mut file_offset: u32 = args.base;
    commands.push(
        ihex::IntelHex
        ::extended_address_command((current_base >> 16) as u16));
    loop {
        let size = target_file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        if file_offset & 0xFF00 != current_base {
            commands.push(
                ihex::IntelHex::extended_address_command(
                    (file_offset >> 16) as u16
                )
            );
            current_base = file_offset & 0xFF00;
        }
        commands.push(
            ihex::IntelHex::data_command(
                (file_offset & 0xFF) as u16,
                &buffer[0..size],
            ).unwrap()
        );
        file_offset += size as u32;
    }
    commands.push(ihex::IntelHex::eof());

    println!("Commands:");
    for cmd in &commands {
        println!("{}", cmd.to_string());
    }

    if !args.dry {
        let port_name: String = if args.serial_port.is_some() {
            args.serial_port.unwrap()
        } else {
            let ports = serialport::available_ports()?;
            if !ports.is_empty() {
                ports.get(0).unwrap().port_name.clone()
            } else {
                eprintln!("{}No serial port found{}", color::Fg(color::Red), color::Fg(color::Reset));
                exit(1);
            }
        };
        debug!("Selected Serial Port: {}", port_name);

        let serial_settings = SerialPortSettings {
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            flow_control: FlowControl::None,
            parity: Parity::None,
            stop_bits: StopBits::One,
            timeout: Duration::from_millis(1000),
        };
        let mut port =
            serialport::open_with_settings(&port_name, &serial_settings)?;
        debug!("Serial Port Opened");
        if port.clear(ClearBuffer::All).is_ok() {
            debug!("Serial Buffer Cleared")
        }
        let mut buffer= [0 as u8; 5];
        for cmd in commands {
            let len = port.write(cmd.to_string().as_bytes())?;
            assert_eq!(len, cmd.to_string().len());
            port.flush()?;
            let mut len;
            loop {
                len = port.read(&mut buffer)?;
                if len > 0{
                    break;
                }
            }
            println!("{}", String::from_utf8_lossy(&buffer[0..len]))
        }
    }

    Ok(())
}
