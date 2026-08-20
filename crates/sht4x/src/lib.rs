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

/// Errors returned by the SHT4x driver.
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

/// Measurement repeatability supported by the SHT4x.
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
/// requested duration, per `SHT45-HEAT-PWR-001`.
///
/// The wattages below are the datasheet's **typical** values at **VDD = 3.3 V**.
/// They are not a delivered or guaranteed figure: a typical value is not a
/// bound, and the figures are qualified to that one supply voltage. Treat them
/// as naming which documented level you selected, not as how much energy the
/// device will dissipate.
///
/// The driver selects the command byte. It does not meter delivered energy or
/// limit duty cycle, which stay with the caller under `SHT45-HEAT-SEQ-001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaterPower {
    /// Highest level, typically 200 mW at 3.3 V: `0x39` when long, `0x32` when short.
    High,
    /// Middle level, typically 110 mW at 3.3 V: `0x2F` when long, `0x24` when short.
    Medium,
    /// Lowest level, typically 20 mW at 3.3 V: `0x1E` when long, `0x15` when short.
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

/// An SHT4x connected to abstract asynchronous I2C and delay resources.
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
    ///
    /// Per `SHT45-SN-CMD-001` this is a command write, a wait for the command's
    /// execution time, and then a *separate* six-byte read — not a combined
    /// `write_read`. Both response words are CRC-validated under
    /// `SHT45-CRC-001`.
    ///
    /// For an SHT43 this serial is the key its ISO/IEC 17025 calibration
    /// certificate is filed under (`SHT4X-SHT43-CAL-001`). Retrieving or
    /// applying that certificate is outside this driver.
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
    ///
    /// Writes the command of `SHT45-RST-CMD-001`, which returns no payload, and
    /// then waits the `SHT45-RST-TIME-001` idle-time bound. Per
    /// `SHT45-RST-ABORT-001` this aborts an in-flight measurement or heater
    /// pulse.
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
    ///
    /// Selects the command of `SHT45-MEAS-CMD-001` for the requested
    /// repeatability and waits that repeatability's Table 5 **maximum** from
    /// `SHT45-MEAS-TIME-001` rather than its typical value, then reads and
    /// converts six bytes.
    ///
    /// Results are uncropped under `SHT45-MEAS-CONV-001`: a relative humidity
    /// outside 0–100 %RH is returned as computed rather than clamped.
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

#[inline(always)]
fn crc8(bytes: [u8; 2]) -> u8 {
    let mut crc = 0xff;

    for byte in bytes {
        crc ^= byte;

        for _ in 0..8 {
            let mask = 0u8.wrapping_sub(crc >> 7);
            crc = (crc << 1) ^ (0x31 & mask);
        }
    }

    crc
}

#[inline]
fn div_65535(n: u32) -> u32 {
    // Exact for the ranges used below.
    (n + (n >> 16) + 1) >> 16
}

fn convert_temperature(ticks: u16) -> i32 {
    let ticks = u32::from(ticks);

    // 175_000 = 2 * 65_535 + 43_930
    let fractional = div_65535(43_930 * ticks);

    -45_000 + (2 * ticks + fractional) as i32
}

fn convert_humidity(ticks: u16) -> i32 {
    let ticks = u32::from(ticks);

    // 125_000 = 65_535 + 59_465
    let fractional = div_65535(59_465 * ticks);

    -6_000 + (ticks + fractional) as i32
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
mod tests;
