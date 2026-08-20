# ph-sht4x-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT4x humidity and temperature sensors — SHT40, SHT41, SHT43, and SHT45 — over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Supported devices:** SHT40, SHT41, SHT43, and SHT45, at any of the three documented I2C addresses. The address comes from the part number, not the sensor model.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, soft-reset abort/recovery, and every documented address, against the independent model. That model implements behavior the datasheet states for the SHT4x without part qualification, recorded as `SHT4X-FAMILY-SCOPE-001`; **no check has been executed against any physical part**, so coverage across the family rests on that documentary basis rather than on execution.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Responsibility and boundaries

This repository owns truthful supported SHT4x operations on one device through an abstract async I2C bus.

It does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application or duty-cycle policy; retrieval or application of the SHT43's ISO/IEC 17025 three-point calibration data; model conformance beyond the explicitly named host-only serial-number, T/RH, heater-pulse, and soft-reset checks; or physical qualification.

The supported set is the SHT40, SHT41, SHT43, and SHT45 at any documented address, resting on what the datasheet declares rather than on any part having been exercised. No operation has been executed against a physical device of any model. See [the contract](docs/CONTRACT.md).

## Supported scope

- Device/family: Sensirion SHT4x — SHT40, SHT41, SHT43, SHT45
- Addressing: `0x44`, `0x45`, or `0x46`, chosen by the caller from position 7 of
  the part number. The address is not a function of the sensor model, so it is
  not inferred and the bus is never scanned.
- Transport: abstract async I2C and delay resources
- Rust: `1.92.0` on the pinned `1.92.0` toolchain
- Runtime posture: `no_std`, no allocation, and no unsafe code
- Verified targets: `thumbv7em-none-eabihf` and `thumbv6m-none-eabi`, compiled
  by the local gate. The crate is target-agnostic above its abstract
  `embedded-hal-async` resources; these two are the compilation evidence that
  exists, not a statement that other targets are unsupported. Host compilation
  alone establishes nothing about either.
- Supported operations: implementation-tested serial-number read, one-shot T/RH
  measurement at high, medium, or low repeatability, all six long/short heater
  pulses, and soft reset over abstract async I2C with the device-required
  command delays

## Quick start

The crate provides implementation-tested serial-number and one-shot T/RH reads,
all six heater pulses, and soft reset for one SHT45-AD1B device. The driver owns
each pulse's complete wait and read sequence; the caller owns application-level
heater cadence and duty-cycle policy. See [the package README](crates/sht4x/README.md).

A heater pulse converts while the heater is still on, so the reading it returns
describes the heated sensor rather than one taken with the heater off. How the
two differ is heater physics and stays unclaimed here, as does what either
implies about the surrounding air. The two share the `Measurement` type and are
not substitutes for one another.

## Evidence and limitations

Model conformance covers the host-only serial-number, T/RH measure, all six heater pulses, and soft-reset abort/recovery checks described below; no physical-device claim is made. Host compilation, linting, tests, and package inspection establish only their named software properties.

See [the repository contract](docs/CONTRACT.md) for the current responsibility, invariants, evidence posture, and source handling.

The independent model package is documented in [its README](crates/sht4x-model/README.md). It covers selected serial-number, one-shot T/RH, heater-pulse, and soft-reset-abort behavior; this model-only evidence does not establish driver conformance or silicon behavior.

## Verification

Run `cargo xtask ci`. This local gate is authoritative; no hosted workflow is assumed.

It checks formatting, the declared version and publication lock across all three
lifecycle-controlled manifests, lints with warnings denied, tests, compilation
for the verified bare-metal targets, documentation, and construction and
inspection of the driver's package archive. Every dependency-resolving cargo
invocation uses `--locked`, so the committed `Cargo.lock` is the resolved
dependency set rather than whatever resolves on the day.

A check that cannot run says so, and distinguishes why: `skipped` when a
prerequisite is absent, such as an uninstalled target or cargo-deny, and
`indeterminate` when a prerequisite exists but could not be interrogated, such
as a `rustup target list` that fails. Neither is a passed check.

The gate runs over uncommitted work. When the tree is dirty — or when its state
cannot be read, because cargo inspects the repository without the git CLI — the
package checks cover the working tree instead of the committed one and say so.
The release process runs from a clean checkout with git present, where no such
notice can appear.

## Contributing, security, and releases

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — the responsibility boundary, the
  proposition rule every device fact goes through, the evidence states and what
  may not be collapsed, and what a bug report needs.
- [`SECURITY.md`](SECURITY.md) — private reporting, and what is and is not in
  scope for a `no_std`, allocation-free, unsafe-free driver.
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — expected behavior in project
  spaces.
- [`RELEASING.md`](RELEASING.md) — the version rules, the ordinary-release gate,
  and the exact steps. Publication is a maintainer decision and never a CI side
  effect; this repository is currently unpublished.
- [`AGENTS.md`](AGENTS.md) — additional constraints for coding agents.

## License

MIT
