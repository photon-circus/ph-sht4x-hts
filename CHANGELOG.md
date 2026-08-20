# Changelog

Caller-visible changes to this workspace. Add new entries under `## Unreleased`.
Release assembly closes that heading; do not close it in a feature change.

Record a public guarantee here by what a caller can do, match, or must not
assume — not by the internal work that established it. Device propositions, gate
mechanics, and internal refactors belong in `docs/CONTRACT.md`, the verification
docs, and the commit history.

Versioning follows [`RELEASING.md`](RELEASING.md). The current published
version is `0.1.0-incubating.1`.

## Unreleased

No caller-visible changes yet.

## 0.1.0-incubating.1 - 2026-08-20

This initial prerelease adds an async `no_std` driver so embedded applications
can use the documented SHT4x serial-number, measurement, heater, and reset
operations through abstract I2C resources. It addresses the absence of a
bounded Photon Circus SHT4x driver while deliberately leaving board resources,
heater duty-cycle policy, SHT43 certificate retrieval, and physical
qualification to integration. That narrow boundary is the cost of keeping the
API and its claims truthful; host-model conformance remains software evidence
only.

Requires Rust `1.98.0` on the pinned toolchain. The local gate compiles the
driver for `thumbv6m-none-eabi`, `thumbv7m-none-eabi`,
`thumbv7em-none-eabihf`, `thumbv8m.main-none-eabihf`, and
`riscv32imac-unknown-none-elf`; those are the compilation evidence that exists,
not a claim that other targets are unsupported.

### Added

- **Driver.** `Sht4x::new` takes an `Address`, the bus, and the delay.
  `Address::{A, B, C}` are `0x44`, `0x45`, and `0x46`, read from part-number
  position 7. The address is not a function of the sensor model; the driver
  never infers it and never scans the bus. There is no sensor-model parameter:
  accuracy grade specifies a reading rather than a step in producing one. The
  driver package is `ph-sht4x-hts`. The `sht4x` in that name is the family
  identifier, not a claim beyond the disclosures below.

- **Operations.** Implementation-tested `read_serial_number`, `measure` at high,
  medium, or low repeatability, `heater_pulse` for all six power/duration
  combinations, and `reset`. Each owns its required wait: Table 5 maxima for
  measurement, inclusive 1.1/0.11 s heater-on maxima, 1 ms for reset, and a
  conservative 10 ms serial guard matching Sensirion's current reference
  driver where the datasheet states no serial duration. Every six-byte frame is
  CRC-8 checked on both words. NACK is `Error::NoAcknowledge`, distinct from
  other bus errors. The driver does not retry. Conversion is uncropped integer
  millidegrees Celsius and milli-%RH.

- **Heater.** `HeaterPower::{High, Medium, Low}` select the documented command
  bytes whose typical power at VDD = 3.3 V is 200 mW, 110 mW, and 20 mW. Those
  figures name the level; they are not delivered or guaranteed power, and they
  are not retained at any other supply. Duty cycle stays with the caller. The
  `Measurement` from `heater_pulse` is taken while the heater is still on.
  `measure` and `heater_pulse` share that type, so the type system does not
  record which reading you have. Heater physics and what either result implies
  about the surrounding air stay unclaimed. The caller must keep total
  heater-on time below 10% of sensor lifetime, operate the heater only below
  65 °C ambient, keep the sensor at or below 125 °C, and provide a supply that
  tolerates up to approximately 75 mA at the highest setting.

- **Errors.** `Error` is `#[non_exhaustive]`. It implements `Display` when the
  transport error implements `Display`, and `core::error::Error` when the
  transport error implements that trait; `source()` then exposes the transport
  error. `Error::Crc` names which response word failed.

- **SHT43 calibration.** The driver returns the serial number a certificate is
  filed under. Retrieving and applying the ISO/IEC 17025 three-point calibration
  is outside this repository. Omitting the certificate introduces no conversion
  error the driver could otherwise have corrected.

- **Model and conformance.** An unpublished independent model covers serial
  readout, one-shot T/RH, all six heater pulses, and soft-reset abort, at any of
  the three documented addresses. Host-only conformance compares the public
  driver to that model for every current public operation. The model implements
  behavior the datasheet states for the SHT4x without part qualification
  (`SHT4X-FAMILY-SCOPE-001`); the suite is one modeled behavior across three
  addresses, not four devices. A write that would discard an unconsumed response
  is the model's `WriteWithPendingResponse` limitation, not a guessed device
  outcome.

### Changed

- Serial-number reads now use the current Sensirion reference driver's 10 ms
  guard rather than an unsupported 10 µs value. Heater operations now wait the
  inclusive 1.1/0.11 s `tHeater` maxima instead of double-counting the trailing
  high-repeatability measurement.

- The local gate now prints a check-by-check summary when it finishes, and
  reports the unit and model-conformance coverage totals (and per-file line
  coverage) instead of leaving `target/coverage/*.json` unread. Percentages
  remain informational: they are not thresholds and are not physical evidence.
  A durable copy is written to `target/coverage/summary.txt`. Each coverage
  layer now starts from clean instrumentation artifacts so consecutive runs do
  not merge their denominators.

- The local gate now runs the test suite in both the dev and release profiles,
  and compiles the verified bare-metal targets as `--release`. Optimized codegen
  is no longer an untested path next to debug-only tests and debug-only
  cross-compilation.

- The local gate's verified compilation set is now five targets chosen for
  representational coverage rather than two Cortex-M samples:
  `thumbv6m-none-eabi` (no atomics), `thumbv7m-none-eabi` (atomics, soft-float),
  `thumbv7em-none-eabihf` (atomics, hard-float), `thumbv8m.main-none-eabihf`
  (ARMv8-M), and `riscv32imac-unknown-none-elf` (RISC-V). These remain
  compilation evidence, not a claim that other targets are unsupported.

### Known issues

- No public operation has been executed against a physical SHT40, SHT41, SHT43,
  or SHT45. Family coverage rests on `SHT4X-FAMILY-SCOPE-001`'s documentary
  basis rather than on execution. Model conformance is not physical evidence.
- Command futures are not cancellation-safe after their write may have been
  acknowledged. Dropping one can leave an operation in progress or a response
  unread; integration must re-establish a known idle transaction state before
  issuing another command.
