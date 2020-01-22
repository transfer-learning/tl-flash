use std::fmt::Error;
use simple_error::SimpleError;

#[derive(Copy, Clone)]
pub enum CommandType {
    Data,
    EOF,
    ExtendedAddress,
}

impl Into<u8> for CommandType {
    fn into(self) -> u8 {
        match self {
            CommandType::ExtendedAddress => 0x04,
            CommandType::Data => 0x00,
            CommandType::EOF => 0x01,
        }
    }
}

pub struct IntelHex {
    data: Vec<u8>,
    command_type: CommandType,
    address: u16,
}

impl Into<String> for IntelHex {
    fn into(self) -> String {
        self.to_string()
    }
}

impl ToString for IntelHex {
    fn to_string(&self) -> String {
        // :[LEN][ADDR][TYPE][DATA][CHECKSUM]
        let mut str = String::from(format!(":{:02X}{:04X}{:02X}",
                                           self.data.len(),
                                           self.address,
                                           Into::<u8>::into(self.command_type)
        ));

        for a in &self.data {
            str.push_str(&format!("{:02X}", a));
        }

        str.push_str(&format!("{:02X}", self.compute_checksum()));

        return str;
    }
}

impl IntelHex {
    fn new(command_type: CommandType) -> IntelHex {
        IntelHex {
            data: Vec::new(),
            command_type,
            address: 0,
        }
    }

    pub fn extended_address_command(addr: u16) -> IntelHex {
        let mut hex = IntelHex::new(CommandType::ExtendedAddress);
        // Big Endian Push
        hex.push_byte(((addr >> 8) & 0xFF) as u8);
        hex.push_byte((addr & 0xFF) as u8);
        hex
    }

    pub fn eof() -> IntelHex {
        IntelHex::new(CommandType::EOF)
    }

    pub fn data_command(base_addr: u16, data: &[u8]) -> Result<IntelHex, SimpleError> {
        if data.len() > 0xFF {
            return Err(SimpleError::new("data exceed max length (255 bytes)"))
        }
        let mut hex = IntelHex::new(CommandType::Data);
        hex.data.append(data.into_vec().as_mut());
        hex.address = base_addr;
        return Ok(hex);
    }

    pub fn push_byte(&mut self, byte: u8) {
        self.data.push(byte);
    }

    fn compute_checksum(&self) -> u8 {
        assert!(self.data.len() <= 0xFF, "[IntelHex] Data is too long (Max 255 Bytes)");
        (!((&self.data).into_iter()
            // Length and Data
            .fold(self.data.len() as u8, |sum, item| { sum.wrapping_add(*item) })
            .wrapping_add(Into::<u8>::into(self.command_type))
            .wrapping_add((self.address >> 8) as u8)
            .wrapping_add((self.address & 0xFF) as u8) // Address Low Byte
        )).wrapping_add(1)
    }
}

