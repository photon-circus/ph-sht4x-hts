# ph-sht45-hts-model

Unpublished independent behavioral model for the SHT45-AD1B serial-number
readout.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets `publish = false`.
> **Fidelity:** Model-only software behavior; it is neither driver conformance
> evidence nor physical-device evidence.

## Fidelity declaration

Modeled: an idle SHT45-AD1B at 7-bit I2C address `0x44`, with an explicitly
provided OTP serial, accepting a separate `0x89` write followed by a six-byte
read. The response contains two big-endian serial words and their CRC-8 bytes;
the serial can be read again after another command write. Malformed write
lengths are reported separately from unsupported one-byte commands.

Uncovered: every other SHT45 command, measurement and heater timing, reset,
clock stretching, ambient physics, autonomous CRC corruption, and busy-state
NACK behavior. The model does not represent hidden wall time.

The model exposes separate write and read operations, representing the
two-STOP transaction domain. Combined `write_read` or repeated-start behavior
is unsupported by this model.

This package is a behavioral selection aid for model-only tests, not a public
driver or a claim of model conformance by `ph-sht45-hts`.

## License

MIT
