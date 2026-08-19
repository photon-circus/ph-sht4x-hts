# Changelog

## Unreleased

### Added

- Initialized the bounded Sensirion SHT45 driver repository with an unpublished Incubating scaffold.
- Retained the source-backed SHT45 serial-number propositions used to unblock
  later driver and model work, including the command execution timing for the
  response read.
- Retained the source-backed SHT45 T/RH measurement propositions for command
  selection, maximum timing, integer conversion, and one-shot measurement
  data, without claiming the operation is implemented or model-conformant.
- Added the implementation-tested serial-number read through abstract async I2C,
  including the device-required command delay, mandatory CRC validation, and
  distinct no-acknowledge errors.
- Added implementation-tested high, medium, and low repeatability T/RH
  measurement with Table 5 maximum delays, mandatory CRC validation, and
  uncropped integer millidegree and milli-%RH conversion.
- Added an independent unpublished behavioral model for SHT45 serial-number and
  one-shot T/RH readouts, with explicit OTP and conversion-tick inputs, maximum
  busy frontiers, one-shot consumption, distinct out-of-fidelity and transaction
  errors, and model-only trace tests.
- Added an unpublished host-only conformance package that adapts the public
  driver I2C trace to the independent model, verifies the unequal-word
  `0x1234_5678` serial result and transmission order, and checks that a corrupted
  response produces the driver's CRC error.
- Added host-only model conformance for high, medium, and low T/RH measurement,
  including shared relative-delay advancement, injected `0xBEEF` ticks,
  independently asserted command and maximum-delay mappings, adapter CRC
  corruption, and a busy-model check for a no-op delay.
- Retained the source-backed SHT45 soft-reset command, 1 ms idle-time bound, and
  measurement-abort propositions from Datasheet D1 Version 7.3, without claiming
  reset implementation or model conformance.
- Added model-only soft-reset behavior that aborts an in-flight measurement,
  preserves the explicit OTP serial, and returns to idle after the 1 ms reset
  busy interval while preserving busy-write error precedence, without claiming
  driver conformance or physical evidence.
- Retained source-backed SHT45 heater-pulse command, completion-bound, and
  sequencing propositions for bounded driver and model work without claiming
  driver implementation, conformance, or physical evidence.
- Added model-only behavior for all six SHT45 heater-pulse commands, including
  explicit conversion ticks, exact long and short busy frontiers, one-shot CRC
  frame consumption, soft-reset abort, and busy-write rejection, without
  claiming driver conformance or physical evidence.
- Added the implementation-tested public soft-reset operation, which writes
  `0x94`, waits 1 ms, performs no response read, and maps write NACK and bus
  errors without claiming model conformance or physical evidence.
- Added host-only public driver/model conformance coverage for soft-reset abort
  of an in-flight measurement, the routed 1 ms delay, and serial recovery;
  this does not establish physical reset timing or hardware evidence.
- Added the implementation-tested public heater-pulse operation for all six
  power/duration combinations, with complete long/short waits, mandatory CRC
  validation, integer T/RH readout, and distinct NACK/bus errors. Heater model
  conformance, application duty-cycle policy, and physical evidence remain
  unclaimed.
- Added host-only public driver/model conformance for all six heater pulses,
  including routed long/short delays, injected integer T/RH results, CRC
  corruption discrimination, a no-op-delay busy check, and public soft-reset
  abort with serial recovery. This does not establish heater physics,
  application duty-cycle policy, or physical evidence.

### Changed

- The local gate now compiles the driver for the `thumbv7em-none-eabihf` and
  `thumbv6m-none-eabi` bare-metal targets, reporting a distinct skip when a
  target is not installed. A `no_std` driver was previously only ever compiled
  for the host, which establishes nothing about the targets it exists to serve.
- The local gate now constructs the driver's package archive rather than only
  listing its contents, so cargo's verification build runs against the unpacked
  tree. `cargo package --list` exits successfully for a package whose source
  file has been excluded; construction does not. Over a dirty working tree the
  package checks cover that tree and print a notice saying so, so the gate stays
  runnable over uncommitted work without silently changing what it verified. An
  unreadable repository status is treated the same as a dirty tree, since cargo
  inspects the repository without the git CLI and would otherwise abort a gate
  that could not check.
- The declared version and publication lock now cover all three manifests
  instead of the driver alone, and are read from each `[package]` table rather
  than through `cargo pkgid`, which resolves through `Cargo.lock` and cannot see
  a manifest that has drifted from it.
- Every cargo invocation in the gate now uses `--locked`, making the committed
  `Cargo.lock` the resolved dependency set for verification.

### Documentation

- Recorded the verified bare-metal targets and what the local gate establishes.

### Known issues

- Model conformance covers every current public device operation; no operation
  is physically validated.
