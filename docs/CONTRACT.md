# Sensirion SHT45 driver contract

## Responsibility

The repository owns truthful supported SHT4x operations on one device through an abstract async I2C bus.

## Non-goals

The repository does not own board topology; concrete bus/GPIO/power resources; sampling cadence; retry/escalation policy; heater application policy; retrieval or application of the SHT43's ISO/IEC 17025 three-point calibration data, per `SHT4X-SHT43-CAL-001`; model conformance beyond the explicitly named host-only serial-number, T/RH, heater-pulse, and soft-reset checks; or physical qualification.

The supported device set is the SHT40, SHT41, SHT43, and SHT45 at any documented address. That set rests on `SHT4X-FAMILY-SCOPE-001`, which records what the datasheet declares rather than what any part does: no operation has been executed against a physical device of any model, and the model conformance below was run against the independent model, not against silicon.

## Initial invariants

- Public behavior must not claim a device capability without retained, exact provenance.
- Single-device operations own the device-required sequencing, legal encodings, units, timing, state authority, and recovery semantics they need to be truthful.
- Concrete resources and application policy stay outside the driver.

Refine this list when implementation creates review-blocking invariants; do not inventory hypothetical behavior.

## Sources and device propositions

### Widening to the SHT4x family

The supported device set is being widened from the SHT45-AD1B alone to the
SHT40, SHT41, SHT43, and SHT45. The propositions below were retained while the
scope was one part, and their identifiers say so.

Identifiers are not rewritten. Section 10.2 requires an identifier to name one
stable referent, never to be reused or redefined, and to remain resolvable after
it is superseded or split. A proposition whose referent widens from the
SHT45-AD1B to the family is therefore a *different* proposition: it receives a
new `SHT4X-` identifier, and the `SHT45-` record it supersedes stays here,
marked superseded and still resolvable, so citations in merged commits, review
threads, and agent notes keep resolving.

A `SHT45-` proposition that is genuinely part-specific is not superseded at all.
It stays as it is and gains a family sibling rather than a replacement.

**Until a `SHT4X-` proposition exists for a behavior, that behavior is retained
for the SHT45-AD1B only**, whatever the package is named. The package name
carries the family identifier; the propositions carry the claims, and only they
decide what is supported.

### Outstanding reads for the widening

This decision work bounds what the widening needs and names the dependent work
it enables. Each item is a read of the already-pinned datasheet — no new source
is required — and none is a claim until it is performed and recorded:

| Needed | Enables | Status |
| --- | --- | --- |
| Question | Settled by | Outcome |
| --- | --- | --- |
| The per-part I2C addresses | Tables 11 and 12 | The address follows part-number position 7, not the sensor model. `SHT4X-PART-NOM-001`, `SHT4X-I2C-ADDR-001`. |
| Whether Table 8's commands, Table 5's timings, and section 4.6's conversion are stated per part | Bounded search of the pinned document | They are stated for the SHT4x without part qualification. `SHT4X-FAMILY-SCOPE-001`. |
| Whether any driver-observable behavior varies by accuracy grade | Same search | None is declared. Accuracy is a specification of a reading, not a step in producing one. `SHT4X-ACC-001`. |

The widening therefore needs no sensor-model parameter. It needs a
caller-supplied address, constrained to the three documented values, and honest
disclosure of what the evidence covers.

What the search does not settle is device behavior. `SHT4X-FAMILY-SCOPE-001`
records what the document declares; it creates no physical claim, no validation
assignment, and no release block, and it does not make the existing
model-conformance evidence cover a part it was never run against.

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

- `SHT4X-PART-NOM-001` — An SHT4x orderable part number encodes its properties
  positionally (Table 11). Position 5 is the accuracy grade: `0` base, `1`
  intermediate, `5` best, `3` ISO 17025 certified — the digit that makes a part
  an SHT40, SHT41, SHT45, or SHT43. Position 7 is the I2C interface: `A` for
  address `0x44`, `B` for `0x45`, `C` for `0x46`. Position 9 is `1` reserved or
  `C` three-point calibrated and certified. Evidence state: supported.
  Local consequence: accuracy grade and I2C address are independent positions of
  the part number. Neither is derivable from the other, so the driver must not
  infer an address from a sensor model or a model from an address.
- `SHT4X-I2C-ADDR-001` — The 7-bit I2C address of an SHT4x is fixed by position
  7 of its part number and is `0x44`, `0x45`, or `0x46` (Table 11; Table 12
  ordering rows). It is not a function of the sensor model: Table 12 lists
  SHT40 at `0x44`, `0x45`, and `0x46`, SHT43 at both `0x44` and `0x45`, and
  SHT41 and SHT45 at `0x44`. Evidence state: supported. **Supersedes
  `SHT45-I2C-ADDR-001`**, whose referent was one part rather than the family.
  Driver requirement: the address is a caller-supplied value constrained to the
  three documented values, not a constant and not something selected by a
  sensor-model parameter. A caller reads it off the part number they ordered.
