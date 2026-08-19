# ph-sht45-hts-conformance

Unpublished host-only conformance checks for the SHT45-AD1B serial-number
operation.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets
> `publish = false`.
> **Evidence:** The checks establish model conformance for the public
> serial-number read only. They are not physical-device evidence.

The integration test adapts the driver's abstract async I2C calls to the
independent model. It verifies the separate `0x89` write and six-byte read
trace through the driver's public result using unequal serial words to
discriminate transmission order, and corrupts a model response in the adapter
to ensure the driver's public CRC error is observable.

The adapter is test-only and is not compiled into either production library.
