#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// The 7-bit I2C addresses an SHT4x can answer on, per `SHT4X-I2C-ADDR-001`.
///
/// Ordered as part-number position 7 `A`, `B`, `C`.
pub const ADDRESSES: [u8; 3] = [0x44, 0x45, 0x46];
/// The address the model uses when none is chosen.
pub const DEFAULT_ADDRESS: u8 = ADDRESSES[0];
/// The modeled serial-number command byte.
pub const SERIAL_NUMBER_COMMAND: u8 = 0x89;
/// The modeled high-repeatability measurement command byte.
pub const MEASURE_HIGH_COMMAND: u8 = 0xfd;
/// The modeled medium-repeatability measurement command byte.
pub const MEASURE_MEDIUM_COMMAND: u8 = 0xf6;
/// The modeled low-repeatability measurement command byte.
pub const MEASURE_LOW_COMMAND: u8 = 0xe0;
/// The modeled soft-reset command byte.
pub const SOFT_RESET_COMMAND: u8 = 0x94;
/// The modeled long heater command bytes, in the order `SHT45-HEAT-PWR-001`
/// reads as descending power.
///
/// The model's behavior does not depend on that order; it accepts all three
/// bytes identically, since power is not observable at the transport boundary.
pub const HEATER_LONG_COMMANDS: [u8; 3] = [0x39, 0x2f, 0x1e];
/// The modeled short heater command bytes, in the order `SHT45-HEAT-PWR-001`
/// reads as descending power.
///
/// The model's behavior does not depend on that order; it accepts all three
/// bytes identically, since power is not observable at the transport boundary.
pub const HEATER_SHORT_COMMANDS: [u8; 3] = [0x32, 0x24, 0x15];
const RESPONSE_LEN: usize = 6;

const HIGH_MEASUREMENT_NS: u64 = 8_300_000;
const MEDIUM_MEASUREMENT_NS: u64 = 4_500_000;
const LOW_MEASUREMENT_NS: u64 = 1_600_000;
const SOFT_RESET_NS: u64 = 1_000_000;
const LONG_HEATER_NS: u64 = 1_108_300_000;
const SHORT_HEATER_NS: u64 = 118_300_000;

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
    /// A read was attempted without a preceding command write.
    ReadBeforeCommand,
    /// A measurement or reset read was attempted before its maximum completion time.
    Busy,
    /// A command write was attempted while a device action was busy, outside model fidelity.
    WriteWhileBusy,
    /// A nested soft reset was attempted while reset was already busy.
    ResetWhileBusy,
    /// Construction was attempted at an address no SHT4x is documented at.
    ///
    /// `SHT4X-I2C-ADDR-001` retains `0x44`, `0x45`, and `0x46`. Serving modeled
    /// frames anywhere else would invent a device the sources do not place
    /// there.
    UnsupportedAddress(u8),
    /// A command write would have discarded an unconsumed response.
    ///
    /// The sources do not declare what the device does when a new command
    /// arrives while a serial frame, or completed measurement or heater data,
    /// is still waiting to be read. The model rejects the sequence as an
    /// explicit limitation rather than choosing a plausible outcome.
    WriteWithPendingResponse,
    /// A measurement was requested without explicit conversion ticks.
    MissingMeasurementTicks,
    /// A completed measurement was already consumed or is otherwise unavailable.
    MeasurementDataUnavailable,
}

/// Explicit conversion ticks injected into a modeled measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementTicks {
    /// Raw temperature conversion ticks.
    pub temperature: u16,
    /// Raw relative-humidity conversion ticks.
    pub humidity: u16,
}

#[derive(Clone, Copy)]
enum PendingCommand {
    Serial,
    Measurement {
        ready_at_ns: u64,
        ticks: MeasurementTicks,
    },
    Heater {
        ready_at_ns: u64,
        ticks: MeasurementTicks,
    },
    Reset {
        ready_at_ns: u64,
    },
}

/// Independent behavioral model of selected SHT45 operations.
pub struct Sht4xModel {
    address: u8,
    serial: u32,
    measurement_ticks: Option<MeasurementTicks>,
    elapsed_ns: u64,
    pending: Option<PendingCommand>,
    measurement_consumed: bool,
}

