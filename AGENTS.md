# Agent notes — ph-sht4x-hts

The supported set is the SHT40, SHT41, SHT43, and SHT45. It rests on
`SHT4X-FAMILY-SCOPE-001` — a statement about what the datasheet declares, not
about what any part does. Nothing here has been executed against a physical
device of any model, and the conformance suite exercises one modeled behavior
across three addresses, not four devices. Do not let that become "the driver
supports four sensors" in any surface that a caller reads.

The adopted Photon Circus organization standards and this repository's recorded contracts are authoritative. Read `README.md` and `docs/CONTRACT.md` before changing behavior.

## Boundary and priorities

Keep the repository responsible only for truthful supported SHT4x operations on one device through an abstract async I2C bus. Concrete board resources, scheduling, workflow retry, cross-device coordination, and product recovery remain integration concerns.

Prioritize truthful supported behavior, explicit state/error semantics, and narrow evidence over API breadth.

## Coupled changes

- A changed device proposition updates its canonical evidence record, affected implementation/tests, and local consequences in public documentation.
- A changed public guarantee updates tests and `CHANGELOG.md`.
- A changed lifecycle, distribution, model, or physical-evidence fact updates both root and package status disclosures.

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
- **A power level is not a duration, and the power half is unverified.**
  `SHT45-HEAT-CMD-001` groups the six heater bytes by duration;
  `SHT45-HEAT-PWR-001` binds each to a power. Both are needed to justify the
  `HeaterPower`/`HeaterDuration` mapping, and the tests cannot discriminate a
  rotated power mapping on their own. `SHT45-HEAT-PWR-001` is recorded
  **unverified** — its figures have not been read against the pinned datasheet —
  so no public surface may describe a `HeaterPower` variant as a confirmed
  wattage until a maintainer performs that read.
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

## Verification and protected actions

Run `./scripts/ci.sh`; report skipped checks as skipped. Do not publish, create a release, change repository visibility or lifecycle, claim model/physical evidence, or add speculative HIL/model/application scaffolding without explicit maintainer direction and the required evidence.
