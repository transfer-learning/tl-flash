#[macro_use]
extern crate log;
use std::path::PathBuf;
use structopt::StructOpt;
use std::fs::File;
use std::io;
use std::num::ParseIntError;
use serialport::{SerialPortSettings, DataBits, FlowControl, Parity, StopBits, ClearBuffer};

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
}

fn main() -> io::Result<()> {
    // Initialize Logger
    env_logger::init();
    // Arg parse
    let args = Arguments::from_args();
    debug!("Parsed {:?}", args);

    let target_file = File::open(args.file)?;
    debug!("file opened: {:?}", target_file);
    let meta = target_file.metadata()?;
    debug!("File size: {} bytes", meta.len());

    let port_name: String = if args.serial_port.is_some() {
        args.serial_port.unwrap()
    } else {
        let ports = serialport::available_ports()?;
        if !ports.is_empty() {
            ports.get(0).unwrap().port_name.clone()
        } else {
            panic!("No serial port found")
        }
    };
    debug!("Selected Serial Port: {}", port_name);

    let serial_settings = SerialPortSettings {
        baud_rate: 115200,
        data_bits: DataBits::Eight,
        flow_control: FlowControl::None,
        parity: Parity::None,
        stop_bits: StopBits::One,
        timeout: Default::default(),
    };
    let port = serialport::open_with_settings(&port_name, &serial_settings)?;
    debug!("Serial Port Opened");
    if port.clear(ClearBuffer::All).is_ok() {
        debug!("Serial Buffer Cleared")
    }
    let hex = ihex::IntelHex::extended_address_command(0xb0ba);

    Ok(())
}
