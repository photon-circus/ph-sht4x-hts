# ph-sht45-hts-conformance

Unpublished host-only conformance checks for selected SHT45-AD1B operations.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets
> `publish = false`.
> **Evidence:** The checks establish model conformance for the public
> serial-number read and T/RH measurement operations only. They are not
> physical-device evidence.

The integration test adapts the driver's abstract async I2C calls to the
independent model. It verifies the separate `0x89` write and six-byte read
trace through the driver's public result using unequal serial words to
discriminate transmission order, and exercises high, medium, and low
measurement commands with model-relative timing, injected ticks, and
adapter-corrupted CRC responses. Delay advancement is shared with the model;
a no-op delay leaves the model busy and fails through the driver's public
NACK error.

The adapter is test-only and is not compiled into either production library.
