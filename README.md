# ph-sht45-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT45 humidity and temperature sensor over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, and soft-reset abort/recovery.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Responsibility and boundaries

This repository owns truthful supported SHT45 operations on one device through an abstract async I2C bus.

It does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application or duty-cycle policy; SHT40/41/43 family claims; model conformance beyond the explicitly named host-only serial-number, T/RH, heater-pulse, and soft-reset checks; or physical qualification.

## Supported scope

- Device/family: Sensirion SHT45
- Transport: abstract async I2C and delay resources
- Rust: `1.92.0` on the pinned `1.92.0` toolchain
- Runtime posture: `no_std`, no allocation, and no unsafe code
- Supported operations: implementation-tested serial-number read, one-shot T/RH
  measurement at high, medium, or low repeatability, all six long/short heater
  pulses, and soft reset over abstract async I2C with the device-required
  command delays

## Quick start

The crate provides implementation-tested serial-number and one-shot T/RH reads,
all six heater pulses, and soft reset for one SHT45-AD1B device. The driver owns
each pulse's complete wait and read sequence; the caller owns application-level
heater cadence and duty-cycle policy. See [the package README](crates/sht45/README.md).

A heater pulse converts while the heater is still on, so the reading it returns
describes the heated sensor rather than the surrounding air. How the two differ
is heater physics and stays unclaimed here. It shares the `Measurement` type
with the ambient read and is not a substitute for it.

## Evidence and limitations

Model conformance covers the host-only serial-number, T/RH measure, all six heater pulses, and soft-reset abort/recovery checks described below; no physical-device claim is made. Host compilation, linting, tests, and package inspection establish only their named software properties.

See [the repository contract](docs/CONTRACT.md) for the current responsibility, invariants, evidence posture, and source handling.

The independent model package is documented in [its README](crates/sht45-model/README.md). It covers selected serial-number, one-shot T/RH, heater-pulse, and soft-reset-abort behavior; this model-only evidence does not establish driver conformance or silicon behavior.

## Verification

Run `./scripts/ci.sh`. This local gate is authoritative; no hosted workflow is assumed.

## License

MIT
