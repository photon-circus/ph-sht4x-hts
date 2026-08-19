#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// The modeled SHT45-AD1B 7-bit I2C address.
pub const ADDRESS: u8 = 0x44;
/// The modeled serial-number command byte.
pub const SERIAL_NUMBER_COMMAND: u8 = 0x89;
const RESPONSE_LEN: usize = 6;

/// Errors for transactions outside the model's fidelity boundary.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The transaction addressed something other than the modeled device.
    WrongAddress { expected: u8, actual: u8 },
    /// The command write did not contain exactly one byte.
    InvalidWriteLength { expected: usize, actual: usize },
    /// The one-byte write contained an unsupported command.
    UnsupportedCommand(u8),
    /// The response buffer was not exactly six bytes long.
    InvalidReadLength(usize),
    /// A read was attempted without a preceding serial command write.
    ReadBeforeCommand,
}

/// Independent behavioral model of the idle SHT45 serial-number operation.
pub struct Sht45Model {
    serial: u32,
    command_pending: bool,
}

impl Sht45Model {
    /// Create a model with an explicit OTP serial number.
    pub const fn new(serial: u32) -> Self {
        Self {
            serial,
            command_pending: false,
        }
    }

    /// Apply the modeled command write, including its STOP boundary.
    pub fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        if address != ADDRESS {
            return Err(Error::WrongAddress {
                expected: ADDRESS,
                actual: address,
            });
        }
        if bytes.len() != 1 {
            return Err(Error::InvalidWriteLength {
                expected: 1,
                actual: bytes.len(),
            });
        }
        if bytes[0] != SERIAL_NUMBER_COMMAND {
            return Err(Error::UnsupportedCommand(bytes[0]));
        }
        self.command_pending = true;
        Ok(())
    }

    /// Fill the modeled six-byte response, including its STOP boundary.
    pub fn read(&mut self, address: u8, response: &mut [u8]) -> Result<(), Error> {
        if address != ADDRESS {
            return Err(Error::WrongAddress {
                expected: ADDRESS,
                actual: address,
            });
        }
        if response.len() != RESPONSE_LEN {
            return Err(Error::InvalidReadLength(response.len()));
        }
        if !self.command_pending {
            return Err(Error::ReadBeforeCommand);
        }
        let words = [(self.serial >> 16) as u16, self.serial as u16];
        for (index, word) in words.into_iter().enumerate() {
            let bytes = word.to_be_bytes();
            let offset = index * 3;
            response[offset..offset + 2].copy_from_slice(&bytes);
            response[offset + 2] = crc8(bytes);
        }
        self.command_pending = false;
        Ok(())
    }
}

fn crc8(bytes: [u8; 2]) -> u8 {
    let mut crc = 0xff;
    for byte in bytes {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn models_the_required_serial_trace() {
        let mut model = Sht45Model::new(0xbeef_beef);
        let mut response = [0; 6];
        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    }

    #[test]
    fn serial_read_is_stable_after_another_command() {
        let mut model = Sht45Model::new(0x1234_5678);
        let mut first = [0; 6];
        let mut second = [0; 6];
        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut first).unwrap();
        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut second).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn owns_the_crc_vector() {
        let mut model = Sht45Model::new(0xbeef_beef);
        let mut response = [0; 6];
        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(response[2], 0x92);
    }

    #[test]
    fn rejects_non_modeled_transactions() {
        let mut model = Sht45Model::new(0xbeef_beef);
        let mut response = [0; 6];
        assert_eq!(
            model.write(0x45, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WrongAddress {
                expected: ADDRESS,
                actual: 0x45
            })
        );
        assert_eq!(
            model.write(ADDRESS, &[0x2c]),
            Err(Error::UnsupportedCommand(0x2c))
        );
        assert_eq!(
            model.read(ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );
        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        assert_eq!(
            model.read(ADDRESS, &mut [0; 5]),
            Err(Error::InvalidReadLength(5))
        );
    }

    #[test]
    fn reports_malformed_command_frames_distinctly() {
        let mut model = Sht45Model::new(0xbeef_beef);
        assert_eq!(
            model.write(ADDRESS, &[]),
            Err(Error::InvalidWriteLength {
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(
            model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND, 0x00]),
            Err(Error::InvalidWriteLength {
                expected: 1,
                actual: 2,
            })
        );
    }
}
