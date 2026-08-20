# ph-sht4x-hts

Incubating async no_std Rust driver for the Sensirion SHT4x humidity and temperature sensors — SHT40, SHT41, SHT43, and SHT45 — over abstract I2C.

[![Lifecycle: incubating](https://img.shields.io/badge/lifecycle-incubating-orange.svg)](https://github.com/photon-circus/.github/blob/main/docs/PERIPHERAL_DRIVER_PROFILE.md)
[![MSRV](https://img.shields.io/badge/MSRV-1.98.0-blue.svg)](https://github.com/photon-circus/ph-sht4x-hts/blob/main/Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/photon-circus/ph-sht4x-hts/blob/main/crates/sht4x/LICENSE)

> [!WARNING]
> **Lifecycle:** Incubating — the responsibility is bounded and intended to become a supported driver. Compatibility follows the documented version and release policy, not lifecycle alone.
> **Distribution:** crates.io prerelease version `0.1.0-incubating.1`; publication is a manual maintainer action and the driver manifest allows only crates.io.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, soft-reset abort/recovery, and every documented address, against the independent model. That model implements behavior the datasheet states for the SHT4x without part qualification, recorded as `SHT4X-FAMILY-SCOPE-001`, plus a declared 10 ms serial guard adopted from Sensirion's current reference driver where the datasheet gives no duration; the guard is not a physical busy claim. **No check has been executed against any physical part**, so coverage across the family rests on that documentary basis rather than on execution.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or ph-hil-qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Availability

The release artifact is version `0.1.0-incubating.1` and is intended for
crates.io only:

```sh
cargo add ph-sht4x-hts@0.1.0-incubating.1
```

This prerelease retains the Incubating lifecycle and the evidence limitations
in the warning above.

## Usage

```rust,no_run
use ph_sht4x_hts::{Address, Repeatability, Sht4x};

async fn read_once<I2C, DELAY>(
    i2c: I2C,
    delay: DELAY,
) -> Result<(), ph_sht4x_hts::Error<I2C::Error>>
where
    I2C: embedded_hal_async::i2c::I2c,
    DELAY: embedded_hal_async::delay::DelayNs,
{
    let mut sensor = Sht4x::new(Address::B, i2c, delay);
    let _serial = sensor.read_serial_number().await?;
    let _measurement = sensor.measure(Repeatability::High).await?;
    Ok(())
}
```

The caller supplies abstract `embedded-hal-async` I2C and delay resources and
the 7-bit address from part-number position 7. The driver never infers the
address from the sensor model and never scans the bus.

## Semantics and limitations

The package supports implementation-tested serial-number and one-shot T/RH
reads, all six heater pulses, and soft reset for one SHT4x — an SHT40, SHT41,
SHT43, or SHT45 — at 7-bit I2C address `0x44`, `0x45`, or `0x46`.

Each device fact below is retained once, with exact provenance, as an identified
proposition in the [repository contract](https://github.com/photon-circus/ph-sht4x-hts/blob/main/docs/CONTRACT.md).
This README cites those identifiers and states only what the driver does with
them; it does not keep a second copy of the propositions or of their source
coordinates. The contract is not part of the packaged crate, so that link is
absolute and resolves from a package registry or from rendered documentation as
well as from the repository.

| Operation | Propositions consumed | What the driver does |
| --- | --- | --- |
| Addressing | `SHT4X-PART-NOM-001`, `SHT4X-I2C-ADDR-001` | Takes the address from the caller as `Address::A`, `Address::B`, or `Address::C` for `0x44`, `0x45`, or `0x46`. The caller reads it off position 7 of the part number they ordered; it is not a function of the sensor model, so the driver never infers it and never scans the bus. |
| Part coverage | `SHT4X-FAMILY-SCOPE-001`, `SHT4X-ACC-001` | Nothing branches on the sensor model. Accuracy grade is a specification of a reading rather than a step in producing one, so there is no grade-dependent processing and the driver makes no accuracy claim of its own. |
| `read_serial_number` | `SHT4X-SN-CMD-001`, `SHT4X-SN-WAIT-001`, `SHT45-CRC-001` | Writes the command, applies the conservative 10 ms wait used by Sensirion's current reference driver, then reads six bytes in a separate transaction and returns the transmission-order `u32`. The wait is not presented as a datasheet maximum. |
| `measure` | `SHT45-MEAS-CMD-001`, `SHT45-MEAS-TIME-001`, `SHT45-MEAS-CONV-001` | Writes the repeatability-selected command, waits that repeatability's Table 5 **maximum** rather than its typical, then reads and converts six bytes. |
| `heater_pulse` | `SHT45-HEAT-CMD-001`, `SHT45-HEAT-PWR-001`, `SHT4X-HEAT-TIME-001`, `SHT45-HEAT-SEQ-001`, `SHT4X-HEAT-USE-001` | Writes the power- and duration-selected command, waits the inclusive 1.1 s or 0.11 s heater-on maximum, then reads one six-byte frame. The caller owns the documented operating constraints. |
| `reset` | `SHT45-RST-CMD-001`, `SHT45-RST-TIME-001` | Writes the soft-reset command, waits the idle-time bound, and performs no response read. |
| Every read frame | `SHT45-CRC-001` | Validates both CRC-8 bytes. A mismatch is an error and the driver does not retry. |
| Device not ready | `SHT45-I2C-XFER-001` | The device NACKs a read header while a documented measurement or heater action is busy, so a premature read there surfaces as `Error::NoAcknowledge`. The datasheet defines no corresponding behavior during the serial reference interval. The driver does not retry. |

Which heater command carries which power level is `SHT45-HEAT-PWR-001`: 200 mW,
110 mW, or 20 mW, descending. Those are the datasheet's typical figures at
VDD = 3.3 V, not a delivered or guaranteed power, and they are qualified to that
one supply voltage.

Three consequences are worth stating plainly, because they shape how the driver
is called.

- Each operation holds its future across its complete requested wait, so a long
  heater pulse requests a 1.1-second delay (`SHT4X-HEAT-TIME-001`), in addition
  to transport and scheduler overhead.
- These command futures are not cancellation-safe after their write may have
  been acknowledged. Dropping one can leave an action in progress or a response
  unread. Do not issue another command until integration has re-established a
  known idle transaction state; soft reset is not claimed to discard an
  already-completed unread response.
- Conversion results are uncropped integer millidegrees Celsius and milli-%RH,
  so a reading outside 0–100 %RH is reported rather than clamped
  (`SHT45-MEAS-CONV-001`).
- An SHT43's three-point calibration certificate is out-of-band data retrieved
  over a network rather than a device operation, so this driver neither
  retrieves nor applies it (`SHT4X-SHT43-CAL-001`). It does supply the serial
  number the certificate is filed under. Omitting the certificate introduces no
  error the driver could otherwise have corrected: the certificate refines the
  stated accuracy of a reading, it does not change how the reading is converted.

A heater pulse converts while the heater is still on (`SHT45-HEAT-SEQ-001`). The
`Measurement` it returns therefore describes the heated sensor rather than the
surrounding air. How the two differ is heater physics, which this repository
does not retain, model, bound, or correct.

Under `SHT4X-HEAT-USE-001`, total heater-on time must remain below 10% of
sensor lifetime, the heater must only be operated below 65 °C ambient, sensor
temperature must remain at or below 125 °C, and the supply must tolerate up to
approximately 75 mA at the highest setting without resetting the sensor.
Specifications do not apply while heating. The driver exposes these limits but
does not enforce cadence, thermal policy, or supply design.

`heater_pulse` and `measure` share the `Measurement` type, so nothing in the
type system prevents one being used where the other is meant. Use `measure` for
a reading taken with the heater off. What either reading implies about the
surrounding air is a system-calibration question this repository does not
answer.

## Features

This crate has no optional features and no default-feature effects.

## Platform support

Requires Rust `1.98.0`. The crate is `no_std`, allocates no memory, and forbids
unsafe code (`#![forbid(unsafe_code)]` applies to this crate's source, not its
transitive graph). It uses abstract `embedded-hal-async` I2C and delay
resources. The caller owns those resources, power-up timing, scheduling,
retries, cancellation recovery, and heater cadence, duty-cycle, thermal, and
supply policy. The driver owns the serial reference wait and the measurement,
heater-pulse, and soft-reset commands' documented waits.

The local gate compiles the driver in the release profile for `thumbv6m-none-eabi` (Cortex-M0, no
atomics), `thumbv7m-none-eabi` (Cortex-M3, atomics, soft-float),
`thumbv7em-none-eabihf` (Cortex-M4F, atomics, hard-float),
`thumbv8m.main-none-eabihf` (Cortex-M33, ARMv8-M), and
`riscv32imac-unknown-none-elf` (RISC-V with atomics). The crate is
target-agnostic above its abstract resources; those five are the compilation
evidence that exists, not a statement that other targets are unsupported. Host
compilation alone establishes nothing about any of them.

The independent model is a separate unpublished package and is not a user
dependency of this crate. Its existence does not establish conformance for this
driver.

## Documentation and support

- Source repository: <https://github.com/photon-circus/ph-sht4x-hts>
- Issues: <https://github.com/photon-circus/ph-sht4x-hts/issues>
- Security reports: [SECURITY.md](https://github.com/photon-circus/ph-sht4x-hts/blob/main/SECURITY.md)
- Device propositions and evidence: [repository contract](https://github.com/photon-circus/ph-sht4x-hts/blob/main/docs/CONTRACT.md)

API documentation for this release is available from
[docs.rs](https://docs.rs/ph-sht4x-hts/0.1.0-incubating.1/ph_sht4x_hts/).

## License

MIT
