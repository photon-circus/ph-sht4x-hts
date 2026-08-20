extern crate std;

use super::*;
use embedded_hal::i2c::{ErrorType, Operation, SevenBitAddress};
use futures_lite::future::block_on;
use std::{cell::RefCell, rc::Rc, vec, vec::Vec};

#[derive(Debug, PartialEq, Eq)]
enum FakeError {
    NoAcknowledge,
    Bus,
}

impl core::fmt::Display for FakeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoAcknowledge => f.write_str("device did not acknowledge"),
            Self::Bus => f.write_str("bus error"),
        }
    }
}

impl core::error::Error for FakeError {}

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
enum Event {
    Write(SevenBitAddress, Vec<u8>),
    DelayNs(u32),
    Read(SevenBitAddress, usize),
}

struct FakeI2c {
    events: Rc<RefCell<Vec<Event>>>,
    response: [u8; 6],
    write_error: Option<FakeError>,
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
                    self.events
                        .borrow_mut()
                        .push(Event::Write(address, bytes.to_vec()));
                    if let Some(error) = self.write_error.take() {
                        return Err(error);
                    }
                }
                Operation::Read(bytes) => {
                    self.events
                        .borrow_mut()
                        .push(Event::Read(address, bytes.len()));
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

struct FakeDelay {
    events: Rc<RefCell<Vec<Event>>>,
}

impl DelayNs for FakeDelay {
    async fn delay_ns(&mut self, ns: u32) {
        self.events.borrow_mut().push(Event::DelayNs(ns));
    }
}

fn fake(response: [u8; 6]) -> (FakeI2c, FakeDelay, Rc<RefCell<Vec<Event>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (
        FakeI2c {
            events: Rc::clone(&events),
            response,
            write_error: None,
            read_error: None,
        },
        FakeDelay {
            events: Rc::clone(&events),
        },
        events,
    )
}

#[test]
fn errors_render_the_detail_a_caller_needs() {
    use std::string::ToString;

    assert_eq!(
        Error::<FakeError>::Crc {
            word: 1,
            expected: 0x92,
            actual: 0x93,
        }
        .to_string(),
        "CRC mismatch on response word 1: computed 0x92, received 0x93"
    );
    assert_eq!(
        Error::NoAcknowledge(FakeError::NoAcknowledge).to_string(),
        "device did not acknowledge the transfer: device did not acknowledge"
    );
    assert_eq!(
        Error::I2c(FakeError::Bus).to_string(),
        "I2C transfer failed: bus error"
    );
}

#[test]
fn transport_errors_are_reachable_as_a_source() {
    use core::error::Error as _;

    assert!(
        Error::I2c(FakeError::Bus)
            .source()
            .is_some_and(|source| source.is::<FakeError>())
    );
    assert!(
        Error::<FakeError>::Crc {
            word: 0,
            expected: 0,
            actual: 1,
        }
        .source()
        .is_none()
    );
}

#[test]
fn every_documented_address_is_used_on_the_bus() {
    // `SHT4X-I2C-ADDR-001`: the address comes from the part number, so the
    // driver must address what it was constructed for rather than a constant.
    for (address, bits) in [(Address::A, 0x44), (Address::B, 0x45), (Address::C, 0x46)] {
        let (i2c, delay, events) = fake([0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
        let mut sensor = Sht4x::new(address, i2c, delay);

        assert_eq!(sensor.address(), address);
        assert_eq!(block_on(sensor.read_serial_number()), Ok(0xbeef_beef));
        assert_eq!(
            *events.borrow(),
            vec![
                Event::Write(bits, vec![SERIAL_NUMBER_COMMAND]),
                Event::DelayNs(10_000),
                Event::Read(bits, 6),
            ]
        );
    }
}

#[test]
fn crc_vector_matches_datasheet_example() {
    assert_eq!(crc8([0xbe, 0xef]), 0x92);
}

#[test]
fn converts_measurement_vectors_without_cropping() {
    assert_eq!(convert_temperature(0), -45_000);
    assert_eq!(convert_humidity(0), -6_000);
    assert_eq!(convert_temperature(u16::MAX), 130_000);
    assert_eq!(convert_humidity(u16::MAX), 119_000);
    assert_eq!(convert_temperature(0xbeef), 85_523);
    assert_eq!(convert_humidity(0xbeef), 87_230);
}

#[test]
fn measures_at_each_repeatability_with_the_required_command_and_delay() {
    for (repeatability, command, delay_ns) in [
        (Repeatability::High, 0xfd, 8_300_000),
        (Repeatability::Medium, 0xf6, 4_500_000),
        (Repeatability::Low, 0xe0, 1_600_000),
    ] {
        let (i2c, delay, events) = fake([0, 0, 0x81, 0, 0, 0x81]);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);
        assert_eq!(
            block_on(sensor.measure(repeatability)),
            Ok(Measurement {
                t_mdeg_c: -45_000,
                rh_milli_pct: -6_000,
            })
        );
        assert_eq!(
            *events.borrow(),
            vec![
                Event::Write(Address::A.bits(), vec![command]),
                Event::DelayNs(delay_ns),
                Event::Read(Address::A.bits(), 6),
            ]
        );
    }
}

#[test]
fn measures_and_converts_both_words() {
    let (i2c, delay, _) = fake([0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(
        block_on(sensor.measure(Repeatability::High)),
        Ok(Measurement {
            t_mdeg_c: 85_523,
            rh_milli_pct: 87_230,
        })
    );
}

#[test]
fn runs_each_heater_pulse_with_the_required_command_and_delay() {
    for (power, duration, command, delay_ns) in [
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
        let (i2c, delay, events) = fake([0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
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
            vec![
                Event::Write(Address::A.bits(), vec![command]),
                Event::DelayNs(delay_ns),
                Event::Read(Address::A.bits(), 6),
            ]
        );
    }
}

#[test]
fn rejects_each_corrupt_heater_measurement_crc() {
    for index in [2, 5] {
        let mut response = [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92];
        response[index] ^= 1;
        let (i2c, delay, _) = fake(response);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);
        assert!(matches!(
            block_on(sensor.heater_pulse(HeaterPower::Low, HeaterDuration::Short)),
            Err(Error::Crc { .. })
        ));
    }
}

#[test]
fn maps_heater_write_errors_without_delaying_or_reading() {
    for (write_error, expected) in [
        (
            FakeError::NoAcknowledge,
            Error::NoAcknowledge(FakeError::NoAcknowledge),
        ),
        (FakeError::Bus, Error::I2c(FakeError::Bus)),
    ] {
        let (mut i2c, delay, events) = fake([0; 6]);
        i2c.write_error = Some(write_error);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        assert_eq!(
            block_on(sensor.heater_pulse(HeaterPower::High, HeaterDuration::Long)),
            Err(expected)
        );
        assert_eq!(
            *events.borrow(),
            vec![Event::Write(Address::A.bits(), vec![0x39])]
        );
    }
}

#[test]
fn surfaces_heater_read_errors_after_the_required_wait() {
    for (read_error, expected) in [
        (
            FakeError::NoAcknowledge,
            Error::NoAcknowledge(FakeError::NoAcknowledge),
        ),
        (FakeError::Bus, Error::I2c(FakeError::Bus)),
    ] {
        let (mut i2c, delay, events) = fake([0; 6]);
        i2c.read_error = Some(read_error);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        assert_eq!(
            block_on(sensor.heater_pulse(HeaterPower::Medium, HeaterDuration::Short)),
            Err(expected)
        );
        assert_eq!(
            *events.borrow(),
            vec![
                Event::Write(Address::A.bits(), vec![0x24]),
                Event::DelayNs(118_300_000),
                Event::Read(Address::A.bits(), 6),
            ]
        );
    }
}

#[test]
fn rejects_each_corrupt_measurement_crc() {
    for index in [2, 5] {
        let mut response = [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92];
        response[index] ^= 1;
        let (i2c, delay, _) = fake(response);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);
        assert!(matches!(
            block_on(sensor.measure(Repeatability::Medium)),
            Err(Error::Crc { .. })
        ));
    }
}

#[test]
fn surfaces_measurement_read_errors() {
    for (read_error, expected) in [
        (
            FakeError::NoAcknowledge,
            Error::NoAcknowledge(FakeError::NoAcknowledge),
        ),
        (FakeError::Bus, Error::I2c(FakeError::Bus)),
    ] {
        let (mut i2c, delay, _) = fake([0, 0, 0x81, 0, 0, 0x81]);
        i2c.read_error = Some(read_error);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);
        assert_eq!(block_on(sensor.measure(Repeatability::Low)), Err(expected));
    }
}

#[test]
fn reads_serial_with_two_transactions_and_validates_crc() {
    let (i2c, delay, events) = fake([0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(block_on(sensor.read_serial_number()), Ok(0xbeef_beef));
    assert_eq!(
        *events.borrow(),
        vec![
            Event::Write(Address::A.bits(), vec![SERIAL_NUMBER_COMMAND]),
            Event::DelayNs(10_000),
            Event::Read(Address::A.bits(), 6),
        ]
    );
}

#[test]
fn resets_with_one_write_and_one_millisecond_delay_without_reading() {
    let (i2c, delay, events) = fake([0; 6]);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);

    assert_eq!(block_on(sensor.reset()), Ok(()));
    assert_eq!(
        *events.borrow(),
        vec![
            Event::Write(Address::A.bits(), vec![SOFT_RESET_COMMAND]),
            Event::DelayNs(1_000_000),
        ]
    );
}

#[test]
fn maps_reset_write_errors_without_delaying() {
    for (write_error, expected) in [
        (
            FakeError::NoAcknowledge,
            Error::NoAcknowledge(FakeError::NoAcknowledge),
        ),
        (FakeError::Bus, Error::I2c(FakeError::Bus)),
    ] {
        let (mut i2c, delay, events) = fake([0; 6]);
        i2c.write_error = Some(write_error);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);

        assert_eq!(block_on(sensor.reset()), Err(expected));
        assert_eq!(
            *events.borrow(),
            vec![Event::Write(Address::A.bits(), vec![SOFT_RESET_COMMAND])]
        );
    }
}

#[test]
fn rejects_each_corrupt_crc() {
    for index in [2, 5] {
        let mut response = [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92];
        response[index] ^= 1;
        let (i2c, delay, _) = fake(response);
        let mut sensor = Sht4x::new(Address::A, i2c, delay);
        assert!(matches!(
            block_on(sensor.read_serial_number()),
            Err(Error::Crc { .. })
        ));
    }
}

#[test]
fn surfaces_nack_as_not_acknowledge_error() {
    let (mut i2c, delay, _) = fake([0; 6]);
    i2c.read_error = Some(FakeError::NoAcknowledge);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(
        block_on(sensor.read_serial_number()),
        Err(Error::NoAcknowledge(FakeError::NoAcknowledge))
    );
}

#[test]
fn preserves_non_nack_bus_error() {
    let (mut i2c, delay, _) = fake([0; 6]);
    i2c.read_error = Some(FakeError::Bus);
    let mut sensor = Sht4x::new(Address::A, i2c, delay);
    assert_eq!(
        block_on(sensor.read_serial_number()),
        Err(Error::I2c(FakeError::Bus))
    );
}
