# ph-sht4x-hts

Incubating unpublished async no_std Rust driver for the Sensirion SHT4x humidity and temperature sensors — SHT40, SHT41, SHT43, and SHT45 — over abstract I2C.

> [!WARNING]
> **Lifecycle:** Incubating — bounded work intended to become a supported driver
> **Distribution:** Unpublished; the candidate version is `0.1.0-incubating.1` and the manifest sets `publish = false`.
> **Supported devices:** SHT40, SHT41, SHT43, and SHT45, at any of the three documented I2C addresses. The address comes from the part number, not the sensor model.
> **Model conformance:** The unpublished host-only conformance check covers the driver's serial-number read, one-shot T/RH measurement at all three repeatabilities, all six heater pulses, soft-reset abort/recovery, and every documented address, against the independent model. That model implements behavior the datasheet states for the SHT4x without part qualification, recorded as `SHT4X-FAMILY-SCOPE-001`; **no check has been executed against any physical part**, so coverage across the family rests on that documentary basis rather than on execution.
> **Physical evidence:** None. No reviewed physical-device evidence supports a physically observed or qualified claim.
> Evidence and limitations apply only to named operations; publication does not imply hardware qualification.

## Availability

This package is not available from crates.io.

## Current state

The package supports implementation-tested serial-number and one-shot T/RH reads, all six heater pulses, and soft reset for one SHT4x — an SHT40, SHT41, SHT43, or SHT45 — at 7-bit I2C address `0x44`, `0x45`, or `0x46`. The caller supplies the address, reading it off position 7 of the part number per `SHT4X-PART-NOM-001` and `SHT4X-I2C-ADDR-001`; it is not a function of the sensor model, so it is never inferred and the bus is never scanned. The serial read writes command `0x89`, waits at least 10 µs, reads six response bytes, validates both CRC-8 values, and returns the transmission-order `u32` serial number. A measurement writes `0xFD`, `0xF6`, or `0xE0` for high, medium, or low repeatability, waits the corresponding maximum duration of 8.3, 4.5, or 1.6 ms, then reads and validates six response bytes. A heater pulse selects one of the three heater commands available for the requested duration, waits the complete 1.1083 s or 118.3 ms bound, and returns the heater's on-chip high-repeatability measurement. Which command carries which power level is recorded as `SHT45-HEAT-PWR-001`: 200 mW, 110 mW, or 20 mW, descending. Those are the datasheet's typical figures at VDD = 3.3 V, not a delivered or guaranteed power, and they are qualified to that one supply voltage. Soft reset writes `0x94`, waits 1 ms, and performs no response read. Measurement results are uncropped integer millidegrees Celsius and milli-%RH.

## Heater readings are not ambient readings

A heater pulse converts while the heater is still on. The `Measurement` it
returns therefore describes the heated sensor rather than the surrounding air.
How the two differ is heater physics, which this repository does not retain,
model, bound, or correct.

`heater_pulse` and `measure` share the `Measurement` type, so nothing in the
type system prevents one being used where the other is meant. Use `measure` for
a reading taken with the heater off. What either reading implies about the
surrounding air is a system-calibration question this repository does not
answer.

## Platform support

The crate is `no_std`, uses abstract `embedded-hal-async` I2C and delay resources, allocates no memory, and forbids unsafe code. The caller owns those resources, power-up timing, scheduling, retries, recovery policy, and heater cadence or duty-cycle policy. The driver owns the serial, measurement, heater-pulse, and soft-reset commands' execution waits. Model conformance covers the serial-number read, T/RH measurement at high, medium, and low repeatability, all six heater pulses, soft-reset abort/recovery, and every documented address. It compares the driver against the independent model, which implements behavior the datasheet states for the SHT4x without part qualification (`SHT4X-FAMILY-SCOPE-001`). No check has been run against a physical part of any model, so coverage across the family rests on that documentary basis and not on execution; no physical-device claim is made.

The independent model is a separate package; its existence does not establish conformance for this driver.

## License

MIT
