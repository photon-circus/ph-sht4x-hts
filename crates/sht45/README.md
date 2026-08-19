# ph-sht45-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT45 humidity and temperature sensor over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, and soft-reset abort/recovery.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Availability

This package is not available from crates.io.

## Current state

The package supports implementation-tested serial-number and one-shot T/RH
reads, all six heater pulses, and soft reset for one SHT45-AD1B device.

Each device fact below is retained once, with exact provenance, as an
identified proposition in the
[repository contract](https://github.com/photon-circus/ph-sht45-hts/blob/main/docs/CONTRACT.md). This README cites those identifiers and
states only what the driver does with them; it does not keep a second copy of
the propositions or of their source coordinates. The contract is not part of
the packaged crate, so that link is absolute and resolves from crates.io and
docs.rs as well as from the repository.

| Operation | Propositions consumed | What the driver does |
| --- | --- | --- |
| Addressing | `SHT45-I2C-ADDR-001` | Addresses one SHT45-AD1B at 7-bit `0x44`. There is no address parameter. |
| `read_serial_number` | `SHT45-SN-CMD-001`, `SHT45-CRC-001` | Writes the command, waits its execution time, then reads six bytes in a separate transaction and returns the transmission-order `u32`. |
| `measure` | `SHT45-MEAS-CMD-001`, `SHT45-MEAS-TIME-001`, `SHT45-MEAS-CONV-001` | Writes the repeatability-selected command, waits that repeatability's Table 5 **maximum** rather than its typical, then reads and converts six bytes. |
| `heater_pulse` | `SHT45-HEAT-CMD-001`, `SHT45-HEAT-TIME-001`, `SHT45-HEAT-SEQ-001` | Writes the power- and duration-selected command, waits the complete pulse-plus-measurement bound, then reads one six-byte frame. |
| `reset` | `SHT45-RST-CMD-001`, `SHT45-RST-TIME-001` | Writes the soft-reset command, waits the idle-time bound, and performs no response read. |
| Every read frame | `SHT45-CRC-001` | Validates both CRC-8 bytes. A mismatch is an error and the driver does not retry. |

Two consequences are worth stating plainly because they shape how the driver is
called. Each operation blocks for its device-required wait, so a long heater
pulse holds the future for over a second. Conversion results are uncropped
integer millidegrees Celsius and milli-%RH, so a reading outside 0–100 %RH is
reported rather than clamped.

## Platform support

The crate is `no_std`, uses abstract `embedded-hal-async` I2C and delay resources, allocates no memory, and forbids unsafe code. The caller owns those resources, power-up timing, scheduling, retries, recovery policy, and heater cadence or duty-cycle policy. The driver owns the serial, measurement, heater-pulse, and soft-reset commands' execution waits. Model conformance covers the serial-number read, T/RH measurement at high, medium, and low repeatability, all six heater pulses, and soft-reset abort/recovery; no physical-device claim is made.

The independent model is a separate package; its existence does not establish conformance for this driver.

## License

MIT
