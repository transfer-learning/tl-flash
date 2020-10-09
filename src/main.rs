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
use indicatif::{HumanBytes, ProgressBar, ProgressStyle, ProgressDrawTarget};
use std::cmp::min;

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

    #[structopt(short, parse(from_occurrences))]
    verbose: usize,
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
    debug!("File size: {}", HumanBytes(meta.len()));

    let file_size_status = ProgressBar::new(0);
    file_size_status.set_draw_target(ProgressDrawTarget::stdout());
    let convert_status = ProgressBar::new(meta.len());
    convert_status.set_draw_target(ProgressDrawTarget::stdout());
    file_size_status.set_style(ProgressStyle::default_bar().template("{msg}"));
    file_size_status.set_message(format!("Input File Size {}", HumanBytes(meta.len())).as_str());
    convert_status.set_style(ProgressStyle::default_bar().template("{prefix} [{bar}] {msg} {bytes}/{total_bytes}"));
    convert_status.set_message("Covert Binary to IntelHex");

    let mut commands: Vec<IntelHex> = Vec::new();
//    convert_status.set_length(meta.len());
//    convert_status.set_style(ProgressStyle::default_bar());
//    convert_status.set_message("Converting Binary to IntelHex");
    let mut buffer: [u8; 255] = [0; 255];
    let mut current_base: u32 = args.base & 0xFFFF_0000;
    let mut file_offset: u32 = args.base;
    commands.push(
        ihex::IntelHex
        ::extended_address_command((current_base >> 16) as u16));
    loop {
        let size = target_file.read(&mut buffer)?;
        let mut sent = 0;
        convert_status.inc(size as u64);
        if size == 0 {
            break;
        }
        let sendable_without_change_of_addr = ((current_base | 0x0000_FFFF) - file_offset + 1) as usize;
        let first_batch = min(sendable_without_change_of_addr, size);
        if first_batch > 0 {
            commands.push(
                ihex::IntelHex::data_command(
                    (file_offset & 0xFFFF) as u16,
                    &buffer[0..first_batch],
                ).unwrap()
            );
            file_offset += first_batch as u32;
            sent = first_batch;
        }
        if size - sent == 0 {
            continue;
        }

        if file_offset & 0xFFFF0000 != current_base {
            commands.push(
                ihex::IntelHex::extended_address_command(
                    (file_offset >> 16) as u16
                )
            );
            current_base = file_offset & 0xFFFF0000;
        }
        commands.push(
            ihex::IntelHex::data_command(
                (file_offset & 0xFFFF) as u16,
                &buffer[sent..size],
            ).unwrap()
        );
        file_offset += size as u32;
    }
    commands.push(ihex::IntelHex::eof());
    convert_status.finish_with_message(&format!("{} Intel Hex Commands", commands.len()));
    convert_status.tick();

    if args.verbose >= 2 {
        println!("Commands:");
        for cmd in &commands {
            println!("{}", cmd.to_string());
        }
    }

    if !args.dry {
        let download_bar = ProgressBar::new(commands.len() as u64);
        download_bar.set_style(ProgressStyle::default_bar().template("{prefix} [{bar}] {msg} {pos}/{len}[{percent}%] eta[{eta_precise}]"));
        download_bar.set_message("Sending Intel Hex Commands");
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
        println!("Timeout 3s");

        let serial_settings = SerialPortSettings {
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            flow_control: FlowControl::None,
            parity: Parity::None,
            stop_bits: StopBits::One,
            timeout: Duration::from_millis(3000),
        };
        let mut port =
            serialport::open_with_settings(&port_name, &serial_settings)?;
        println!("Serial Port Opened");
        if port.clear(ClearBuffer::All).is_ok() {
            println!("Serial Buffer Cleared")
        }
        let mut buffer = [0 as u8; 5];
        for (i, cmd) in commands.iter().enumerate() {
            download_bar.set_position(i as u64);
            loop {
                let len = port.write(cmd.to_string().as_bytes())?;
                assert_eq!(len, cmd.to_string().len(), "command potentially contains non-ascii");
                port.flush()?;
                let mut len: usize;
                loop {
                    len = port.read(&mut buffer).expect("EHHHH");
                    if len > 0 {
                        break;
                    }
                }
                if buffer[0] as char == cmd.command_type().ack_char() {
                    break;
                }
                println!("{} NACK Recived: {}, retrying...{}",
                         color::Fg(color::Red),
                         buffer[0] as char,
                         color::Fg(color::Reset)
                );
                port.clear(ClearBuffer::All)?;
            }
        }
        download_bar.finish_with_message("Downloaded");
    }
    Ok(())
}
