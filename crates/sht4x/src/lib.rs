#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use embedded_hal_async::{delay::DelayNs, i2c::I2c};

/// The 7-bit I2C address an SHT4x answers on.
///
/// Fixed by position 7 of the part number under `SHT4X-PART-NOM-001`, and
/// recorded as `SHT4X-I2C-ADDR-001`. It is **not** a function of the sensor
/// model: an SHT40 ships at all three addresses and an SHT43 at two, so read
/// this off the part number you ordered rather than inferring it from whether
/// the part is an SHT40, SHT41, SHT43, or SHT45.
///
/// The variants are named for the part-number position they correspond to, so
/// the lookup is direct: `SHT40-BD1B-R2` has `B` at position 7 and therefore
/// answers on [`Address::B`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    /// Part-number position 7 `A`: 7-bit address `0x44`.
    A,
    /// Part-number position 7 `B`: 7-bit address `0x45`.
    B,
    /// Part-number position 7 `C`: 7-bit address `0x46`.
    C,
}

impl Address {
    /// The 7-bit address as it appears on the bus.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::A => 0x44,
            Self::B => 0x45,
            Self::C => 0x46,
        }
    }
}

const SERIAL_NUMBER_COMMAND: u8 = 0x89;
const SERIAL_NUMBER_DURATION_US: u32 = 10;
// Every response this driver reads is the same shape: two big-endian 16-bit
// words, each followed by its CRC-8 byte.
const RESPONSE_LEN: usize = 6;
const SOFT_RESET_COMMAND: u8 = 0x94;
const SOFT_RESET_DURATION_US: u32 = 1_000;
// Each heater bound is the pulse itself plus the high-repeatability
// measurement that runs before the data is available, per `SHT45-HEAT-TIME-001`
// and `SHT45-HEAT-SEQ-001`. Dropping the trailing 8_300 would read the frame
// before the device has it.
const HEATER_LONG_DURATION_US: u32 = 1_100_000 + MEASUREMENT_HIGH_DURATION_US;
const HEATER_SHORT_DURATION_US: u32 = 110_000 + MEASUREMENT_HIGH_DURATION_US;
const MEASUREMENT_HIGH_DURATION_US: u32 = 8_300;
const MEASUREMENT_MEDIUM_DURATION_US: u32 = 4_500;
const MEASUREMENT_LOW_DURATION_US: u32 = 1_600;

/// Errors returned by the SHT45 driver.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error<E> {
    /// The device or bus rejected the transfer for a reason other than NACK.
    I2c(E),
    /// The device was not ready or otherwise did not acknowledge the transfer.
    ///
    /// Per `SHT45-I2C-XFER-001` the device NACKs a read header while it is
    /// busy, so this is the expected result of reading before the operation's
    /// required wait has elapsed. The driver does not retry.
    NoAcknowledge(E),
    /// One of the two response words failed its CRC check.
    ///
    /// The driver does not retry a failed CRC, per `SHT45-CRC-001`.
    Crc {
        /// Which of the two 16-bit response words failed: `0` or `1`.
        ///
        /// For a measurement or heater pulse, word `0` is temperature and word
        /// `1` is relative humidity. For a serial-number read they are the
        /// high and low halves of the serial in transmission order.
        word: usize,
        /// The CRC-8 computed over the received word.
        expected: u8,
        /// The CRC-8 byte the device sent.
        actual: u8,
    },
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2c(error) => write!(f, "I2C transfer failed: {error}"),
            Self::NoAcknowledge(error) => {
                write!(f, "device did not acknowledge the transfer: {error}")
            }
            Self::Crc {
                word,
                expected,
                actual,
            } => write!(
                f,
                "CRC mismatch on response word {word}: computed {expected:#04x}, received {actual:#04x}"
            ),
        }
    }
}

impl<E> core::error::Error for Error<E>
where
    E: core::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::I2c(error) | Self::NoAcknowledge(error) => Some(error),
            Self::Crc { .. } => None,
        }
    }
}

/// Measurement repeatability supported by the SHT45.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeatability {
    /// High repeatability: maximum measurement duration 8.3 ms.
    High,
    /// Medium repeatability: maximum measurement duration 4.5 ms.
    Medium,
    /// Low repeatability: maximum measurement duration 1.6 ms.
    Low,
}

