# ph-sht45-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT45 humidity and temperature sensor over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Model conformance:** Serial-number read only. An unpublished host-only conformance check compares the public driver result with an independent model's idle serial-number trace; other operations remain uncovered.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Responsibility and boundaries

This repository owns truthful supported SHT45 operations on one device through an abstract async I2C bus.

It does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application policy; SHT40/41/43 family claims; model conformance beyond the explicitly named host-only serial-number check; or physical qualification.

## Supported scope

- Device/family: Sensirion SHT45
- Transport: abstract async I2C and delay resources
- Rust: `1.92.0` on the pinned `1.92.0` toolchain
- Runtime posture: `no_std`, no allocation, and no unsafe code
- Supported operation: implementation-tested serial-number read over abstract
  async I2C with the device-required command delay

## Quick start

The crate provides an implementation-tested serial-number read for one SHT45-AD1B device. See [the package README](crates/sht45/README.md).

## Evidence and limitations

Model conformance is limited to the host-only serial-number check described below; no physical-device claim is made. Host compilation, linting, tests, and package inspection establish only their named software properties.

See [the repository contract](docs/CONTRACT.md) for the current responsibility, invariants, evidence posture, and source handling.

The independent model package is documented in [its README](crates/sht45-model/README.md). Its existence is model-only evidence and does not establish driver conformance or silicon behavior.

## Verification

Run `./scripts/ci.sh`. This local gate is authoritative; no hosted workflow is assumed.

## License

MIT
