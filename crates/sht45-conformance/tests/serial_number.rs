use embedded_hal::i2c::{ErrorType, Operation, SevenBitAddress};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use futures_lite::future::block_on;
use ph_sht45_hts::{ADDRESS, Error as DriverError, Sht45};
use ph_sht45_hts_model::{
    Error as ModelError, MEASURE_HIGH_COMMAND, SERIAL_NUMBER_COMMAND, Sht45Model,
};
use std::{cell::RefCell, rc::Rc, vec::Vec};

#[derive(Debug, PartialEq, Eq)]
enum AdapterError {
    Model(ModelError),
    UnexpectedOperation,
}

impl embedded_hal::i2c::Error for AdapterError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        if matches!(self, Self::Model(ModelError::Busy)) {
            return embedded_hal::i2c::ErrorKind::NoAcknowledge(
                embedded_hal::i2c::NoAcknowledgeSource::Data,
            );
        }
        embedded_hal::i2c::ErrorKind::Other
    }
}

type SharedModel = Rc<RefCell<Sht45Model>>;
type SharedEvents = Rc<RefCell<Vec<AdapterEvent>>>;

#[derive(Debug, PartialEq, Eq)]
enum AdapterEvent {
    Write {
        address: SevenBitAddress,
        bytes: Vec<u8>,
    },
    Read {
        address: SevenBitAddress,
        length: usize,
    },
    DelayNs(u32),
}

struct ModelI2c {
    model: SharedModel,
    events: SharedEvents,
    corrupt_crc: bool,
}

struct ModelDelay {
    model: SharedModel,
    events: SharedEvents,
}

impl ModelI2c {
    fn new(serial: u32) -> (Self, ModelDelay, SharedEvents) {
        Self::with_model(Sht45Model::new(serial))
    }

    fn with_measurement_ticks(
        serial: u32,
        temperature: u16,
        humidity: u16,
    ) -> (Self, ModelDelay, SharedEvents) {
        Self::with_model(Sht45Model::new(serial).with_measurement_ticks(temperature, humidity))
    }

