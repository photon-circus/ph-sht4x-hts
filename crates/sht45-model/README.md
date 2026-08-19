# ph-sht45-hts-model

Unpublished independent behavioral model for selected SHT45-AD1B serial-number,
one-shot T/RH, and heater-pulse operations, including soft-reset abort behavior.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets `publish = false`.
> **Fidelity:** Model-only software behavior; it is neither driver conformance
> evidence nor physical-device evidence.

## Fidelity declaration

The canonical device propositions and their exact provenance live in the
repository contract, [`docs/CONTRACT.md`](../../docs/CONTRACT.md). This
declaration cites those identifiers and records only the model's own
consequence. It does not restate the propositions, the vendor wording, or the
source coordinates.

### Purpose and applied-stimulus boundary

The model predicts device-observable behavior at the abstract I2C transport
boundary the driver uses. It accepts separate `write` and `read` operations,
representing the two-STOP transaction domain; combined `write_read` or
repeated-start behavior is unsupported.

It holds no wall clock. Callers advance relative time explicitly through
`advance_ns`, and the model remains quiescent between inputs. Environmental
truth belongs to the caller: the OTP serial and the temperature and
relative-humidity conversion ticks are injected, never generated.

### Modeled

| Behavior | Proposition | Model consequence |
| --- | --- | --- |
| Device identity | `SHT45-I2C-ADDR-001` | One device at 7-bit `0x44`. Any other address returns `WrongAddress`. |
| Serial readout | `SHT45-SN-CMD-001` | A separate `0x89` write, then a six-byte read of two big-endian words with one CRC byte each. |
| Serial persistence | `SHT45-I2C-XFER-001` | The explicit OTP serial survives other commands and is re-readable after another command write. |
| Word integrity | `SHT45-CRC-001` | One CRC-8 byte per 16-bit word, derived independently of the driver. |
| Measurement commands | `SHT45-MEAS-CMD-001` | `0xFD`, `0xF6`, and `0xE0` accepted; each yields the injected six-byte CRC frame. |
| Measurement timing | `SHT45-MEAS-TIME-001` | Each command's maximum duration is its busy frontier; a read before it returns `Busy`, at or after it succeeds. |
| One-shot deletion | `SHT45-MEAS-ONCE-001` | The frame is deleted after the first successful read; a second read returns `MeasurementDataUnavailable`. |
| Busy read | `SHT45-I2C-XFER-001` | A read while busy is modeled as the device NACK, distinct from a model limitation. |
| Heater commands | `SHT45-HEAT-CMD-001`, `SHT45-MEAS-ONCE-001` | All six bytes accepted, each returning the injected six-byte CRC frame; the frame is deleted after the first successful read on the same terms as a measurement. |
| Heater timing | `SHT45-HEAT-TIME-001` | Long and short maximum busy frontiers, each including the trailing high-repeatability measurement. |
| Soft reset | `SHT45-RST-CMD-001`, `SHT45-RST-TIME-001`, `SHT45-RST-ABORT-001` | `0x94` accepted; the device stays busy for the whole 1 ms interval, then returns to idle with no payload. `SHT45-RST-TIME-001` gives that as a maximum; treating it as the exact frontier is a declared model abstraction. |
| Reset abort | `SHT45-RST-ABORT-001` | Soft reset aborts a busy measurement or heater pulse and begins reset timing. |

### Injected

The OTP serial number and the temperature and relative-humidity conversion
ticks are supplied by the caller. A measurement or heater command issued
without ticks returns `MissingMeasurementTicks` rather than inventing a value.

### Abstracted

The serial command's execution time (`SHT45-SN-CMD-001`) is not modeled as a
busy frontier, so the serial frame is available at the first read. The driver's
wait for it is asserted by the conformance test rather than enforced here.

### Excluded

Every other SHT45 command, heater duty-cycle policy and physics, general-call
reset, clock stretching, ambient physics, and autonomous CRC corruption. Their
absence implies no claim.

### Unsupported

These are explicit model limitations, reported distinctly from device behavior
and never as a fabricated NACK or payload:

- Malformed command frames: a write that is not exactly one byte returns
  `InvalidWriteLength`, separately from `UnsupportedCommand` for an unmodeled
  one-byte command. A read buffer that is not six bytes returns
  `InvalidReadLength`.
- Writes while a measurement, heater, or reset is busy return `WriteWhileBusy`;
  nested soft reset returns the distinct `ResetWhileBusy`. Only the soft-reset
  abort of `SHT45-RST-ABORT-001` is declared, so every other busy write stays a
  limitation.
- A command write that would discard an unconsumed response — an unread serial
  frame, or measurement or heater data that has reached its frontier — returns
  `WriteWithPendingResponse` and commits nothing. No retained source decides
  what the device does for that sequence, including for soft reset, so it is a
  limitation rather than a modeled behavior. A frame the model would not act on
  keeps its own error instead: a malformed length, an unsupported command, or a
  measurement with no injected ticks is reported as such even while a response
  is pending, because such a write commits nothing and so discards nothing.

### Nonclaims

Passing tests establish compatibility only with the behavior declared above.
This package is a behavioral selection aid for model-only tests. It is not a
public driver, not a claim of model conformance by `ph-sht45-hts`, and not
evidence of silicon behavior, electrical timing, or heater physics.

## License

MIT
