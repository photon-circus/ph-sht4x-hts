# Contributing to ph-sht4x-hts

This guide controls over the
[organization-wide fallback](https://github.com/photon-circus/.github/blob/main/CONTRIBUTING.md)
for this repository. Human path: [`README.md`](README.md) → this file →
[`docs/CONTRACT.md`](docs/CONTRACT.md). The contract is the authority on what
this repository owns and what it refuses to own.

Participation is governed by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
Security reports follow [`SECURITY.md`](SECURITY.md) and never go in a public
issue.

## Scope of accepted contributions

This repository owns truthful supported SHT4x operations on **one** device
through an abstract async I2C bus. The decision test is:

> Behavior required to make one supported device operation truthful belongs to
> the driver. Concrete resources, scheduling, composition, and application
> response belong to integration.

The supported set is the SHT40, SHT41, SHT43, and SHT45 at any documented
address. That set rests on what the datasheet declares, not on any part having
been exercised.

Board topology, concrete bus/GPIO/power resources, sampling cadence, retry and
escalation policy, heater duty-cycle policy, retrieval or application of the
SHT43's ISO/IEC 17025 three-point calibration data, and physical qualification
are outside the boundary. A change that adds any of them belongs in an
integration layer, not here, however useful it is.

## Toolchain and setup

The workspace pins Rust `1.98.0` in `rust-toolchain.toml`. Install the coverage
frontend once, then format and run the canonical gate:

```sh
cargo install cargo-llvm-cov --locked # once, if it is not installed
cargo xtask fmt
cargo xtask ci
```

`cargo xtask fmt` rewrites with rustfmt. The gate checks the same invocation
and does not rewrite files.

## Opening a change

Use the organization issue and pull-request templates. Keep one candidate on
one branch, independently acceptable or rejectable. Name what is outside the
change. A pull request that quietly widens the repository's responsibility is
rejected on scope even when the code is good.

For a device-facing bug report, include:

- Device and board revision, and MCU and target triple.
- Bus mode and speed, and the concrete `embedded-hal-async` implementation.
- Enabled features, toolchain version, and package version or commit.
- A minimal reproduction, expected behavior, and observed behavior.
- Logs, bus traces, or register observations where available.
- **Whether the evidence came from physical hardware, a simulation, or a mock.**

That last line decides how the report can be used. A mock or simulated
observation never silently becomes a hardware claim. "Not reproduced" is a
sufficient status for a retained report and creates no reproduction work.

## Coupled changes

The compiler cannot enforce these. A more specific file does not silently
override the owner of another subject.

| Kind of change | Also update |
| --- | --- |
| Device or documentary proposition | Canonical record in `docs/CONTRACT.md`, affected implementation and tests, and the local consequence in public documentation. Identifiers are permanent: never reuse or redefine one. Downstream surfaces cite the identifier; they do not copy the proposition. |
| Public guarantee | Tests and a caller-facing `CHANGELOG.md` entry under `## Unreleased`. |
| Lifecycle, distribution, model, or physical-evidence fact | Both the root and package status disclosures. Those two warning blocks must stay identical. |
| Driver behavior the model also describes | Both sides, each derived independently from the shared proposition. Editing one side merely to make the comparison pass creates a self-confirming system. |

The model is the oracle, not a second driver. It must not reuse the driver's
encoders, decoders, transaction builders, sequencing, or state machine.
Sharing the abstract transport trait and public value types is fine; sharing
the logic under test is not. Where both implement the same proposition, they
implement it separately — the CRC is deliberately derived two different ways
for this reason. The conformance package is the one place they legitimately
meet, and it meets them in a test rather than in a library.

Missing evidence stays missing. A proposition with nothing deciding it says so
plainly rather than being treated as approval, as proof of the opposite
behavior, or as pending work for someone else.

## Evidence sources

The repository reports four evidence states independently, per operation, and
they must not be collapsed:

| State | What establishes it |
| --- | --- |
| Implementation-tested | Unit and scripted-transport tests against the abstract I2C fake |
| Model-only | The independent model's own tests |
| Model-conformant | The public driver exercised against the independent model |
| Physically observed / qualified | Reviewed physical-device evidence — currently **none** |

Coverage percentages are not a fifth evidence state. They disclose how much
production code a named suite executed. Model-conformance coverage is the
completeness of host-only evidence in the absence of a physical run; unit-test
coverage describes the implementation-tested layer. Do not quote the latter in
place of the former. Do not freeze either figure in a README or badge.

Passing tests establish the layer that ran, and nothing above it. A green host
gate is not evidence of silicon behavior, physical timing, or heater physics.
Never describe host-model success as hardware support.

## Canonical validation

`cargo xtask ci` is authoritative; no hosted workflow is assumed. Paste the
printed `ci summary` — every check outcome and the recorded coverage metrics —
into the pull-request evidence table, not the commands you intended to run.
The model-conformance coverage lines are the disclosure of how complete that
host-only evidence is.

The gate runs over uncommitted work. When the tree is dirty, the package checks
cover the working tree rather than the committed one and print a notice saying
so. Only the release procedure needs a clean checkout.

The gate records separate unit and model-conformance coverage summaries under
`target/coverage` (`unit.json`, `conformance.json`, and `summary.txt`). These
percentages are informational software measurements, not acceptance thresholds
or evidence of physical-device behavior. They appear in the gate summary; they
are not used as pass/fail criteria.

Report skipped checks as skipped, and `indeterminate` checks as indeterminate.
Neither is a passed check.

## Pull-request checklist

- Evidence table filled with commands actually run.
- Changelog entry under `## Unreleased` when a caller-visible guarantee changes.
- Out-of-scope items named; no quiet widening of repository responsibility.
- Coupled surfaces from the table above updated in the same change.

## Changelog, compatibility, and release impact

Changelog entries are caller-facing: record what a caller can do, match, or
must not assume. Do not log proposition retention, gate mechanics, or internal
refactors except through that consequence. Do not close `Unreleased`; release
assembly does that.

Do not publish, tag, create a release, change repository visibility, change
lifecycle, claim model or physical evidence, or add speculative
hardware-in-the-loop scaffolding without explicit maintainer direction and the
required evidence. Approval of a pull request does not by itself authorize any
of them. See [`RELEASING.md`](RELEASING.md).
