# Changelog

## Unreleased

### Added

- Initialized the bounded Sensirion SHT45 driver repository with an unpublished Incubating scaffold.
- Retained the source-backed SHT45 serial-number propositions used to unblock
  later driver and model work, including the command execution timing for the
  response read.
- Added the implementation-tested serial-number read through abstract async I2C,
  including the device-required command delay, mandatory CRC validation, and
  distinct no-acknowledge errors.
- Added an independent unpublished behavioral model for the idle SHT45 serial
  readout, with explicit OTP input, distinct command-frame errors, and model-only
  trace tests.
- Added an unpublished host-only conformance package that adapts the public
  driver I2C trace to the independent model, verifies the unequal-word
  `0x1234_5678` serial result and transmission order, and checks that a corrupted
  response produces the driver's CRC error.

### Known issues

- Model conformance covers only the serial-number read; all other operations are
  uncovered, and the serial-number read is not physically validated.
