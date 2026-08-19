# ph-sht45-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT45 humidity and temperature sensor over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, and soft-reset abort/recovery.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Availability

This package is not available from crates.io.

## Current state

The package supports implementation-tested serial-number and one-shot T/RH reads, all six heater pulses, and soft reset for one SHT45-AD1B at 7-bit I2C address `0x44`. The serial read writes command `0x89`, waits at least 10 µs, reads six response bytes, validates both CRC-8 values, and returns the transmission-order `u32` serial number. A measurement writes `0xFD`, `0xF6`, or `0xE0` for high, medium, or low repeatability, waits the corresponding maximum duration of 8.3, 4.5, or 1.6 ms, then reads and validates six response bytes. A heater pulse selects high, medium, or low power and a long or short duration, writes the matching command, waits the complete 1.1083 s or 118.3 ms bound, and returns the heater's on-chip high-repeatability measurement. Soft reset writes `0x94`, waits 1 ms, and performs no response read. Measurement results are uncropped integer millidegrees Celsius and milli-%RH.

## Platform support

The crate is `no_std`, uses abstract `embedded-hal-async` I2C and delay resources, allocates no memory, and forbids unsafe code. The caller owns those resources, power-up timing, scheduling, retries, recovery policy, and heater cadence or duty-cycle policy. The driver owns the serial, measurement, heater-pulse, and soft-reset commands' execution waits. Model conformance covers the serial-number read, T/RH measurement at high, medium, and low repeatability, all six heater pulses, and soft-reset abort/recovery; no physical-device claim is made.

The independent model is a separate package; its existence does not establish conformance for this driver.

## License

MIT
