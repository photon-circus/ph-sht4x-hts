use embedded_hal::i2c::{ErrorType, Operation, SevenBitAddress};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use futures_lite::future::block_on;
use ph_sht4x_hts::{
    Address, Error as DriverError, HeaterDuration, HeaterPower, Measurement, Sht4x,
};
use ph_sht4x_hts_model::{
    Error as ModelError, MEASURE_HIGH_COMMAND, SERIAL_NUMBER_COMMAND, Sht4xModel,
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

type SharedModel = Rc<RefCell<Sht4xModel>>;
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
    corrupt_crc_at: Option<usize>,
}

struct ModelDelay {
    model: SharedModel,
    events: SharedEvents,
}

impl ModelI2c {
    fn new(serial: u32) -> (Self, ModelDelay, SharedEvents) {
        Self::at(Address::A, serial)
    }

    fn at(address: Address, serial: u32) -> (Self, ModelDelay, SharedEvents) {
        Self::with_model(
            // `Address` cannot name an undocumented value, so a failure here
            // would be a defect in this harness, not an unsupported input.
            Sht4xModel::at(address.bits(), serial).expect("Address is always documented"),
        )
    }

    fn with_measurement_ticks(
        serial: u32,
        temperature: u16,
        humidity: u16,
    ) -> (Self, ModelDelay, SharedEvents) {
        Self::with_model(
            Sht4xModel::at(Address::A.bits(), serial)
                .expect("Address is always documented")
                .with_measurement_ticks(temperature, humidity),
        )
    }

    fn with_model(model: Sht4xModel) -> (Self, ModelDelay, SharedEvents) {
        let model = Rc::new(RefCell::new(model));
        let events = Rc::new(RefCell::new(Vec::new()));
        let i2c = Self {
            model: Rc::clone(&model),
            events: Rc::clone(&events),
            corrupt_crc_at: None,
        };
        let delay = ModelDelay {
            model,
            events: Rc::clone(&events),
        };
        (i2c, delay, events)
    }

