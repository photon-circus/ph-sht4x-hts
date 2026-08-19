# Agent notes — ph-sht45-hts

The adopted Photon Circus organization standards and this repository's recorded contracts are authoritative. Read `README.md` and `docs/CONTRACT.md` before changing behavior.

## Boundary and priorities

Keep the repository responsible only for truthful supported SHT45 operations on one device through an abstract async I2C bus. Concrete board resources, scheduling, workflow retry, cross-device coordination, and product recovery remain integration concerns.

Prioritize truthful supported behavior, explicit state/error semantics, and narrow evidence over API breadth.

## Coupled changes

- A changed device proposition updates its canonical evidence record, affected implementation/tests, and local consequences in public documentation.
- A changed public guarantee updates tests and `CHANGELOG.md`.
- A changed lifecycle, distribution, model, or physical-evidence fact updates both root and package status disclosures.

## Verification and protected actions

Run `./scripts/ci.sh`; report skipped checks as skipped. Do not publish, create a release, change repository visibility or lifecycle, claim model/physical evidence, or add speculative HIL/model/application scaffolding without explicit maintainer direction and the required evidence.
