# ph-sht45-hts-conformance

Unpublished host-only conformance checks for selected SHT45-AD1B operations.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets
> `publish = false`.
> **Evidence:** The checks establish model conformance for the public
> serial-number read, T/RH measurement, and soft-reset abort/recovery
> operations. They are not
> physical-device evidence.

The integration test adapts the driver's abstract async I2C calls to the
independent model. It verifies the separate `0x89` write and six-byte read
trace through the driver's public result using unequal serial words to
discriminate transmission order, and exercises high, medium, and low
measurement commands with model-relative timing, injected ticks, and
adapter-corrupted CRC responses. The test independently asserts each public
repeatability's exact command byte and maximum-delay frontier. Delay
advancement is shared with the model; a no-op delay leaves the model busy and
fails through the driver's public error path. Soft-reset coverage starts an
in-flight measurement, routes the driver's 1 ms delay into model time, and
verifies serial recovery; a no-op reset delay remains visibly busy.

The adapter is test-only and is not compiled into either production library.
