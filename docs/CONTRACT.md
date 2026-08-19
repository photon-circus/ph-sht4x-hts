# Sensirion SHT45 driver contract

## Responsibility

The repository owns truthful supported SHT45 operations on one device through an abstract async I2C bus.

## Non-goals

The repository does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application policy; SHT40/41/43 family claims; model conformance; or physical qualification.

## Initial invariants

- Public behavior must not claim a device capability without retained, exact provenance.
- Single-device operations own the device-required sequencing, legal encodings, units, timing, state authority, and recovery semantics they need to be truthful.
- Concrete resources and application policy stay outside the driver.

Refine this list when implementation creates review-blocking invariants; do not inventory hypothetical behavior.

## Sources and device propositions

No source-backed device proposition is retained yet. Add the smallest permanent proposition and exact provenance only when current implementation, model, conformance, physical evidence, or bug disposition consumes it. Missing evidence remains undefined and creates no claim or validation assignment.

Vendor source files remain untracked unless redistribution is explicitly permitted.

## Evidence posture

- Implementation-tested: inert scaffold and repository checks only.
- Model-conformant: none.
- Physically observed: none.
- Qualified: none.

## Definition of stable

The repository can be considered stable only after its supported operations, limitations, failure/recovery behavior, target scope, and proportionate evidence are bounded and reproducibly verified. Lifecycle promotion and publication remain separate maintainer decisions.