impl Sht4xModel {
    /// Create a model at `DEFAULT_ADDRESS` with an explicit OTP serial number.
    pub const fn new(serial: u32) -> Self {
        Self::unchecked(DEFAULT_ADDRESS, serial)
    }

    /// Create a model answering on an explicit 7-bit address.
    ///
    /// The address is an input rather than a constant because it is a property
    /// of the part number, not of the sensor model — `SHT4X-I2C-ADDR-001`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedAddress`] for anything outside [`ADDRESSES`].
    /// The retained sources place an SHT4x at three addresses; a model that
    /// answered elsewhere would be inventing a device rather than predicting
    /// one, which is the value-domain leakage the behavioral-model standard
    /// forbids.
    pub const fn at(address: u8, serial: u32) -> Result<Self, Error> {
        // The contiguous range is `ADDRESSES`; the sweep in
        // `refuses_to_model_a_device_at_an_undocumented_address` fails if the
        // two ever drift apart.
        match address {
            0x44..=0x46 => Ok(Self::unchecked(address, serial)),
            other => Err(Error::UnsupportedAddress(other)),
        }
    }

    const fn unchecked(address: u8, serial: u32) -> Self {
        Self {
            address,
            serial,
            measurement_ticks: None,
            elapsed_ns: 0,
            pending: None,
            measurement_consumed: false,
        }
    }

    /// Inject explicit raw conversion ticks for subsequent measurements.
    pub const fn with_measurement_ticks(mut self, temperature: u16, humidity: u16) -> Self {
        self.measurement_ticks = Some(MeasurementTicks {
            temperature,
            humidity,
        });
        self
    }

    /// Advance modeled relative time without using a wall clock.
    pub fn advance_ns(&mut self, nanoseconds: u64) {
        self.elapsed_ns = self.elapsed_ns.saturating_add(nanoseconds);
    }

    /// Apply the modeled command write, including its STOP boundary.
    pub fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        if address != self.address {
            return Err(Error::WrongAddress {
                expected: self.address,
                actual: address,
            });
        }
        let action_busy = matches!(
            self.pending,
            Some(PendingCommand::Measurement { ready_at_ns, .. }
                | PendingCommand::Heater { ready_at_ns, .. })
                if self.elapsed_ns < ready_at_ns
        );
        let reset_busy = matches!(
            self.pending,
            Some(PendingCommand::Reset { ready_at_ns }) if self.elapsed_ns < ready_at_ns
        );
        if action_busy && bytes != [SOFT_RESET_COMMAND] {
            return Err(Error::WriteWhileBusy);
        }
        if reset_busy {
            return if bytes == [SOFT_RESET_COMMAND] {
                Err(Error::ResetWhileBusy)
            } else {
                Err(Error::WriteWhileBusy)
            };
        }
        if bytes.len() != 1 {
            return Err(Error::InvalidWriteLength {
                expected: 1,
                actual: bytes.len(),
            });
        }

        // Resolve the write into the state it would produce before touching
        // anything. A frame the device would not act on — a malformed length, an
        // unsupported command, a measurement with no injected ticks — cannot
        // discard a pending response, so its own error has to survive the
        // pending-response guard below rather than be masked by it.
        let next = match bytes[0] {
            SOFT_RESET_COMMAND => PendingCommand::Reset {
                ready_at_ns: self.elapsed_ns.saturating_add(SOFT_RESET_NS),
            },
            SERIAL_NUMBER_COMMAND => PendingCommand::Serial,
            command @ (MEASURE_HIGH_COMMAND | MEASURE_MEDIUM_COMMAND | MEASURE_LOW_COMMAND) => {
                let ticks = self
                    .measurement_ticks
                    .ok_or(Error::MissingMeasurementTicks)?;
                let duration_ns = match command {
                    MEASURE_HIGH_COMMAND => HIGH_MEASUREMENT_NS,
                    MEASURE_MEDIUM_COMMAND => MEDIUM_MEASUREMENT_NS,
                    _ => LOW_MEASUREMENT_NS,
                };
                PendingCommand::Measurement {
                    ready_at_ns: self.elapsed_ns.saturating_add(duration_ns),
                    ticks,
                }
            }
            command
                if HEATER_LONG_COMMANDS.contains(&command)
                    || HEATER_SHORT_COMMANDS.contains(&command) =>
            {
                let ticks = self
                    .measurement_ticks
                    .ok_or(Error::MissingMeasurementTicks)?;
                let duration_ns = if HEATER_LONG_COMMANDS.contains(&command) {
                    LONG_HEATER_NS
                } else {
                    SHORT_HEATER_NS
                };
                PendingCommand::Heater {
                    ready_at_ns: self.elapsed_ns.saturating_add(duration_ns),
                    ticks,
                }
            }
            command => return Err(Error::UnsupportedCommand(command)),
        };