    /// Corrupt one CRC byte of the next model frame. Index 2 is the first
    /// word's CRC, index 5 the second's.
    fn corrupt_next_crc_at(&mut self, index: usize) {
        self.corrupt_crc_at = Some(index);
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
                if let Some(index) = self.corrupt_crc_at.take() {
                    response[index] ^= 1;
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
    let (i2c, delay, events) = ModelI2c::new(0x1234_5678);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
    assert_eq!(
        *events.borrow(),
        [
            AdapterEvent::Write {
                address: Address::A.bits(),
                bytes: vec![SERIAL_NUMBER_COMMAND],
            },
            AdapterEvent::DelayNs(10_000),
            AdapterEvent::Read {
                address: Address::A.bits(),
                length: 6,
            },
        ]
    );
}

#[test]
fn public_serial_read_rejects_a_corrupted_crc_on_either_word() {
    for (index, expected_word) in [(2, 0), (5, 1)] {
        let (mut i2c, delay, _events) = ModelI2c::new(0x1234_5678);
        i2c.corrupt_next_crc_at(index);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        match block_on(sensor.read_serial_number()) {
            Err(DriverError::Crc { word, .. }) => assert_eq!(word, expected_word),
            other => panic!("expected a CRC error on word {expected_word}, got {other:?}"),
        }
    }
}

/// The adapter is the conformance package's own code, so it owns this check.
/// The driver issues one operation per transaction; anything else would mean
/// the comparison is no longer exercising the two-STOP domain the model
/// declares, and the adapter must say so rather than guess.
#[test]
fn adapter_rejects_a_transaction_the_model_domain_does_not_cover() {
    let (mut i2c, _delay, _events) = ModelI2c::new(0xbeef_beef);
    let mut response = [0; 6];
    let mut operations = [
        Operation::Write(&[SERIAL_NUMBER_COMMAND]),
        Operation::Read(&mut response),
    ];

    assert_eq!(
        block_on(i2c.transaction(Address::A.bits(), &mut operations)),
        Err(AdapterError::UnexpectedOperation)
    );
}

/// A model limitation must not reach the driver dressed as a device response.
/// `Busy` is documented device behavior under `SHT45-I2C-XFER-001` and maps to
/// a NACK; everything else is the model saying it cannot answer, and must not
/// claim the device produced it.
#[test]
fn adapter_distinguishes_device_behavior_from_model_limitations() {
    use embedded_hal::i2c::{Error as _, ErrorKind, NoAcknowledgeSource};

    assert_eq!(
        AdapterError::Model(ModelError::Busy).kind(),
        ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
    );
    for limitation in [
        ModelError::ReadBeforeCommand,
        ModelError::WriteWhileBusy,
        ModelError::MeasurementDataUnavailable,
    ] {
        assert_eq!(AdapterError::Model(limitation).kind(), ErrorKind::Other);
    }
    assert_eq!(AdapterError::UnexpectedOperation.kind(), ErrorKind::Other);
}

/// `SHT4X-I2C-ADDR-001` makes the address a part-number property rather than a
/// constant, so the driver and the model must agree on it for each documented
/// value. A model fixed at one address could not discriminate a driver that
/// ignored the address it was constructed with; here each is built for the same
/// address independently, and a mismatch surfaces as the model's `WrongAddress`
/// through the driver's public error path.
#[test]
fn public_operations_conform_at_every_documented_address() {
    for address in [Address::A, Address::B, Address::C] {
        let (i2c, delay, events) = ModelI2c::at(address, 0x1234_5678);
        let mut sensor = Sht4x::new(address, i2c, delay);

        assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
        assert_eq!(
            *events.borrow(),
            [
                AdapterEvent::Write {
                    address: address.bits(),
                    bytes: vec![SERIAL_NUMBER_COMMAND],
                },
                AdapterEvent::DelayNs(10_000),
                AdapterEvent::Read {
                    address: address.bits(),
                    length: 6,
                },
            ]
        );
    }
}

#[test]
fn a_driver_addressing_the_wrong_device_is_visible_through_its_public_error() {
    // Build the model for one address and the driver for another. Nothing else
    // differs, so this fails only if the driver's address reaches the bus.
    let (i2c, delay, _events) = ModelI2c::at(Address::B, 0x1234_5678);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);

    assert_eq!(
        block_on(sensor.read_serial_number()),
        Err(DriverError::I2c(AdapterError::Model(
            ModelError::WrongAddress {
                expected: Address::B.bits(),
                actual: Address::A.bits(),
            }
        )))
    );
}

/// The driver derives CRC-8 with a bit-at-a-time shift register; the model
/// reduces four bits per table lookup. They are deliberately different
/// formulations of `SHT45-CRC-001` so that a defect in one cannot hide in the
/// other. This sweeps every 16-bit word through the model's frame and the
/// driver's validation to show the two agree across the whole input domain,
/// not merely on the datasheet's single vector.
#[test]
fn driver_and_model_crc_agree_across_every_word() {
    for word in 0..=u16::MAX {
        let serial = (u32::from(word) << 16) | u32::from(word);
        let (i2c, _delay, _events) = ModelI2c::new(serial);
        let mut sensor = Sht4x::new(Address::A, i2c, NoopDelay);

        assert_eq!(
            block_on(sensor.read_serial_number()),
            Ok(serial),
            "word {word:#06x}"
        );
    }
}

#[test]
fn public_measure_conforms_at_each_repeatability_frontier() {
    for (repeatability, expected_command, expected_delay_ns) in [
        (ph_sht4x_hts::Repeatability::High, 0xfd, 8_300_000),
        (ph_sht4x_hts::Repeatability::Medium, 0xf6, 4_500_000),
        (ph_sht4x_hts::Repeatability::Low, 0xe0, 1_600_000),
    ] {
        let (i2c, delay, events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        assert_eq!(
            block_on(sensor.measure(repeatability)),
            Ok(ph_sht4x_hts::Measurement {
                t_mdeg_c: 85_523,
                rh_milli_pct: 87_230,
            })
        );
        assert_eq!(
            *events.borrow(),
            [
                AdapterEvent::Write {
                    address: Address::A.bits(),
                    bytes: vec![expected_command],
                },
                AdapterEvent::DelayNs(expected_delay_ns),
                AdapterEvent::Read {
                    address: Address::A.bits(),
                    length: 6,
                },
            ]
        );
    }
}

#[test]
fn public_measure_rejects_an_adapter_corrupted_model_frame() {
    let (mut i2c, delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    i2c.corrupt_next_crc_at(2);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);

    assert!(matches!(
        block_on(sensor.measure(ph_sht4x_hts::Repeatability::High)),
        Err(DriverError::Crc { word: 0, .. })
    ));
}

#[test]
fn public_measure_requires_delay_to_reach_the_model_frontier() {
    let (i2c, _delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    let mut sensor = Sht4x::new(Address::A, i2c, NoopDelay);

    assert!(matches!(
        block_on(sensor.measure(ph_sht4x_hts::Repeatability::High)),
        Err(DriverError::NoAcknowledge(_))
    ));
}

#[test]
fn public_heater_pulse_conforms_for_all_power_and_duration_selections() {
    for (power, duration, expected_command, expected_delay_ns) in [
        (HeaterPower::High, HeaterDuration::Long, 0x39, 1_108_300_000),
        (
            HeaterPower::Medium,
            HeaterDuration::Long,
            0x2f,
            1_108_300_000,
        ),
        (HeaterPower::Low, HeaterDuration::Long, 0x1e, 1_108_300_000),
        (HeaterPower::High, HeaterDuration::Short, 0x32, 118_300_000),
        (
            HeaterPower::Medium,
            HeaterDuration::Short,
            0x24,
            118_300_000,
        ),
        (HeaterPower::Low, HeaterDuration::Short, 0x15, 118_300_000),
    ] {
        let (i2c, delay, events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        assert_eq!(
            block_on(sensor.heater_pulse(power, duration)),
            Ok(Measurement {
                t_mdeg_c: 85_523,
                rh_milli_pct: 87_230,
            })
        );
        assert_eq!(
            *events.borrow(),
            [
                AdapterEvent::Write {
                    address: Address::A.bits(),
                    bytes: vec![expected_command],
                },
                AdapterEvent::DelayNs(expected_delay_ns),
                AdapterEvent::Read {
                    address: Address::A.bits(),
                    length: 6,
                },
            ]
        );
    }
}

#[test]
fn public_heater_pulse_rejects_an_adapter_corrupted_model_frame() {
    let (mut i2c, delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    i2c.corrupt_next_crc_at(2);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);

    assert!(matches!(
        block_on(sensor.heater_pulse(HeaterPower::High, HeaterDuration::Short)),
        Err(DriverError::Crc { word: 0, .. })
    ));
}

#[test]
fn public_heater_pulse_requires_delay_to_reach_the_model_frontier() {
    let (i2c, _delay, _events) = ModelI2c::with_measurement_ticks(0, 0xbeef, 0xbeef);
    let mut sensor = Sht4x::new(Address::A, i2c, NoopDelay);

    assert!(matches!(
        block_on(sensor.heater_pulse(HeaterPower::High, HeaterDuration::Long)),
        Err(DriverError::NoAcknowledge(_))
    ));
}

#[test]
fn public_reset_aborts_an_in_flight_measurement_and_preserves_serial() {
    let (mut i2c, delay, events) = ModelI2c::with_measurement_ticks(0x1234_5678, 0xbeef, 0xbeef);
    let mut operations = [Operation::Write(&[MEASURE_HIGH_COMMAND])];
    block_on(i2c.transaction(Address::A.bits(), &mut operations)).unwrap();
    events.borrow_mut().clear();

    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        *events.borrow(),
        [
            AdapterEvent::Write {
                address: Address::A.bits(),
                bytes: vec![ph_sht4x_hts_model::SOFT_RESET_COMMAND],
            },
            AdapterEvent::DelayNs(1_000_000),
        ]
    );

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
    assert_eq!(
        &events.borrow()[2..],
        [
            AdapterEvent::Write {
                address: Address::A.bits(),
                bytes: vec![SERIAL_NUMBER_COMMAND],
            },
            AdapterEvent::DelayNs(10_000),
            AdapterEvent::Read {
                address: Address::A.bits(),
                length: 6,
            },
        ]
    );
}

#[test]
fn public_reset_without_delay_does_not_fake_idle_recovery() {
    let (mut i2c, _delay, _events) = ModelI2c::with_measurement_ticks(0x1234_5678, 0xbeef, 0xbeef);
    let mut operations = [Operation::Write(&[MEASURE_HIGH_COMMAND])];
    block_on(i2c.transaction(Address::A.bits(), &mut operations)).unwrap();

    let mut sensor = Sht4x::new(Address::A, i2c, NoopDelay);
    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        block_on(sensor.read_serial_number()),
        Err(DriverError::I2c(AdapterError::Model(
            ModelError::WriteWhileBusy,
        )))
    );
}

#[test]
fn public_reset_aborts_an_in_flight_heater_pulse_and_preserves_serial() {
    let (mut i2c, delay, events) = ModelI2c::with_measurement_ticks(0x1234_5678, 0xbeef, 0xbeef);
    let mut operations = [Operation::Write(&[0x39])];
    block_on(i2c.transaction(Address::A.bits(), &mut operations)).unwrap();
    events.borrow_mut().clear();

    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        *events.borrow(),
        [
            AdapterEvent::Write {
                address: Address::A.bits(),
                bytes: vec![ph_sht4x_hts_model::SOFT_RESET_COMMAND],
            },
            AdapterEvent::DelayNs(1_000_000),
        ]
    );

    assert_eq!(block_on(sensor.read_serial_number()), Ok(0x1234_5678));
    assert_eq!(
        &events.borrow()[2..],
        [
            AdapterEvent::Write {
                address: Address::A.bits(),
                bytes: vec![SERIAL_NUMBER_COMMAND],
            },
            AdapterEvent::DelayNs(10_000),
            AdapterEvent::Read {
                address: Address::A.bits(),
                length: 6,
            },
        ]
    );
}