/// Heater power selected for one bounded heater pulse.
///
/// Each variant selects one of the three heater commands available for the
/// requested duration. Which documented power level each command carries is
/// recorded as `SHT45-HEAT-PWR-001`, and that record is **unverified**: its
/// figures have not been checked against the pinned datasheet. Read the variant
/// names as the retained reading of that ordering, not as a confirmed device
/// fact, and do not depend on a particular wattage.
///
/// The driver selects the command byte. It does not meter delivered energy or
/// limit duty cycle, which stay with the caller under `SHT45-HEAT-SEQ-001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaterPower {
    /// Command `0x39` when long, `0x32` when short; read as the highest level.
    High,
    /// Command `0x2F` when long, `0x24` when short; read as the middle level.
    Medium,
    /// Command `0x1E` when long, `0x15` when short; read as the lowest level.
    Low,
}

/// Duration selected for one bounded heater pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaterDuration {
    /// Long pulse: 1.1 s heating followed by a high-repeatability measurement.
    Long,
    /// Short pulse: 0.11 s heating followed by a high-repeatability measurement.
    Short,
}

/// One temperature and relative-humidity measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    /// Temperature in thousandths of a degree Celsius.
    pub t_mdeg_c: i32,
    /// Relative humidity in thousandths of a percent.
    pub rh_milli_pct: i32,
}

/// An SHT45 connected to abstract asynchronous I2C and delay resources.
pub struct Sht4x<I2C, DELAY> {
    address: Address,
    i2c: I2C,
    delay: DELAY,
}

impl<I2C, DELAY> Sht4x<I2C, DELAY> {
    /// Create a driver for one SHT4x at the given address.
    ///
    /// The address comes from position 7 of the part number, per
    /// [`Address`]. Every operation on the returned driver addresses that one
    /// device; nothing here scans the bus or infers which part is present.
    pub const fn new(address: Address, i2c: I2C, delay: DELAY) -> Self {
        Self {
            address,
            i2c,
            delay,
        }
    }

    /// The address this driver was constructed for.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Read the device's 32-bit transmission-order serial number.
    pub async fn read_serial_number<E>(&mut self) -> Result<u32, Error<E>>
    where
        I2C: I2c<Error = E>,
        DELAY: DelayNs,
        E: embedded_hal::i2c::Error,
    {
        self.i2c
            .write(self.address.bits(), &[SERIAL_NUMBER_COMMAND])
            .await
            .map_err(map_i2c_error)?;

        self.delay.delay_us(SERIAL_NUMBER_DURATION_US).await;

        let mut response = [0; RESPONSE_LEN];
        self.i2c
            .read(self.address.bits(), &mut response)
            .await
            .map_err(map_i2c_error)?;

        let (high, low) = decode_words(response)?;
        Ok((u32::from(high) << 16) | u32::from(low))
    }

    /// Perform a soft reset and wait for the device to return to idle.
    pub async fn reset<E>(&mut self) -> Result<(), Error<E>>
    where
        I2C: I2c<Error = E>,
        DELAY: DelayNs,
        E: embedded_hal::i2c::Error,
    {
        self.i2c
            .write(self.address.bits(), &[SOFT_RESET_COMMAND])
            .await
            .map_err(map_i2c_error)?;

        self.delay.delay_us(SOFT_RESET_DURATION_US).await;
        Ok(())
    }

    /// Perform one temperature and relative-humidity measurement.
    pub async fn measure<E>(
        &mut self,
        repeatability: Repeatability,
    ) -> Result<Measurement, Error<E>>
    where
        I2C: I2c<Error = E>,
        DELAY: DelayNs,
        E: embedded_hal::i2c::Error,
    {
        let (command, duration_us) = match repeatability {
            Repeatability::High => (0xfd, MEASUREMENT_HIGH_DURATION_US),
            Repeatability::Medium => (0xf6, MEASUREMENT_MEDIUM_DURATION_US),
            Repeatability::Low => (0xe0, MEASUREMENT_LOW_DURATION_US),
        };

        self.i2c
            .write(self.address.bits(), &[command])
            .await
            .map_err(map_i2c_error)?;

        self.delay.delay_us(duration_us).await;

        let mut response = [0; RESPONSE_LEN];
        self.i2c
            .read(self.address.bits(), &mut response)
            .await
            .map_err(map_i2c_error)?;

        decode_measurement(response)
    }