- `SHT45-I2C-ADDR-001` — *(Superseded by `SHT4X-I2C-ADDR-001`; retained so
  existing citations resolve.)* The SHT45-AD1B 7-bit I2C address is `0x44`
  (device overview product table, ordering rows SHT45-AD1B-R2/R3; quick-start
  pseudocode). Evidence state: supported for the SHT45-AD1B.
  Its local consequence — that `0x45`/`0x46` are "SHT40" addresses — was a
  narrower reading than Table 11 supports: those addresses belong to part-number
  position 7 across the family, not to the SHT40. `SHT4X-I2C-ADDR-001` records
  the family fact; this record remains true of the SHT45-AD1B alone.
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
  A command write arriving while an unconsumed response is still pending — an
  unread serial frame, or measurement or heater data that reached its frontier —
  is likewise rejected as outside model fidelity rather than silently discarding
  that response; no retained source decides the device's behavior for that
  sequence, and soft reset is not declared to abort a completed response. That
  rejection applies only to a write the model would otherwise act on: a
  malformed length, an unsupported command, or a measurement without injected
  ticks keeps its own error, because such a frame commits nothing and so
  discards nothing.

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
  Table 8's caption qualifies those figures: they are **typical** values, valid
  for **VDD = 3.3 V**. Evidence state: supported.
  Driver requirement: `HeaterPower::High`, `HeaterPower::Medium`, and
  `HeaterPower::Low` name those levels in descending order and select the
  matching byte for either pulse duration.
  Local consequence: the public API names the documented level so a caller can
  choose deliberately, and no surface may present the wattage as a delivered or
  guaranteed figure. A typical value is not a bound, and the figures are
  qualified to one supply voltage; what the device draws at any other voltage is
  not something this repository has retained. Per `SHT45-HEAT-SEQ-001`,
  delivered energy, duty-cycle limiting, and watt metering remain outside this
  repository regardless.

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

- `SHT4X-ACC-001` — Measurement accuracy varies across the family by the
  accuracy grade at part-number position 5: base, intermediate, best, and the
  ISO 17025 certified grade (Table 11; Table 12 details column). Evidence state:
  supported.
  Local consequence: accuracy is a specification of a reading, not a step in
  producing one. The driver performs no grade-dependent processing, selects
  nothing by grade, and makes no accuracy claim of its own — stated accuracy
  belongs to the part a caller ordered, and system calibration and
  product-level accuracy are integration concerns.
- `SHT4X-FAMILY-SCOPE-001` — A documentary proposition about a bounded search of
  the pinned datasheet. Outside Tables 11 and 12, the accuracy grade of
  `SHT4X-ACC-001`, and the SHT43 calibration of `SHT4X-SHT43-CAL-001`, the
  document declares no variation between the SHT40, SHT41, SHT43, and SHT45. Its
  command table (Table 8), timing table (Table 5), CRC definition (Table 7,
  section 4.4), transfer behavior (section 4.1), conversion formulae (section
  4.6), reset behavior (sections 4.8), and heater sequence (section 4.9) are
  stated for the SHT4x without part qualification. Evidence state: supported as
  a statement about the document.
  Local consequence: the behaviors retained below for the SHT45 are documented
  for the family, so the driver may address any SHT4x at a documented address
  without part-dependent branching. Each `SHT45-` record keeps its own referent
  and wording; downstream work that relies on a behavior holding family-wide
  cites this identifier alongside it, rather than any `SHT45-` identifier being
  redefined.
  **Non-claim:** this records what the document declares, not what silicon does.
  A source that does not distinguish the parts is not evidence that the parts are
  indistinguishable. No physical evidence exists for any SHT4x here, and none of
  the model-conformance evidence recorded below was executed against a part
  other than the modeled SHT45.
- `SHT4X-SHT43-CAL-001` — Every SHT43 carries an individual three-point
  calibration at −30 °C, 5 °C, and 70 °C, accredited to ISO/IEC 17025:2017 by
  the Swiss Accreditation Service under SCS 0158. The expanded measurement
  uncertainty (k = 2, 95 % confidence) is 0.40 °C at −30 °C and 0.20 °C at both
  5 °C and 70 °C, under the shared-risk decision rule of JCGM 106:2012 section
  8.2. Each sensor is identified by the serial number read with the command of
  section 4.7, and its certificate and calibration data are downloaded per
  sensor, or reel-wise, from `libellus.sensirion.com` (section 2.4, Table 3).
  Evidence state: supported.
  Local consequence: this is an out-of-band data product retrieved over a
  network, not a device operation over I2C, so retrieving or applying it is
  outside this repository — as system calibration and product-level accuracy
  already are. The driver applies no per-device correction. What it does supply
  is the identifying serial number, through the operation recorded by
  `SHT45-SN-CMD-001`, which is the key the certificate is filed under.
  Omitting the certificate introduces no error the driver could otherwise have
  corrected: the certificate refines the stated accuracy of a reading, it does
  not change how the reading is converted.

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
  low repeatability, all six heater pulses, soft-reset abort/recovery, and each
  of the three documented I2C addresses, through the unpublished host-only
  conformance package's public driver/model adapter check, with independently
  asserted command, address, and maximum-delay mappings. This covers every
  current public device operation.
  It does not cover every supported *device*. The check runs against the
  independent model, and the model implements one behavior — the one the
  datasheet states for the SHT4x without part qualification, per
  `SHT4X-FAMILY-SCOPE-001`. Nothing here was executed against an SHT40, SHT41,
  SHT43, or SHT45. That the driver is claimed to work across the family follows
  from what the document declares, not from having exercised more than one part,
  and a document that does not distinguish the parts is not evidence that the
  parts are indistinguishable.
- Physically observed: none.
- Qualified: none.

## Definition of stable

The repository can be considered stable only after its supported operations, limitations, failure/recovery behavior, target scope, and proportionate evidence are bounded and reproducibly verified. Lifecycle promotion and publication remain separate maintainer decisions.
