# Sensirion SHT45 driver contract

## Responsibility

The repository owns truthful supported SHT45 operations on one device through an abstract async I2C bus.

## Non-goals

The repository does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application policy; SHT40/41/43 family claims; model conformance beyond the explicitly named host-only serial-number and T/RH checks; or physical qualification.

## Initial invariants

- Public behavior must not claim a device capability without retained, exact provenance.
- Single-device operations own the device-required sequencing, legal encodings, units, timing, state authority, and recovery semantics they need to be truthful.
- Concrete resources and application policy stay outside the driver.

Refine this list when implementation creates review-blocking invariants; do not inventory hypothetical behavior.

## Sources and device propositions

Retained propositions are limited to facts consumed by the implemented
serial-number and T/RH measurement operations or retained to bound prospective
soft-reset work. They are supported by
Sensirion Datasheet SHT4x, D1 Version 7.3 (June 2026),
`HT_DS_Datasheet_SHT4x_V7.3.pdf`,
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
  Local consequence: the driver issues `write([0x89])`, waits at least 0.01 ms
  through an abstract async delay resource, and then issues a separate 6-byte
  `read`; `0x89` is not the I2C read address byte.
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
- `SHT45-MEAS-CMD-001` — T/RH measurement uses command `0xFD` at high,
  `0xF6` at medium, or `0xE0` at low repeatability; the response is 6 bytes
  containing a 16-bit temperature word and CRC, followed by a 16-bit relative-
  humidity word and CRC (Table 8, section 4.3). Evidence state: supported.
  Driver requirement: the measurement operation issues one
  repeatability-selected command and then one separate 6-byte `read`; it will
  not poll or use `write_read`.
- `SHT45-MEAS-TIME-001` — Maximum measurement durations are 1.6 ms at low,
  4.5 ms at medium, and 8.3 ms at high repeatability (Table 5, `tMEAS,*`).
  Typical values are not the completion bound. Evidence state: supported.
  Driver requirement: the measurement operation waits the
  Table 5 maximum through the existing `DelayNs` resource: 1600, 4500, or
  8300 µs respectively, then performs the 6-byte read.
- `SHT45-MEAS-CONV-001` — Temperature and relative humidity convert as
  `T °C = -45 + 175 * t_ticks / 65535` and
  `RH % = -6 + 125 * rh_ticks / 65535` (section 4.6, formulae 1–2).
  Uncropped results may lie outside 0–100 %RH; the °F formula is not consumed.
  Evidence state: supported. Driver requirement: the measurement operation
  converts with integer millidegree and milli-%RH formulas,
  without floating point, and leaves results uncropped.
- `SHT45-MEAS-ONCE-001` — Measurement data can be received once and is deleted
  after the first acknowledged read header (section 4.1). Evidence state:
  supported. Model requirement: the measurement model uses explicitly injected
  measurement ticks; a read while the device is busy before the maximum timing
  frontier is a device NACK under `SHT45-I2C-XFER-001`, while a second read
  without a new command is a model limitation rather than an invented payload
  or NACK. The current model rejects every write while a measurement is busy as
  outside model fidelity without replacing the pending measurement. A future
  soft-reset model must implement the exception in `SHT45-RST-ABORT-001`.

- `SHT45-RST-CMD-001` — Soft reset is command byte `0x94`; the device ACKs and
  returns no CRC data payload (Table 8). Evidence state: supported.
  Local consequence: a future driver reset operation writes `[0x94]` and does
  not read a response payload.
- `SHT45-RST-TIME-001` — After ACK of soft reset, the maximum time to idle is
  1 ms (`tSR`, Table 5). The same bound is stated for general-call reset, which
  this repository does not consume. Evidence state: supported. Local
  consequence: a future driver reset operation waits 1000 µs after the ACK.
- `SHT45-RST-ABORT-001` — Any command that triggers an action can be aborted by
  soft reset (section 4.8). Evidence state: supported. Future model requirement:
  accept `0x94` while measurement is busy, discard the pending measurement, and
  remain busy for the 1 ms reset interval; reads before that interval NACK and
  reads after it observe an idle device. Until that behavior is implemented,
  soft reset remains outside the model's fidelity boundary.

Add the smallest permanent proposition and exact provenance only when current
implementation, model, conformance, physical evidence, or bug disposition
consumes it. Missing evidence remains undefined and creates no claim or
validation assignment.

## Evidence posture

- Implementation-tested: serial-number and T/RH measurement sequencing,
  response decoding, CRC validation, integer conversion, and error mapping
  through a scripted abstract I2C fake. This does not establish device behavior.
- Model-conformant: serial-number read and T/RH measurement at high, medium,
  and low repeatability, through the unpublished host-only conformance
  package's public driver/model adapter check, with independently asserted
  command and maximum-delay mappings. Other operations are uncovered.
- Physically observed: none.
- Qualified: none.

## Definition of stable

The repository can be considered stable only after its supported operations, limitations, failure/recovery behavior, target scope, and proportionate evidence are bounded and reproducibly verified. Lifecycle promotion and publication remain separate maintainer decisions.
