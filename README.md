# ph-sht4x-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT4x humidity and temperature sensors — SHT40, SHT41, SHT43, and SHT45 — over abstract I2C.

[![Lifecycle: incubating](https://img.shields.io/badge/lifecycle-incubating-orange.svg)](https://github.com/photon-circus/.github/blob/main/docs/PERIPHERAL_DRIVER_PROFILE.md)
[![MSRV](https://img.shields.io/badge/MSRV-1.98.0-blue.svg)](Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> [!WARNING]
> **Lifecycle:** Incubating — the responsibility is bounded and intended to become a supported driver. Compatibility follows the documented version and release policy, not lifecycle alone.
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, soft-reset abort/recovery, and every documented address, against the independent model. That model implements behavior the datasheet states for the SHT4x without part qualification, recorded as `SHT4X-FAMILY-SCOPE-001`, plus a declared 10 ms serial guard adopted from Sensirion's current reference driver where the datasheet gives no duration; the guard is not a physical busy claim. **No check has been executed against any physical part**, so coverage across the family rests on that documentary basis rather than on execution.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or ph-hil-qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Packages in this workspace

| Package | Role | Distribution |
| --- | --- | --- |
| [`ph-sht4x-hts`](crates/sht4x/README.md) | Primary driver | Unpublished; `publish = false` |
| [`ph-sht4x-hts-model`](crates/sht4x-model/README.md) | Host-only independent behavioral model; not a user dependency | Unpublished; `publish = false` |
| [`ph-sht4x-hts-conformance`](crates/sht4x-conformance/README.md) | Host-only public-driver/model comparison; not a user dependency | Unpublished; `publish = false` |

## Responsibility and boundaries

This repository owns truthful supported SHT4x operations on one device through an abstract async I2C bus.

It does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application or duty-cycle policy; retrieval or application of the SHT43's ISO/IEC 17025 three-point calibration data; model conformance beyond the explicitly named host-only serial-number, T/RH, heater-pulse, and soft-reset checks; or physical qualification.

The supported set is the SHT40, SHT41, SHT43, and SHT45 at any documented address, resting on what the datasheet declares rather than on any part having been exercised. No operation has been executed against a physical device of any model. See [the contract](docs/CONTRACT.md).

## Quick start

See the [driver package README](crates/sht4x/README.md).

## Supported scope

- Device/family: Sensirion SHT4x — SHT40, SHT41, SHT43, SHT45
- Addressing: `0x44`, `0x45`, or `0x46`, chosen by the caller from position 7 of
  the part number. The address is not a function of the sensor model, so it is
  not inferred and the bus is never scanned.
- Transport: abstract async I2C and delay resources
- Rust: `1.98.0` on the pinned `1.98.0` toolchain
- Runtime posture: `no_std`, no allocation, and no unsafe code
- Verified targets: `thumbv6m-none-eabi` (Cortex-M0, no atomics),
  `thumbv7m-none-eabi` (Cortex-M3, atomics, soft-float),
  `thumbv7em-none-eabihf` (Cortex-M4F, atomics, hard-float),
  `thumbv8m.main-none-eabihf` (Cortex-M33, ARMv8-M), and
  `riscv32imac-unknown-none-elf` (RISC-V with atomics), compiled `--release` by
  the local gate. The crate is target-agnostic above its abstract `embedded-hal-async`
  resources; these five are the compilation evidence that exists, not a
  statement that other targets are unsupported. Host compilation alone
  establishes nothing about any of them.
- Supported operations: implementation-tested serial-number read, one-shot T/RH
  measurement at high, medium, or low repeatability, all six long/short heater
  pulses, and soft reset over abstract async I2C with the documented
  measurement/heater/reset waits and the conservative serial reference wait

## Evidence and limitations

Host compilation, linting, tests, coverage summaries, and package inspection
establish only their named software properties. They do not prove silicon
behavior, physical timing, heater physics, or hardware support.

This repository ships no physical-device evidence. The strongest evidence it
does ship is host-only model conformance of the named public operations. How
much of the production driver and model those checks actually executed is the
local gate's **model-conformance coverage**, printed in the `ci summary` and
written to `target/coverage/summary.txt`. Unit-test coverage of the same files
measures the implementation-tested layer; quoting it in place of conformance
coverage overstates what the public comparison established. Neither figure is
kept in this README: both are measurements of a run, not device propositions,
not thresholds, and not hardware claims.

See [the repository contract](docs/CONTRACT.md) for the current responsibility,
invariants, evidence posture, and source handling. The independent model's
fidelity declaration lives in [its README](crates/sht4x-model/README.md); that
model-only evidence does not establish driver conformance. What the host-only
comparison establishes, and what its execution coverage means, lives in
[the conformance package README](crates/sht4x-conformance/README.md).

## Documentation

- [`docs/CONTRACT.md`](docs/CONTRACT.md) — canonical device propositions, provenance, and evidence state
- [`CHANGELOG.md`](CHANGELOG.md) — caller-visible changes
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to propose, implement, verify, and submit a change
- [`SECURITY.md`](SECURITY.md) — private reporting and the driver's security surface
- [`RELEASING.md`](RELEASING.md) — version rules and publication steps
- [`AGENTS.md`](AGENTS.md) — agent routing, load-bearing traps, and protected actions
- [`crates/sht4x/README.md`](crates/sht4x/README.md) — packaged driver evaluation and usage
- [`crates/sht4x-model/README.md`](crates/sht4x-model/README.md) — one maintained model fidelity declaration
- [`crates/sht4x-conformance/README.md`](crates/sht4x-conformance/README.md) — what host-only driver/model comparison establishes, and what its execution coverage means

## Verification

The pinned toolchain installs its required components and all five verified
targets. Install the pinned Cargo tools with
`cargo install cargo-llvm-cov --version 0.8.7 --locked` and
`cargo install cargo-deny --version 0.20.2 --locked`, then run
`cargo xtask ci`. This complete local gate is authoritative. The bounded hosted
`ci` workflow runs the same gate for contributor feedback only after the
repository becomes public.

It checks formatting, the declared version and publication lock across all three
lifecycle-controlled manifests, lints with warnings denied, tests in both the
dev and release profiles, host code coverage, release compilation for the
verified bare-metal targets, documentation, and construction and inspection of
the driver's package archive. Every dependency-resolving cargo invocation uses
`--locked`, so the committed `Cargo.lock` is the resolved dependency set rather
than whatever resolves on the day.

The gate writes machine-readable summaries to `target/coverage/unit.json` and
`target/coverage/conformance.json`, and a human-readable
`target/coverage/summary.txt`. The first JSON measures driver and independent-
model code exercised by their unit tests; the second measures those production
implementations when the host-only conformance suite runs and is the
completeness figure to cite for host-only evidence. Instrumented build artifacts
are cleared before each layer so consecutive gate runs cannot merge their
denominators. A missing coverage tool, failed coverage test, or empty report
fails the gate. Coverage percentages are reported in the gate summary and in
`summary.txt`; they are not thresholds, and they create no device, silicon, or
physical-evidence claim. How to read them is under
[Evidence and limitations](#evidence-and-limitations).

When the gate finishes, it prints a summary of every check (`passed`, `skipped`,
`indeterminate`, or `failed`), the recorded coverage metrics, and an overall
result. Paste that summary into a pull request. A skipped or indeterminate check
makes the run incomplete and the command exits unsuccessfully.

The gate runs over uncommitted work. When the tree is dirty — or when its state
cannot be read, because cargo inspects the repository without the git CLI — the
package checks cover the working tree instead of the committed one and say so.
The release process runs from a clean checkout with git present, where no such
notice can appear.

## Contributing and releases

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contribution workflow
- [`SECURITY.md`](SECURITY.md) — private reporting
- [`CHANGELOG.md`](CHANGELOG.md) — caller-visible changes
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — expected behavior in project spaces
- [`RELEASING.md`](RELEASING.md) — version rules and publication steps. Publication is a maintainer decision and never a CI side effect; this repository is currently unpublished.

## License

MIT
