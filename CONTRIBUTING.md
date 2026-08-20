# Contributing to ph-sht4x-hts

This guide controls over the
[organization-wide fallback](https://github.com/photon-circus/.github/blob/main/CONTRIBUTING.md)
for this repository. Read `README.md`, [`docs/CONTRACT.md`](docs/CONTRACT.md),
and `AGENTS.md` before changing behavior; the contract is the authority on what
this repository owns and what it refuses to own.

## What belongs here

This repository owns truthful supported SHT45 operations on **one** device
through an abstract async I2C bus. The decision test is:

> Behavior required to make one supported device operation truthful belongs to
> the driver. Concrete resources, scheduling, composition, and application
> response belong to integration.

Board topology, concrete bus/GPIO/power resources, sampling cadence,
retry and escalation policy, heater duty-cycle policy, SHT40/41/43 family
support, and physical qualification are outside the boundary. A change that
adds any of them belongs in an integration layer, not here, however useful it
is.

## Every device fact needs a proposition

This is the rule most likely to send a pull request back.

A change that relies on a device or documentary fact cites the stable
identifier for that fact in [`docs/CONTRACT.md`](docs/CONTRACT.md). If the fact
is not yet retained, add the smallest permanent proposition and its exact
provenance in the same change. Identifiers are permanent: never reuse or
redefine one, and give a changed meaning a new identifier.

Do not restate a proposition elsewhere. Downstream surfaces — the package
READMEs, the model's fidelity declaration, rustdoc — cite the identifier and
state only their own local consequence. `docs/CONTRACT.md` is the single
canonical owner of the wording, the vendor coordinates, and the evidence state.

Missing evidence stays missing. A proposition with nothing deciding it says so
plainly rather than being treated as approval, as proof of the opposite
behavior, or as pending work for someone else.

## Do not upgrade a claim without the evidence

The repository reports four evidence states independently, per operation, and
they must not be collapsed:

| State | What establishes it |
| --- | --- |
| Implementation-tested | Unit and scripted-transport tests against the abstract I2C fake |
| Model-only | The independent model's own tests |
| Model-conformant | The public driver exercised against the independent model |
| Physically observed / qualified | Reviewed physical-device evidence — currently **none** |

Passing tests establish the layer that ran, and nothing above it. A green host
gate is not evidence of silicon behavior, physical timing, or heater physics.
Never describe host-model success as hardware support.

## Coupled changes the compiler cannot enforce

- A changed device proposition updates its canonical evidence record, the
  affected implementation and tests, and the local consequences in public
  documentation.
- A changed public guarantee updates tests and `CHANGELOG.md`.
- A changed lifecycle, distribution, model, or physical-evidence fact updates
  **both** the root and package status disclosures, and the repository
  description.
- A driver behavior change that the model also describes updates both, each
  derived independently from the shared proposition. Editing one side merely to
  make the comparison pass creates a self-confirming system.

## Keeping driver and model independent

The model is the oracle, not a second driver. It must not reuse the driver's
encoders, decoders, transaction builders, sequencing, or state machine.
Sharing the abstract transport trait and public value types is fine; sharing
the logic under test is not. Where both implement the same proposition, they
implement it separately — the CRC is deliberately derived two different ways
for this reason.

The conformance package depends on both. That is the one place they legitimately
meet, and it meets them in a test rather than in a library.

## Verification

Run the canonical gate:

```sh
./scripts/ci.sh
```

It is authoritative for this repository; no hosted workflow is assumed. It
requires a committed tree, because it constructs and verifies the driver's
package archive.

Report skipped checks as skipped. A skipped check is not a passed check.

## Pull requests

Use the organization pull-request template and fill in the evidence table with
the commands you actually ran, not the ones you intended to. Keep one candidate
on one branch, keep it independently acceptable or rejectable, and add
changelog entries beneath `## Unreleased`. Do not close `Unreleased`; release
assembly does that.

Name what is outside the change. A pull request that quietly widens the
repository's responsibility is rejected on scope even when the code is good.

## Reporting a bug

Open an issue using the organization templates. For anything device-facing,
include:

- Device and board revision, and MCU and target triple.
- Bus mode and speed, and the concrete `embedded-hal-async` implementation.
- Enabled features, toolchain version, and package version or commit.
- A minimal reproduction, expected behavior, and observed behavior.
- Logs, bus traces, or register observations where available.
- **Whether the evidence came from physical hardware, a simulation, or a mock.**

That last line decides how the report can be used. A mock or simulated
observation never silently becomes a hardware claim. "Not reproduced" is a
sufficient status for a retained report and creates no reproduction work.

## Protected actions

Do not publish, tag, create a release, change repository visibility, change
lifecycle, claim model or physical evidence, or add speculative
hardware-in-the-loop scaffolding without explicit maintainer direction and the
required evidence. Approval of a pull request does not by itself authorize any
of them. See [`RELEASING.md`](RELEASING.md).

## Conduct

Participation is governed by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
Security reports follow [`SECURITY.md`](SECURITY.md) and never go in a public
issue.