        let response_pending = matches!(self.pending, Some(PendingCommand::Serial))
            || matches!(
                self.pending,
                Some(PendingCommand::Measurement { ready_at_ns, .. }
                    | PendingCommand::Heater { ready_at_ns, .. })
                    if self.elapsed_ns >= ready_at_ns
            );
        if response_pending {
            return Err(Error::WriteWithPendingResponse);
        }

        self.pending = Some(next);
        self.measurement_consumed = false;
        Ok(())
    }

    /// Fill the modeled six-byte response, including its STOP boundary.
    pub fn read(&mut self, address: u8, response: &mut [u8]) -> Result<(), Error> {
        if address != self.address {
            return Err(Error::WrongAddress {
                expected: self.address,
                actual: address,
            });
        }
        if response.len() != RESPONSE_LEN {
            return Err(Error::InvalidReadLength(response.len()));
        }
        match self.pending {
            Some(PendingCommand::Serial) => {
                write_frame(response, [(self.serial >> 16) as u16, self.serial as u16]);
                self.pending = None;
                self.measurement_consumed = false;
                Ok(())
            }
            // A heater pulse's trailing conversion returns the same
            // high-repeatability frame a measurement does, per
            // `SHT45-HEAT-CMD-001`. Only the frontier differs, and that is
            // already carried in `ready_at_ns`.
            Some(
                PendingCommand::Measurement { ready_at_ns, ticks }
                | PendingCommand::Heater { ready_at_ns, ticks },
            ) => {
                if self.elapsed_ns < ready_at_ns {
                    return Err(Error::Busy);
                }
                write_frame(response, [ticks.temperature, ticks.humidity]);
                self.pending = None;
                self.measurement_consumed = true;
                Ok(())
            }
            Some(PendingCommand::Reset { ready_at_ns }) => {
                if self.elapsed_ns < ready_at_ns {
                    return Err(Error::Busy);
                }
                self.pending = None;
                self.measurement_consumed = false;
                Err(Error::ReadBeforeCommand)
            }
            None if self.measurement_consumed => Err(Error::MeasurementDataUnavailable),
            None => Err(Error::ReadBeforeCommand),
        }
    }
}

/// Lay two big-endian words and their CRC bytes into a six-byte response.
fn write_frame(response: &mut [u8], words: [u16; 2]) {
    for (index, word) in words.into_iter().enumerate() {
        let bytes = word.to_be_bytes();
        let offset = index * 3;
        response[offset..offset + 2].copy_from_slice(&bytes);
        response[offset + 2] = crc8(bytes);
    }
}

/// Remainders of each four-bit message nibble under the `SHT45-CRC-001`
/// polynomial, most-significant bit first.
///
/// Entry `i` is the remainder left by dividing `i << 4` by `0x31` over four
/// shifts. Reducing four bits per lookup is a different derivation from the
/// driver's bit-at-a-time shift register, which is the point: a defect in one
/// formulation does not reproduce itself in the other, so driver-versus-model
/// comparison can still discriminate on the CRC.
const CRC_NIBBLE_REMAINDERS: [u8; 16] = [
    0x00, 0x31, 0x62, 0x53, 0xc4, 0xf5, 0xa6, 0x97, 0xb9, 0x88, 0xdb, 0xea, 0x7d, 0x4c, 0x1f, 0x2e,
];

fn crc8(bytes: [u8; 2]) -> u8 {
    let mut crc = 0xff_u8;
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0f] {
            let index = ((crc >> 4) ^ nibble) & 0x0f;
            crc = (crc << 4) ^ CRC_NIBBLE_REMAINDERS[index as usize];
        }
    }
    crc
}

#[cfg(test)]
mod tests;