    /// Run one heater pulse and return its on-chip high-repeatability measurement.
    ///
    /// # The returned reading is not an ambient measurement
    ///
    /// Under `SHT45-HEAT-SEQ-001` the device converts *while the heater is still
    /// on*, so the returned [`Measurement`] describes the heated sensor rather
    /// than the surrounding air. How the two differ is heater physics, which this
    /// repository does not retain, model, bound, or correct. It shares the
    /// [`Measurement`] type with [`Sht4x::measure`] and is not a substitute for
    /// it: use [`Sht4x::measure`] for a reading taken with the heater off. What
    /// either reading implies about the surrounding air is a system-calibration
    /// question this repository does not answer.
    ///
    /// The caller owns application-level heater policy, including pulse cadence and
    /// duty-cycle limiting. This operation owns the selected command's complete
    /// device-required wait and response read: once the command write is
    /// acknowledged it does not return until that wait has elapsed — over one
    /// second for a long pulse. A write that fails returns immediately, without
    /// waiting or reading.
    pub async fn heater_pulse<E>(
        &mut self,
        power: HeaterPower,
        duration: HeaterDuration,
    ) -> Result<Measurement, Error<E>>
    where
        I2C: I2c<Error = E>,
        DELAY: DelayNs,
        E: embedded_hal::i2c::Error,
    {
        let (command, duration_us) = match (duration, power) {
            (HeaterDuration::Long, HeaterPower::High) => (0x39, HEATER_LONG_DURATION_US),
            (HeaterDuration::Long, HeaterPower::Medium) => (0x2f, HEATER_LONG_DURATION_US),
            (HeaterDuration::Long, HeaterPower::Low) => (0x1e, HEATER_LONG_DURATION_US),
            (HeaterDuration::Short, HeaterPower::High) => (0x32, HEATER_SHORT_DURATION_US),
            (HeaterDuration::Short, HeaterPower::Medium) => (0x24, HEATER_SHORT_DURATION_US),
            (HeaterDuration::Short, HeaterPower::Low) => (0x15, HEATER_SHORT_DURATION_US),
        };

        self.i2c
            .write(self.address.bits(), &[command])
            .await
            .map_err(map_i2c_error)?;

        self.delay.delay_us(duration_us).await;

        let mut response = [0; RESPONSE_LEN];
        self.i2c
            .read(self.address.bits(), &mut response)
            .await
            .map_err(map_i2c_error)?;

        decode_measurement(response)
    }

    /// Return the underlying I2C bus and delay resources.
    pub fn release(self) -> (I2C, DELAY) {
        (self.i2c, self.delay)
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

fn convert_temperature(ticks: u16) -> i32 {
    -45_000 + (175_000_i64 * i64::from(ticks) / 65_535) as i32
}

fn convert_humidity(ticks: u16) -> i32 {
    -6_000 + (125_000_i64 * i64::from(ticks) / 65_535) as i32
}

/// Validate both CRC-8 bytes and return the two 16-bit words in transmission
/// order.
///
/// Serial-number and measurement responses share this frame shape, so they
/// share this decode. A CRC failure names the word it failed on.
fn decode_words<E>(response: [u8; RESPONSE_LEN]) -> Result<(u16, u16), Error<E>> {
    let first = u16::from_be_bytes([response[0], response[1]]);
    let second = u16::from_be_bytes([response[3], response[4]]);
    for (word, (value, actual)) in [(first, response[2]), (second, response[5])]
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

    Ok((first, second))
}

fn decode_measurement<E>(response: [u8; RESPONSE_LEN]) -> Result<Measurement, Error<E>> {
    let (temperature, humidity) = decode_words(response)?;
    Ok(Measurement {
        t_mdeg_c: convert_temperature(temperature),
        rh_milli_pct: convert_humidity(humidity),
    })
}

#[cfg(test)]
mod tests {
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
}
