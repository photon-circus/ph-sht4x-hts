# Changelog

## Unreleased

### Added

- Initialized the bounded Sensirion SHT45 driver repository with an unpublished Incubating scaffold.
- Retained the source-backed SHT45 serial-number propositions used to unblock
  later driver and model work, including the command execution timing for the
  response read.
- Added the implementation-tested serial-number read through abstract async I2C,
  including mandatory CRC validation and distinct no-acknowledge errors.

### Known issues

- The serial-number read is not physically validated and has no model-conformance
  evidence.
