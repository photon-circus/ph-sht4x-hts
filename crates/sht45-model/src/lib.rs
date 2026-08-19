#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// The modeled SHT45-AD1B 7-bit I2C address.
pub const ADDRESS: u8 = 0x44;
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
/// The modeled long heater command bytes, ordered 200 mW, 110 mW, 20 mW.
///
/// The power each byte selects is recorded as `SHT45-HEAT-PWR-001`; this array
/// carries that order so a consumer can index it by power level.
pub const HEATER_LONG_COMMANDS: [u8; 3] = [0x39, 0x2f, 0x1e];
/// The modeled short heater command bytes, ordered 200 mW, 110 mW, 20 mW.
///
/// The power each byte selects is recorded as `SHT45-HEAT-PWR-001`; this array
/// carries that order so a consumer can index it by power level.
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
pub struct Sht45Model {
    serial: u32,
    measurement_ticks: Option<MeasurementTicks>,
    elapsed_ns: u64,
    pending: Option<PendingCommand>,
    measurement_consumed: bool,
}

impl Sht45Model {
    /// Create a model with an explicit OTP serial number.
    pub const fn new(serial: u32) -> Self {
        Self {
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
        if address != ADDRESS {
            return Err(Error::WrongAddress {
                expected: ADDRESS,
                actual: address,
            });
        }
        let action_busy = matches!(
            self.pending,
            Some(PendingCommand::Measurement { ready_at_ns, .. })
                | Some(PendingCommand::Heater { ready_at_ns, .. })
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
        match bytes[0] {
            SOFT_RESET_COMMAND => {
                self.pending = Some(PendingCommand::Reset {
                    ready_at_ns: self.elapsed_ns.saturating_add(SOFT_RESET_NS),
                });
                self.measurement_consumed = false;
                Ok(())
            }
            SERIAL_NUMBER_COMMAND => {
                self.pending = Some(PendingCommand::Serial);
                self.measurement_consumed = false;
                Ok(())
            }
            MEASURE_HIGH_COMMAND | MEASURE_MEDIUM_COMMAND | MEASURE_LOW_COMMAND => {
                let ticks = self
                    .measurement_ticks
                    .ok_or(Error::MissingMeasurementTicks)?;
                let duration_ns = match bytes[0] {
                    MEASURE_HIGH_COMMAND => HIGH_MEASUREMENT_NS,
                    MEASURE_MEDIUM_COMMAND => MEDIUM_MEASUREMENT_NS,
                    MEASURE_LOW_COMMAND => LOW_MEASUREMENT_NS,
                    _ => unreachable!(),
                };
                self.pending = Some(PendingCommand::Measurement {
                    ready_at_ns: self.elapsed_ns.saturating_add(duration_ns),
                    ticks,
                });
                self.measurement_consumed = false;
                Ok(())
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
                self.pending = Some(PendingCommand::Heater {
                    ready_at_ns: self.elapsed_ns.saturating_add(duration_ns),
                    ticks,
                });
                self.measurement_consumed = false;
                Ok(())
            }
            command => Err(Error::UnsupportedCommand(command)),
        }
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
        match self.pending {
            Some(PendingCommand::Serial) => {
                let words = [(self.serial >> 16) as u16, self.serial as u16];
                for (index, word) in words.into_iter().enumerate() {
                    let bytes = word.to_be_bytes();
                    let offset = index * 3;
                    response[offset..offset + 2].copy_from_slice(&bytes);
                    response[offset + 2] = crc8(bytes);
                }
                self.pending = None;
                self.measurement_consumed = false;
                Ok(())
            }
            Some(PendingCommand::Measurement { ready_at_ns, ticks }) => {
                if self.elapsed_ns < ready_at_ns {
                    return Err(Error::Busy);
                }
                let words = [ticks.temperature, ticks.humidity];
                for (index, word) in words.into_iter().enumerate() {
                    let bytes = word.to_be_bytes();
                    let offset = index * 3;
                    response[offset..offset + 2].copy_from_slice(&bytes);
                    response[offset + 2] = crc8(bytes);
                }
                self.pending = None;
                self.measurement_consumed = true;
                Ok(())
            }
            Some(PendingCommand::Heater { ready_at_ns, ticks }) => {
                if self.elapsed_ns < ready_at_ns {
                    return Err(Error::Busy);
                }
                let words = [ticks.temperature, ticks.humidity];
                for (index, word) in words.into_iter().enumerate() {
                    let bytes = word.to_be_bytes();
                    let offset = index * 3;
                    response[offset..offset + 2].copy_from_slice(&bytes);
                    response[offset + 2] = crc8(bytes);
                }
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
    fn models_high_measurement_busy_frontier_and_frame() {
        let mut model = Sht45Model::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]).unwrap();
        model.advance_ns(8_299_999);
        assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        model.read(ADDRESS, &mut response).unwrap();

        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
    }

    #[test]
    fn models_medium_and_low_measurement_frontiers() {
        for (command, duration_ns) in [
            (MEASURE_MEDIUM_COMMAND, 4_500_000),
            (MEASURE_LOW_COMMAND, 1_600_000),
        ] {
            let mut model = Sht45Model::new(0).with_measurement_ticks(0x1234, 0x5678);
            let mut response = [0; 6];

            model.write(ADDRESS, &[command]).unwrap();
            model.advance_ns(duration_ns - 1);
            assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
            model.advance_ns(1);
            model.read(ADDRESS, &mut response).unwrap();
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
            let mut model = Sht45Model::new(0).with_measurement_ticks(0xbeef, 0xbeef);
            let mut response = [0; 6];

            model.write(ADDRESS, &[command]).unwrap();
            model.advance_ns(duration_ns - 1);
            assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
            model.advance_ns(1);
            model.read(ADDRESS, &mut response).unwrap();
            assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
            assert_eq!(
                model.read(ADDRESS, &mut response),
                Err(Error::MeasurementDataUnavailable)
            );
        }
    }

    #[test]
    fn retains_split_delay_progress_and_consumes_measurement_once() {
        let mut model = Sht45Model::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]).unwrap();
        model.advance_ns(4_000_000);
        model.advance_ns(4_300_000);
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(
            model.read(ADDRESS, &mut response),
            Err(Error::MeasurementDataUnavailable)
        );
    }

    #[test]
    fn rejects_writes_while_busy_without_replacing_the_measurement() {
        let mut model = Sht45Model::new(0).with_measurement_ticks(0x1234, 0x5678);
        let mut response = [0; 6];

        model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]).unwrap();
        model.advance_ns(4_000_000);
        assert_eq!(
            model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(
            model.write(ADDRESS, &[MEASURE_LOW_COMMAND]),
            Err(Error::WriteWhileBusy)
        );

        model.advance_ns(4_300_000);
        model.read(ADDRESS, &mut response).unwrap();
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
        let mut model = Sht45Model::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]).unwrap();
        model.advance_ns(1_000_000);
        model.write(ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
        model.advance_ns(SOFT_RESET_NS - 1);
        assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        assert_eq!(
            model.read(ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn aborts_heater_with_soft_reset_and_preserves_serial() {
        let mut model = Sht45Model::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model.write(ADDRESS, &[HEATER_LONG_COMMANDS[0]]).unwrap();
        model.advance_ns(1_000_000);
        assert_eq!(model.write(ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
        model.advance_ns(SOFT_RESET_NS - 1);
        assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        assert_eq!(
            model.read(ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn soft_reset_is_the_only_busy_write_exception() {
        let mut model = Sht45Model::new(0).with_measurement_ticks(0x1234, 0x5678);

        model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]).unwrap();
        model.advance_ns(1_000_000);
        assert_eq!(model.write(ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
        assert_eq!(
            model.write(ADDRESS, &[SOFT_RESET_COMMAND]),
            Err(Error::ResetWhileBusy)
        );
        assert_eq!(
            model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
    }

    #[test]
    fn rejects_non_reset_writes_while_heater_is_busy() {
        let mut model = Sht45Model::new(0).with_measurement_ticks(0x1234, 0x5678);

        model.write(ADDRESS, &[HEATER_SHORT_COMMANDS[0]]).unwrap();
        assert_eq!(
            model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(
            model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::WriteWhileBusy)
        );
        assert_eq!(model.write(ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
    }

    #[test]
    fn busy_state_precedes_malformed_write_validation() {
        for pending_command in [MEASURE_HIGH_COMMAND, SOFT_RESET_COMMAND] {
            let mut model = Sht45Model::new(0).with_measurement_ticks(0x1234, 0x5678);
            model.write(ADDRESS, &[pending_command]).unwrap();

            for bytes in [&[][..], &[SERIAL_NUMBER_COMMAND, 0x00][..]] {
                assert_eq!(model.write(ADDRESS, bytes), Err(Error::WriteWhileBusy));
            }
        }
    }

    #[test]
    fn reset_accumulates_split_delay_and_works_when_idle() {
        let mut model = Sht45Model::new(0x1234_5678);
        let mut response = [0; 6];

        model.write(ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
        model.advance_ns(400_000);
        assert_eq!(model.read(ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(600_000);
        assert_eq!(
            model.read(ADDRESS, &mut response),
            Err(Error::ReadBeforeCommand)
        );

        model.write(ADDRESS, &[SERIAL_NUMBER_COMMAND]).unwrap();
        model.read(ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }

    #[test]
    fn requires_explicit_measurement_ticks() {
        let mut model = Sht45Model::new(0);

        assert_eq!(
            model.write(ADDRESS, &[MEASURE_HIGH_COMMAND]),
            Err(Error::MissingMeasurementTicks)
        );
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
