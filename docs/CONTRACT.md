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

Retained propositions are limited to facts consumed by the planned serial-number
read. They are supported by Sensirion Datasheet SHT4x, D1 Version 7.3 (June
2026), `HT_DS_Datasheet_SHT4x_V7.3.pdf`,
https://sensirion.com/media/documents/33FD6951/6A7C10A0/HT_DS_Datasheet_SHT4x_V7.3.pdf,
retrieved 2026-08-19, 1,049,911 bytes, SHA-256
`8db4a43f17149b76811cfb504caaeca4ef844ddc710cb9b45905c51c7ddfe3c2`.
Redistribution of the vendor PDF is not claimed; the PDF is not committed to
this repository. Older SHT4x PDF revisions are not co-authority.

- `SHT45-I2C-ADDR-001` — The SHT45-AD1B 7-bit I2C address is `0x44` (device
  overview product table, ordering rows SHT45-AD1B-R2/R3; quick-start
  pseudocode). Evidence state: supported.
  Local consequence: the driver and model address one SHT45-AD1B at 7-bit
  `0x44` only; there is no address parameter, and SHT40 `0x45`/`0x46` are
  unsupported here.
- `SHT45-SN-CMD-001` — The serial number is read with command byte `0x89` as
  two 16-bit words, each followed by an 8-bit CRC; the response length
  including CRC is 6 bytes, and the command duration is 0.01 ms (Table 8,
  section 4.7). Evidence state: supported.
  Local consequence: the driver issues `write([0x89])` followed by a separate
  6-byte `read`, waiting at least 0.01 ms between them; `0x89` is not the I2C
  read address byte.
- `SHT45-CRC-001` — For each 16-bit read word, CRC-8 uses polynomial `0x31`,
  initialization `0xFF`, no input/output reflection, and final XOR `0x00`;
  the example is `CRC(0xBEEF) = 0x92` (Table 7, section 4.4). Evidence state:
  supported.
  Local consequence: CRC validation is mandatory, a mismatch is an error, and
  the driver does not retry.
- `SHT45-I2C-XFER-001` — I2C transfers begin with START and end with STOP;
  the sensor does not support clock stretching, and a read header while the
  sensor is busy (for example, during measurement or heating) is NACKed
  (section 4.1, Figure 14; Table 8). Evidence state: supported.
  Local consequence: the model treats the OTP serial as an explicit initial
  input and as re-readable; section 4.1's measurement-register deletion does
  not apply to serial. The caller, not the driver, satisfies `tPU` (Table 5).

Add the smallest permanent proposition and exact provenance only when current
implementation, model, conformance, physical evidence, or bug disposition
consumes it. Missing evidence remains undefined and creates no claim or
validation assignment.

## Evidence posture

- Implementation-tested: inert scaffold and repository checks only; the
  retained serial-number propositions do not establish device behavior.
- Model-conformant: none.
- Physically observed: none.
- Qualified: none.

## Definition of stable

The repository can be considered stable only after its supported operations, limitations, failure/recovery behavior, target scope, and proportionate evidence are bounded and reproducibly verified. Lifecycle promotion and publication remain separate maintainer decisions.