    fn with_model(model: Sht45Model) -> (Self, ModelDelay, SharedEvents) {
        let model = Rc::new(RefCell::new(model));
        let events = Rc::new(RefCell::new(Vec::new()));
        let i2c = Self {
            model: Rc::clone(&model),
            events: Rc::clone(&events),
            corrupt_crc: false,
        };
        let delay = ModelDelay {
            model,
            events: Rc::clone(&events),
        };
        (i2c, delay, events)
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
            Operation::Write(bytes) => {
                self.events.borrow_mut().push(AdapterEvent::Write {
                    address,
                    bytes: bytes.to_vec(),
                });
                self.model
                    .borrow_mut()
                    .write(address, bytes)
                    .map_err(AdapterError::Model)
            }
            Operation::Read(response) => {
                self.events.borrow_mut().push(AdapterEvent::Read {
                    address,
                    length: response.len(),
                });
                self.model
                    .borrow_mut()
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

impl DelayNs for ModelDelay {
    async fn delay_ns(&mut self, ns: u32) {
        self.events.borrow_mut().push(AdapterEvent::DelayNs(ns));
        self.model.borrow_mut().advance_ns(u64::from(ns));
    }
}

struct NoopDelay;

impl DelayNs for NoopDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
}

#[test]
fn public_serial_read_conforms_to_the_model_frame() {
    let (i2c, _delay, _events) = ModelI2c::new(0x1234_5678);
    let mut sensor = Sht45::new(i2c, NoopDelay);

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
}

#[test]
fn public_serial_read_rejects_an_adapter_corrupted_model_frame() {
    let (mut i2c, _delay, _events) = ModelI2c::new(0xbeef_beef);
    i2c.corrupt_next_crc();
    let mut sensor = Sht45::new(i2c, NoopDelay);

    assert!(matches!(
        block_on(sensor.read_serial_number()),
        Err(DriverError::Crc { word: 0, .. })
    ));
}

#[test]
fn adapter_exposes_the_model_command_domain() {
    let (mut i2c, _delay, _events) = ModelI2c::new(0xbeef_beef);
    let mut operations = [Operation::Write(&[SERIAL_NUMBER_COMMAND])];

    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();
    let mut response = [0; 6];
    let mut operations = [Operation::Read(&mut response)];
    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();

    assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
}

#[test]
fn public_measure_conforms_at_each_repeatability_frontier() {
    for (repeatability, expected_command, expected_delay_ns) in [
        (ph_sht45_hts::Repeatability::High, 0xfd, 8_300_000),
        (ph_sht45_hts::Repeatability::Medium, 0xf6, 4_500_000),
        (ph_sht45_hts::Repeatability::Low, 0xe0, 1_600_000),
    ] {
        let (i2c, delay, events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
        let mut sensor = Sht45::new(i2c, delay);

        assert_eq!(
            block_on(sensor.measure(repeatability)),
            Ok(ph_sht45_hts::Measurement {
                t_mdeg_c: 85_523,
                rh_milli_pct: 87_230,
            })
        );
        assert_eq!(
            *events.borrow(),
            [
                AdapterEvent::Write {
                    address: ADDRESS,
                    bytes: vec![expected_command],
                },
                AdapterEvent::DelayNs(expected_delay_ns),
                AdapterEvent::Read {
                    address: ADDRESS,
                    length: 6,
                },
            ]
        );
    }
}

#[test]
fn public_measure_rejects_an_adapter_corrupted_model_frame() {
    let (mut i2c, delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    i2c.corrupt_next_crc();
    let mut sensor = Sht45::new(i2c, delay);

    assert!(matches!(
        block_on(sensor.measure(ph_sht45_hts::Repeatability::High)),
        Err(DriverError::Crc { word: 0, .. })
    ));
}

#[test]
fn public_measure_requires_delay_to_reach_the_model_frontier() {
    let (i2c, _delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    let mut sensor = Sht45::new(i2c, NoopDelay);

    assert!(matches!(
        block_on(sensor.measure(ph_sht45_hts::Repeatability::High)),
        Err(DriverError::NoAcknowledge(_))
    ));
}

#[test]
fn public_reset_aborts_an_in_flight_measurement_and_preserves_serial() {
    let (mut i2c, delay, events) = ModelI2c::with_measurement_ticks(0x1234_5678, 0xbeef, 0xbeef);
    let mut operations = [Operation::Write(&[MEASURE_HIGH_COMMAND])];
    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();
    events.borrow_mut().clear();

    let mut sensor = Sht45::new(i2c, delay);
    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        *events.borrow(),
        [
            AdapterEvent::Write {
                address: ADDRESS,
                bytes: vec![ph_sht45_hts_model::SOFT_RESET_COMMAND],
            },
            AdapterEvent::DelayNs(1_000_000),
        ]
    );

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
    assert_eq!(
        &events.borrow()[2..],
        [
            AdapterEvent::Write {
                address: ADDRESS,
                bytes: vec![SERIAL_NUMBER_COMMAND],
            },
            AdapterEvent::DelayNs(10_000),
            AdapterEvent::Read {
                address: ADDRESS,
                length: 6,
            },
        ]
    );
}

#[test]
fn public_reset_without_delay_does_not_fake_idle_recovery() {
    let (mut i2c, _delay, _events) = ModelI2c::with_measurement_ticks(0x1234_5678, 0xbeef, 0xbeef);
    let mut operations = [Operation::Write(&[MEASURE_HIGH_COMMAND])];
    block_on(i2c.transaction(ADDRESS, &mut operations)).unwrap();

    let mut sensor = Sht45::new(i2c, NoopDelay);
    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        block_on(sensor.read_serial_number()),
        Err(DriverError::I2c(AdapterError::Model(
            ModelError::WriteWhileBusy,
        )))
    );
}
