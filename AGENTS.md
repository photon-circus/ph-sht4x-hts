# Agent notes — ph-sht4x-hts

Adopted Photon Circus organization standards and this repository's recorded
contracts are authoritative. [`CONTRIBUTING.md`](CONTRIBUTING.md) owns the
shared contribution workflow. Agent path: this file → `CONTRIBUTING.md` →
[`docs/CONTRACT.md`](docs/CONTRACT.md). Resolve inconsistent instructions with
a maintainer before changing the affected behavior.

## Boundary and priorities

Keep the repository responsible only for truthful supported SHT4x operations on
one device through an abstract async I2C bus. Concrete board resources,
scheduling, workflow retry, cross-device coordination, and product recovery
remain integration concerns.

Prioritize truthful supported behavior, explicit state/error semantics, and
narrow evidence over API breadth.

The supported set is the SHT40, SHT41, SHT43, and SHT45. It rests on
`SHT4X-FAMILY-SCOPE-001` — a statement about what the datasheet declares, not
about what any part does. Nothing here has been executed against a physical
device of any model, and the conformance suite exercises one modeled behavior
across three addresses, not four devices. Do not let that become "the driver
supports four sensors" in any surface that a caller reads.

## Canonical sources

- [`docs/CONTRACT.md`](docs/CONTRACT.md) owns device propositions, provenance, and evidence state.
- Package READMEs and rustdoc own the local API or product consequence; they cite identifiers and do not copy propositions.
- [`CHANGELOG.md`](CHANGELOG.md) owns caller-visible guarantees.
- Root and packaged driver READMEs share one four-field status block; a lifecycle, distribution, model, or physical-evidence change updates both.

## Load-bearing invariants and traps

Things that are expensive to rediscover and cheap to get subtly wrong.

- **Driver and model must be able to disagree.** The model is the oracle, not a
  second driver. Where both implement the same proposition they implement it
  separately: the CRC is deliberately a bit-at-a-time shift register in the
  driver and a nibble-table reduction in the model. Do not "simplify" them into
  one shape, and do not edit either side merely to make a comparison pass.
- **The heater waits are composite.** `HEATER_LONG_DURATION_US` is the pulse
  plus the trailing high-repeatability measurement. Dropping the `8_300` reads
  the frame before the device has it, and no unit test would obviously say why.
- **A power level is not a duration, and the wattages are typical.**
  `SHT45-HEAT-CMD-001` groups the six heater bytes by duration;
  `SHT45-HEAT-PWR-001` binds each to a power. Both are needed to justify the
  `HeaterPower`/`HeaterDuration` mapping, and the tests cannot discriminate a
  rotated power mapping on their own — power is not observable at the transport
  boundary. The 200/110/20 mW figures are Table 8's typical values at
  VDD = 3.3 V; no surface may present them as delivered or guaranteed power.
- **A no-op delay provider is a discriminator, not a shortcut.** Conformance
  tests route the driver's requested delay into model time. A test that uses
  `NoopDelay` is asserting that an insufficient wait fails; using it to avoid
  wiring up time silently disables the timing comparison.
- **Model limitations must not be dressed as device responses.** `Busy` is a
  documented NACK. Every other model error means the model cannot answer, and
  the adapter maps it to `ErrorKind::Other` so it never claims the device
  produced it.
- **Undeclared sequences are rejected, not resolved.** When the sources do not
  say what the device does, the model returns an explicit limitation. Do not
  pick a plausible outcome because the state machine can continue.

## Commands and claims

`cargo xtask ci` is the canonical local gate. It establishes named software
properties: formatting, lifecycle-lock, lints, tests in both the dev and
release profiles, host coverage summaries, release compilation of verified
targets, documentation, and package construction. It prints a final summary of
every check and the recorded coverage metrics. Report skipped and indeterminate
checks as such. None of those results is silicon behavior, physical timing, or
hardware support. When quoting coverage, use the model-conformance totals to
describe host-only evidence completeness. Unit-test totals describe the
implementation-tested layer. Do not freeze either figure in a README or badge.

A changed public guarantee updates tests and a caller-facing `CHANGELOG.md`
entry. Changelog entries record what a caller can do, match, or must not
assume, not the internal work that established it.

## Protected actions

Do not publish, create a release, change repository visibility or lifecycle,
claim model or physical evidence, or add speculative HIL, model, or application
scaffolding without explicit maintainer direction and the required evidence.
See [`RELEASING.md`](RELEASING.md).
