# Sensirion SHT45 driver contract

## Responsibility

The repository owns truthful supported SHT45 operations on one device through an abstract async I2C bus.

## Non-goals

The repository does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application policy; SHT40/41/43 family claims; model conformance beyond the explicitly named host-only serial-number, T/RH, heater-pulse, and soft-reset checks; or physical qualification.

## Initial invariants

- Public behavior must not claim a device capability without retained, exact provenance.
- Single-device operations own the device-required sequencing, legal encodings, units, timing, state authority, and recovery semantics they need to be truthful.
- Concrete resources and application policy stay outside the driver.

Refine this list when implementation creates review-blocking invariants; do not inventory hypothetical behavior.

## Sources and device propositions

Retained propositions are limited to facts consumed by the implemented driver
operations or the independent model's soft-reset and heater-pulse behavior. They
are supported by
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
  or NACK. The model rejects writes while a measurement or heater action is
  busy as outside model fidelity without replacing pending data, except that
  the soft reset in `SHT45-RST-ABORT-001` aborts it and begins reset timing.

- `SHT45-RST-CMD-001` — Soft reset is command byte `0x94`; the device ACKs and
  returns no CRC data payload (Table 8). Evidence state: supported.
  Driver requirement: the reset operation writes `[0x94]` and does not read a
  response payload.
- `SHT45-RST-TIME-001` — After ACK of soft reset, the maximum time to idle is
  1 ms (`tSR`, Table 5). The same bound is stated for general-call reset, which
  this repository does not consume. Evidence state: supported. Driver
  requirement: the reset operation waits 1000 µs after the ACK.
- `SHT45-RST-ABORT-001` — Any command that triggers an action can be aborted by
  soft reset (section 4.8). Evidence state: supported. Model requirement: accept
  `0x94` while measurement is busy, discard the pending measurement, and remain
  busy for the 1 ms reset interval; reads before that interval NACK and reads
  after it observe an idle device. The independent model implements this
  reset/abort trace; this is not driver-conformance or physical evidence.

- `SHT45-HEAT-CMD-001` — The six heater-on commands are `0x39`, `0x2F`, and
  `0x1E` for long pulses, and `0x32`, `0x24`, and `0x15` for short pulses.
  Each returns the same six-byte high-repeatability temperature/relative-
  humidity frame with one CRC byte per 16-bit word (Table 8). Evidence state:
  supported. Driver requirement: the public heater-pulse operation selects one
  command by power and duration and reads one six-byte response; the independent
  model accepts the six commands with explicit conversion ticks and independently
  owns the response CRC frame. Which byte carries which power level is recorded
  separately as `SHT45-HEAT-PWR-001`.
- `SHT45-HEAT-PWR-001` — Each heater command byte selects one of three
  documented heater power levels, stated in the same Table 8 command
  description that gives its pulse duration: `0x39` and `0x32` select 200 mW,
  `0x2F` and `0x24` select 110 mW, and `0x1E` and `0x15` select 20 mW.
  Evidence state: supported. Driver requirement: `HeaterPower::High`,
  `HeaterPower::Medium`, and `HeaterPower::Low` name those three levels in
  descending order and select the matching byte for either pulse duration.
  Local consequence: the public API names the documented level so a caller can
  choose it deliberately; per `SHT45-HEAT-SEQ-001`, delivered energy, duty-cycle
  limiting, and watt metering remain outside this repository.
- `SHT45-HEAT-TIME-001` — The maximum heater time is 1.1 s for a long pulse and
  0.11 s for a short pulse, followed by the high-repeatability measurement
  maximum of 8.3 ms (`tHeater`, `tMEAS,h`, Table 5). Typical pulse widths and
  watt figures are not completion bounds. Evidence state: supported. Driver
  requirement: the heater-pulse operation waits 1_108_300 µs or 118_300 µs
  before its single six-byte read; the independent model returns `Busy` before the
  corresponding frontier and makes the injected six-byte frame available at
  that frontier.
- `SHT45-HEAT-SEQ-001` — The heater sequence is heater on, timer expiry,
  high-repeatability measurement while the heater remains on, heater off, then
  data availability; there is no dedicated heater-off command (§4.9). Evidence
  state: supported. Local consequence: heater application policy, duty-cycle
  limiting, and watt metering remain outside this repository; soft reset
  aborts heater activity through `SHT45-RST-ABORT-001`, and other writes while
  heater-busy remain outside model fidelity.

Add the smallest permanent proposition and exact provenance only when current
implementation, model, conformance, physical evidence, an approved
evidence/decision work item, or bug disposition consumes it. Evidence/decision
work must bound the retained propositions, name the dependent work they enable,
and preserve explicit implementation, conformance, and physical-evidence
non-claims. Missing evidence remains undefined and creates no claim or
validation assignment.

## Evidence posture

- Implementation-tested: serial-number, T/RH measurement, and heater-pulse
  sequencing, response decoding, CRC validation, integer conversion, and error mapping
  through a scripted abstract I2C fake, including all six heater command bytes
  and both complete heater waits, plus soft-reset write sequencing, 1000 µs
  delay, and I2C/NACK error mapping. This does not establish model conformance,
  device behavior, physical timing, or heater duty-cycle policy.
- Model-only: the independent model covers soft reset while idle, measuring, or
  heating, measurement/heater abort, the 1 ms reset-busy frontier, heater and
  measurement busy frontiers, one-shot response deletion, and return to idle
  while preserving the explicit OTP serial. This does not establish driver
  conformance or device behavior.
- Model-conformant: serial-number read, T/RH measurement at high, medium, and
  low repeatability, all six heater pulses, and soft-reset abort/recovery,
  through the unpublished host-only conformance package's public driver/model
  adapter check, with independently asserted command and maximum-delay
  mappings. This covers every current public device operation.
- Physically observed: none.
- Qualified: none.

## Definition of stable

The repository can be considered stable only after its supported operations, limitations, failure/recovery behavior, target scope, and proportionate evidence are bounded and reproducibly verified. Lifecycle promotion and publication remain separate maintainer decisions.
