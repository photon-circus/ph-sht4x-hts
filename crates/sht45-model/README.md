# ph-sht45-hts-model

Unpublished independent behavioral model for selected SHT45-AD1B serial-number
and one-shot T/RH operations.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets `publish = false`.
> **Fidelity:** Model-only software behavior; it is neither driver conformance
> evidence nor physical-device evidence.

## Fidelity declaration

Modeled: an SHT45-AD1B at 7-bit I2C address `0x44`, with an explicitly provided
OTP serial and explicitly injected temperature and relative-humidity conversion
ticks. Serial accepts a separate `0x89` write followed by a six-byte read. The
response contains two big-endian serial words and their CRC-8 bytes; the serial
can be read again after another command write. Measurement accepts `0xFD`,
`0xF6`, and `0xE0` for high, medium, and low repeatability, respectively. It
models each command's maximum busy duration, the six-byte CRC response at or
after that frontier, and deletion after the first successful measurement read.
Malformed write lengths are reported separately from unsupported one-byte
commands.

Uncovered: every other SHT45 command, heater timing, reset, clock stretching,
ambient physics, autonomous CRC corruption, and the device's response to writes
while busy. The model does not represent hidden wall time; callers advance
relative time explicitly, and a busy measurement read is modeled as a device
NACK. Writes addressed to the modeled device while a measurement is busy return
`WriteWhileBusy` as an explicit out-of-fidelity error and leave the pending
measurement unchanged.

The model exposes separate write and read operations, representing the
two-STOP transaction domain. Combined `write_read` or repeated-start behavior
is unsupported by this model.

This package is a behavioral selection aid for model-only tests, not a public
driver or a claim of model conformance by `ph-sht45-hts`.

## License

MIT
