# ph-sht4x-hts-model

Unpublished independent behavioral model for selected SHT4x serial-number,
one-shot T/RH, and heater-pulse operations, including soft-reset abort behavior.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets `publish = false`.
> **Fidelity:** Model-only software behavior; it is neither driver conformance
> evidence nor physical-device evidence.

## Fidelity declaration

Modeled: one SHT4x at an explicitly chosen 7-bit I2C address — `0x44`, `0x45`,
or `0x46`, the values `SHT4X-I2C-ADDR-001` retains — with an explicitly provided
OTP serial and explicitly injected temperature and relative-humidity conversion
ticks. Serial accepts a separate `0x89` write followed by a six-byte read. The
response contains two big-endian serial words and their CRC-8 bytes; the serial
can be read again after another command write. Measurement accepts `0xFD`,
`0xF6`, and `0xE0` for high, medium, and low repeatability, respectively. It
models each command's maximum busy duration, the six-byte CRC response at or
after that frontier, and deletion after the first successful measurement read.
The six heater commands are accepted with long or short maximum busy frontiers
of 1.1083 s or 118.3 ms, respectively, and return the same injected six-byte
CRC frame with one-shot deletion. Malformed write lengths are reported
separately from unsupported one-byte commands. Soft reset accepts command
`0x94`, aborts a pending measurement or heater pulse, and keeps the device busy
for 1 ms before returning to idle. Construction rejects any address outside the
three retained values with `UnsupportedAddress`, rather than serving modeled
frames at an address the sources do not put a device at.

This is **one modeled behavior, not four devices.** `SHT4X-FAMILY-SCOPE-001`
records that the datasheet states this behavior for the SHT4x without
distinguishing the SHT40, SHT41, SHT43, and SHT45, and that documentary fact is
the whole basis on which a single modeled device stands in for any of them. It
is a statement about the document, not about the parts, and nothing here has
been executed against a physical device of any model.

Uncovered: every other SHT4x command, heater duty-cycle policy and physics,
general-call reset, clock stretching, ambient physics, autonomous CRC
corruption, and the device's response to writes while busy apart from
soft-reset abort. The model does not
represent hidden wall time; callers advance relative time explicitly, and a
busy measurement, heater, or reset read is modeled as a device NACK. Writes
addressed to the modeled device while a measurement, heater, or reset is busy return
`WriteWhileBusy` as an explicit out-of-fidelity error, except that nested soft
reset returns its distinct reset-busy limitation. A command write that would
discard an unconsumed response — an unread serial frame, or measurement or
heater data that has reached its frontier — returns
`WriteWithPendingResponse` and commits nothing. No retained source decides what
the device does for that sequence, including for soft reset, so it is an
explicit limitation rather than a modeled behavior. A frame the device would not
act on keeps its own error: a malformed length, an unsupported command, or a
measurement with no injected ticks is reported as such even while a response is
pending, because it could not have discarded anything.

The model exposes separate write and read operations, representing the
two-STOP transaction domain. Combined `write_read` or repeated-start behavior
is unsupported by this model.

This package is a behavioral selection aid for model-only tests, not a public
driver or a claim of model conformance by `ph-sht4x-hts`.

## License

MIT
