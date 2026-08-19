use embedded_hal::i2c::{ErrorType, Operation, SevenBitAddress};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use futures_lite::future::block_on;
use ph_sht45_hts::{ADDRESS, Error as DriverError, Sht45};
use ph_sht45_hts_model::{Error as ModelError, SERIAL_NUMBER_COMMAND, Sht45Model};

#[derive(Debug, PartialEq, Eq)]
enum AdapterError {
    Model(ModelError),
    UnexpectedOperation,
}

impl embedded_hal::i2c::Error for AdapterError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        embedded_hal::i2c::ErrorKind::Other
    }
}

struct ModelI2c {
    model: Sht45Model,
    corrupt_crc: bool,
}

impl ModelI2c {
    fn new(serial: u32) -> Self {
        Self {
            model: Sht45Model::new(serial),
            corrupt_crc: false,
        }
    }

    fn corrupt_next_crc(&mut self) {
        self.corrupt_crc = true;
    }
}

impl ErrorType for ModelI2c {
    type Error = AdapterError;
}

impl I2c for ModelI2c {
    async fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        if operations.len() != 1 {
            return Err(AdapterError::UnexpectedOperation);
        }

        match &mut operations[0] {
            Operation::Write(bytes) => self
                .model
                .write(address, bytes)
                .map_err(AdapterError::Model),
            Operation::Read(response) => {
                self.model
                    .read(address, response)
                    .map_err(AdapterError::Model)?;
                if self.corrupt_crc {
                    response[2] ^= 1;
                    self.corrupt_crc = false;
                }
                Ok(())
            }
        }
    }
}

struct NoopDelay;

impl DelayNs for NoopDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
}

#[test]
fn public_serial_read_conforms_to_the_model_frame() {
    let mut sensor = Sht45::new(ModelI2c::new(0x1234_5678), NoopDelay);

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
}

#[test]
fn public_serial_read_rejects_an_adapter_corrupted_model_frame() {
    let mut i2c = ModelI2c::new(0xbeef_beef);
    i2c.corrupt_next_crc();
    let mut sensor = Sht45::new(i2c, NoopDelay);

    assert!(matches!(
        block_on(sensor.read_serial_number()),
        Err(DriverError::Crc { word: 0, .. })
    ));
}

#[test]
fn adapter_exposes_the_model_command_domain() {
    let mut i2c = ModelI2c::new(0xbeef_beef);
    let mut operations = [Operation::Write(&[SERIAL_NUMBER_COMMAND])];

    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();
    let mut response = [0; 6];
    let mut operations = [Operation::Read(&mut response)];
    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();

    assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
}
