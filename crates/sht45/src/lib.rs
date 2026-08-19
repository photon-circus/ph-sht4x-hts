#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use embedded_hal_async::i2c::I2c;

/// The fixed 7-bit address of the supported SHT45-AD1B device.
pub const ADDRESS: u8 = 0x44;

const SERIAL_NUMBER_COMMAND: u8 = 0x89;
const SERIAL_NUMBER_RESPONSE_LEN: usize = 6;

/// Errors returned by the SHT45 driver.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    /// The device or bus rejected the transfer for a reason other than NACK.
    I2c(E),
    /// The device was not ready or otherwise did not acknowledge the transfer.
    NoAcknowledge(E),
    /// One of the two serial-number words failed its CRC check.
    Crc {
        word: usize,
        expected: u8,
        actual: u8,
    },
}

/// An SHT45 connected to one abstract asynchronous I2C bus.
pub struct Sht45<I2C> {
    i2c: I2C,
}

impl<I2C> Sht45<I2C> {
    /// Create a driver for the SHT45-AD1B at address `0x44`.
    pub const fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Read the device's 32-bit transmission-order serial number.
    pub async fn read_serial_number<E>(&mut self) -> Result<u32, Error<E>>
    where
        I2C: I2c<Error = E>,
        E: embedded_hal::i2c::Error,
    {
        self.i2c
            .write(ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .await
            .map_err(map_i2c_error)?;

        let mut response = [0; SERIAL_NUMBER_RESPONSE_LEN];
        self.i2c
            .read(ADDRESS, &mut response)
            .await
            .map_err(map_i2c_error)?;

        let word0 = u16::from_be_bytes([response[0], response[1]]);
        let word1 = u16::from_be_bytes([response[3], response[4]]);
        for (word, (value, actual)) in [(word0, response[2]), (word1, response[5])]
            .into_iter()
            .enumerate()
        {
            let expected = crc8(value.to_be_bytes());
            if expected != actual {
                return Err(Error::Crc {
                    word,
                    expected,
                    actual,
                });
            }
        }

        Ok((u32::from(word0) << 16) | u32::from(word1))
    }

    /// Return the underlying I2C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

fn map_i2c_error<E>(error: E) -> Error<E>
where
    E: embedded_hal::i2c::Error,
{
    match error.kind() {
        embedded_hal::i2c::ErrorKind::NoAcknowledge(_) => Error::NoAcknowledge(error),
        _ => Error::I2c(error),
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
    use embedded_hal::i2c::{ErrorType, Operation, SevenBitAddress};
    use futures_lite::future::block_on;
    use std::{vec, vec::Vec};

    #[derive(Debug, PartialEq, Eq)]
    enum FakeError {
        NoAcknowledge,
        Bus,
    }

    impl embedded_hal::i2c::Error for FakeError {
        fn kind(&self) -> embedded_hal::i2c::ErrorKind {
            match self {
                Self::NoAcknowledge => embedded_hal::i2c::ErrorKind::NoAcknowledge(
                    embedded_hal::i2c::NoAcknowledgeSource::Address,
                ),
                Self::Bus => embedded_hal::i2c::ErrorKind::Bus,
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Transaction {
        Write(SevenBitAddress, Vec<u8>),
        Read(SevenBitAddress, usize),
    }

    struct FakeI2c {
        transactions: Vec<Transaction>,
        response: [u8; 6],
        read_error: Option<FakeError>,
    }

    impl ErrorType for FakeI2c {
        type Error = FakeError;
    }

    impl embedded_hal_async::i2c::I2c for FakeI2c {
        async fn transaction(
            &mut self,
            address: SevenBitAddress,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            for operation in operations {
                match operation {
                    Operation::Write(bytes) => {
                        self.transactions
                            .push(Transaction::Write(address, bytes.to_vec()));
                    }
                    Operation::Read(bytes) => {
                        self.transactions
                            .push(Transaction::Read(address, bytes.len()));
                        if let Some(error) = self.read_error.take() {
                            return Err(error);
                        }
                        bytes.copy_from_slice(&self.response[..bytes.len()]);
                    }
                }
            }
            Ok(())
        }
    }

    fn fake(response: [u8; 6]) -> FakeI2c {
        FakeI2c {
            transactions: Vec::new(),
            response,
            read_error: None,
        }
    }

    #[test]
    fn crc_vector_matches_datasheet_example() {
        assert_eq!(crc8([0xbe, 0xef]), 0x92);
    }

    #[test]
    fn reads_serial_with_two_transactions_and_validates_crc() {
        let fake = fake([0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
        let mut sensor = Sht45::new(fake);
        assert_eq!(block_on(sensor.read_serial_number()), Ok(0xbeef_beef));
        assert_eq!(
            sensor.release().transactions,
            vec![
                Transaction::Write(ADDRESS, vec![SERIAL_NUMBER_COMMAND]),
                Transaction::Read(ADDRESS, 6),
            ]
        );
    }

    #[test]
    fn rejects_each_corrupt_crc() {
        for index in [2, 5] {
            let mut response = [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92];
            response[index] ^= 1;
            let mut sensor = Sht45::new(fake(response));
            assert!(matches!(
                block_on(sensor.read_serial_number()),
                Err(Error::Crc { .. })
            ));
        }
    }

    #[test]
    fn surfaces_nack_as_not_acknowledge_error() {
        let mut fake = fake([0; 6]);
        fake.read_error = Some(FakeError::NoAcknowledge);
        let mut sensor = Sht45::new(fake);
        assert_eq!(
            block_on(sensor.read_serial_number()),
            Err(Error::NoAcknowledge(FakeError::NoAcknowledge))
        );
    }

    #[test]
    fn preserves_non_nack_bus_error() {
        let mut fake = fake([0; 6]);
        fake.read_error = Some(FakeError::Bus);
        let mut sensor = Sht45::new(fake);
        assert_eq!(
            block_on(sensor.read_serial_number()),
            Err(Error::I2c(FakeError::Bus))
        );
    }
}
