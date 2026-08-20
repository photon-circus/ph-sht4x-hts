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

/// Independent behavioral model of selected SHT4x operations.
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
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn models_the_required_serial_trace() {
        let mut model = Sht4xModel::new(0xbeef_beef);
        let mut response = [0; 6];
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    }

    #[test]
    fn models_high_measurement_busy_frontier_and_frame() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(8_299_999);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();

        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    }

    #[test]
    fn models_medium_and_low_measurement_frontiers() {
        for (command, duration_ns) in [
            (MEASURE_MEDIUM_COMMAND, 4_500_000),
            (MEASURE_LOW_COMMAND, 1_600_000),
        ] {
            let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
            let mut response = [0; 6];

            model.write(DEFAULT_ADDRESS, &[command]).unwrap();
            model.advance_ns(duration_ns - 1);
            assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
            model.advance_ns(1);
            model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        }
    }

    #[test]
    fn models_all_heater_frontiers_and_frame() {
        for (command, duration_ns) in HEATER_LONG_COMMANDS
            .into_iter()
            .map(|command| (command, LONG_HEATER_NS))
            .chain(
                HEATER_SHORT_COMMANDS
                    .into_iter()
                    .map(|command| (command, SHORT_HEATER_NS)),
            )
        {
            let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
            let mut response = [0; 6];

            model.write(DEFAULT_ADDRESS, &[command]).unwrap();
            model.advance_ns(duration_ns - 1);
            assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
            model.advance_ns(1);
            model.read(DEFAULT_ADDRESS, &mut response).unwrap();
            assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
            assert_eq!(
                model.read(DEFAULT_ADDRESS, &mut response),
                Err(Error::MeasurementDataUnavailable)
            );
        }
    }

    #[test]
    fn retains_split_delay_progress_and_consumes_measurement_once() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(4_000_000);
        model.advance_ns(4_300_000);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::MeasurementDataUnavailable)
        );
    }

    #[test]
    fn rejects_writes_while_busy_without_replacing_the_measurement() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(4_000_000);
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_LOW_COMMAND]),
            Err(Error::WriteWhileBusy)
        );

        model.advance_ns(4_300_000);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(
            response,
            [
                0x12,
                0x34,
                crc8([0x12, 0x34]),
                0x56,
                0x78,
                crc8([0x56, 0x78]),
            ]
        );
    }

    #[test]
    fn aborts_measurement_with_soft_reset_and_returns_to_idle() {
        let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(1_000_000);
        model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
        model.advance_ns(SOFT_RESET_NS - 1);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn aborts_heater_with_soft_reset_and_preserves_serial() {
        let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[HEATER_LONG_COMMANDS[0]])
            .unwrap();
        model.advance_ns(1_000_000);
        assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
        model.advance_ns(SOFT_RESET_NS - 1);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn soft_reset_is_the_only_busy_write_exception() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(1_000_000);
        assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]),
            Err(Error::ResetWhileBusy)
        );
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
    }

    #[test]
    fn rejects_non_reset_writes_while_heater_is_busy() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

        model
            .write(DEFAULT_ADDRESS, &[HEATER_SHORT_COMMANDS[0]])
            .unwrap();
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
    }

    #[test]
    fn busy_state_precedes_malformed_write_validation() {
        for pending_command in [MEASURE_HIGH_COMMAND, SOFT_RESET_COMMAND] {
            let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
            model.write(DEFAULT_ADDRESS, &[pending_command]).unwrap();

            for bytes in [&[][..], &[SERIAL_NUMBER_COMMAND, 0x00][..]] {
                assert_eq!(
                    model.write(DEFAULT_ADDRESS, bytes),
                    Err(Error::WriteWhileBusy)
                );
            }
        }
    }

    #[test]
    fn reset_accumulates_split_delay_and_works_when_idle() {
        let mut model = Sht4xModel::new(0x1234_5678);
        let mut response = [0; 6];

        model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
        model.advance_ns(400_000);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(600_000);
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn rejects_a_write_that_would_discard_a_pending_serial_response() {
        let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::WriteWithPendingResponse)
        );

        // The rejected write commits nothing: the serial frame is still there.
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn rejects_a_write_that_would_discard_ready_measurement_data() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(HIGH_MEASUREMENT_NS);
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_LOW_COMMAND]),
            Err(Error::WriteWithPendingResponse)
        );

        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(
            response,
            [
                0x12,
                0x34,
                crc8([0x12, 0x34]),
                0x56,
                0x78,
                crc8([0x56, 0x78]),
            ]
        );
    }

    #[test]
    fn rejects_a_write_that_would_discard_ready_heater_data() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model
            .write(DEFAULT_ADDRESS, &[HEATER_SHORT_COMMANDS[0]])
            .unwrap();
        model.advance_ns(SHORT_HEATER_NS);
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWithPendingResponse)
        );

        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    }

    #[test]
    fn soft_reset_does_not_escape_the_pending_response_boundary() {
        // Soft reset aborts a busy action under SHT45-RST-ABORT-001. Nothing in
        // the sources declares that it also discards an unconsumed response, so
        // that sequence stays an explicit model limitation rather than an
        // inferred device behavior.
        let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);

        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]),
            Err(Error::WriteWithPendingResponse)
        );
    }

    #[test]
    fn a_completed_reset_leaves_no_response_to_discard() {
        // Reset returns no payload, so a command after the reset interval is an
        // ordinary idle write and must not be caught by the pending-response
        // boundary. The public soft-reset conformance trace depends on this.
        let mut model = Sht4xModel::new(0x1234_5678);
        let mut response = [0; 6];

        model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
        model.advance_ns(SOFT_RESET_NS);
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn busy_state_precedes_the_pending_response_boundary() {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

        model
            .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
            .unwrap();
        model.advance_ns(HIGH_MEASUREMENT_NS - 1);
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
    }

    #[test]
    fn a_frame_the_device_would_not_act_on_keeps_its_own_error() {
        // None of these would discard the pending serial frame, so none of them
        // may be reported as `WriteWithPendingResponse`.
        let cases: [(&[u8], Error); 3] = [
            (
                &[],
                Error::InvalidWriteLength {
                    expected: 1,
                    actual: 0,
                },
            ),
            (
                &[SERIAL_NUMBER_COMMAND, 0x00],
                Error::InvalidWriteLength {
                    expected: 1,
                    actual: 2,
                },
            ),
            (&[0x2c], Error::UnsupportedCommand(0x2c)),
        ];

        for (bytes, expected) in cases {
            let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
            let mut response = [0; 6];
            model
                .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
                .unwrap();

            assert_eq!(model.write(DEFAULT_ADDRESS, bytes), Err(expected));

            // And the response it could not have discarded is still there.
            model.read(DEFAULT_ADDRESS, &mut response).unwrap();
            assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
        }
    }

    #[test]
    fn missing_ticks_are_reported_over_a_pending_response() {
        // A measurement with no injected ticks is a model-input error. The model
        // cannot act on it, so it cannot discard the pending frame either.
        let mut model = Sht4xModel::new(0x1234_5678);
        let mut response = [0; 6];
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();

        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::MissingMeasurementTicks)
        );

        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn requires_explicit_measurement_ticks() {
        let mut model = Sht4xModel::new(0);

        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::MissingMeasurementTicks)
        );
    }

    #[test]
    fn serial_read_is_stable_after_another_command() {
        let mut model = Sht4xModel::new(0x1234_5678);
        let mut first = [0; 6];
        let mut second = [0; 6];
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut first).unwrap();
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut second).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn owns_the_crc_vector() {
        let mut model = Sht4xModel::new(0xbeef_beef);
        let mut response = [0; 6];
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response[2], 0x92);
    }

    #[test]
    fn crc_remainder_table_matches_the_polynomial_it_claims() {
        // The table is data, so it is worth deriving independently of itself.
        // Entry i must be i << 4 divided by 0x31 over four most-significant-bit
        // shifts.
        for (index, entry) in CRC_NIBBLE_REMAINDERS.into_iter().enumerate() {
            let mut remainder = (index as u8) << 4;
            for _ in 0..4 {
                remainder = if remainder & 0x80 != 0 {
                    (remainder << 1) ^ 0x31
                } else {
                    remainder << 1
                };
            }
            assert_eq!(remainder, entry, "table entry {index}");
        }
    }

    #[test]
    fn answers_on_whichever_documented_address_it_was_given() {
        // The address is a part-number property, so a model fixed at one address
        // could not discriminate a driver that ignored its own.
        for address in ADDRESSES {
            let mut model = Sht4xModel::at(address, 0xbeef_beef).unwrap();
            let mut response = [0; 6];

            model.write(address, &[SERIAL_NUMBER_COMMAND]).unwrap();
            model.read(address, &mut response).unwrap();
            assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);

            let other = if address == ADDRESSES[0] {
                ADDRESSES[1]
            } else {
                ADDRESSES[0]
            };
            assert_eq!(
                model.write(other, &[SERIAL_NUMBER_COMMAND]),
                Err(Error::WrongAddress {
                    expected: address,
                    actual: other,
                })
            );
        }
    }

    #[test]
    fn refuses_to_model_a_device_at_an_undocumented_address() {
        // `SHT4X-I2C-ADDR-001` retains three addresses. Answering anywhere else
        // would invent a device the sources do not place there.
        for address in [0x00, 0x43, 0x47, 0xff] {
            assert!(matches!(
                Sht4xModel::at(address, 0xbeef_beef),
                Err(Error::UnsupportedAddress(reported)) if reported == address
            ));
        }
        for address in ADDRESSES {
            assert!(Sht4xModel::at(address, 0xbeef_beef).is_ok());
        }
    }

    #[test]
    fn rejects_non_modeled_transactions() {
        let mut model = Sht4xModel::new(0xbeef_beef);
        let mut response = [0; 6];
        assert_eq!(
            model.write(0x45, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WrongAddress {
                expected: DEFAULT_ADDRESS,
                actual: 0x45
            })
        );
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[0x2c]),
            Err(Error::UnsupportedCommand(0x2c))
        );
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut [0; 5]),
            Err(Error::InvalidReadLength(5))
        );
    }

    #[test]
    fn reports_malformed_command_frames_distinctly() {
        let mut model = Sht4xModel::new(0xbeef_beef);
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[]),
            Err(Error::InvalidWriteLength {
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(
            model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND, 0x00]),
            Err(Error::InvalidWriteLength {
                expected: 1,
                actual: 2,
            })
        );
    }
}
