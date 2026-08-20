# ph-sht4x-hts-conformance

Unpublished host-only conformance checks for public SHT4x operations.

> [!WARNING]
> **Lifecycle:** Incubating. This package is unpublished and sets
> `publish = false`.
> **Evidence:** The checks establish model conformance for the public
> serial-number read, T/RH measurement, heater pulse, and soft-reset
> abort/recovery operations. They are not physical-device evidence.

## What the checks compare

The integration test adapts the driver's abstract async I2C calls to the
independent model and asserts through the driver's public surface. Device facts
are cited by the identifiers retained in
[`docs/CONTRACT.md`](../../docs/CONTRACT.md); this README records only what the
comparison establishes.

| Public operation | Propositions under comparison | Discriminating check |
| --- | --- | --- |
| Addressing | `SHT4X-I2C-ADDR-001` | Every documented address is supplied to the driver alongside an independently literal model/bus address, and a driver built for one address against a model built for another must surface as the model's `WrongAddress` through the driver's public error path. |
| `read_serial_number` | `SHT4X-SN-CMD-001`, `SHT4X-SN-WAIT-001`, `SHT45-CRC-001` | The command, 10 ms vendor-reference wait, and address are asserted as independent literals; skipping the wait reaches the model's explicit `SerialReadBeforeReferenceWait` limitation rather than a fabricated device NACK. Unequal serial words distinguish transmission order, and an adapter-corrupted frame must surface as the driver's CRC error. |
| `measure` | `SHT45-MEAS-CMD-001`, `SHT45-MEAS-TIME-001`, `SHT45-MEAS-CONV-001` | Each repeatability's command byte and maximum-delay frontier are asserted independently of the driver, and injected ticks are compared against the decoded public millidegree and milli-%RH result. |
| `heater_pulse` | `SHT45-HEAT-CMD-001`, `SHT4X-HEAT-TIME-001`, `SHT45-MEAS-CONV-001` | All six power and duration selections, each with its own asserted command byte and inclusive long or short wait, and the same injected-tick to decoded-result comparison. |
| `reset` | `SHT45-RST-CMD-001`, `SHT45-RST-TIME-001`, `SHT45-RST-ABORT-001`, `SHT45-I2C-XFER-001` | Reset aborts an in-flight measurement or long heater pulse and routes the driver's delay into model time. Serial recovery afterwards rests on the OTP serial surviving, which is `SHT45-I2C-XFER-001`, not on the reset propositions. |
| Every read frame | `SHT45-CRC-001` | The driver's and the model's CRC are separately derived, so a sweep over all 65,536 words is what establishes they agree rather than a shared implementation. |

Delay advancement is shared with the model, so the driver's requested wait is
the input that moves the model's clock. A no-op delay provider therefore leaves
measurement, heater, or reset behavior busy, and leaves serial before its
reference-driver guard. The first case surfaces as a modeled device NACK; the
serial case remains a distinguishable model limitation. Those no-op cases are
deliberate discriminators, not shortcuts.

Two checks guard the adapter itself rather than the driver: a transaction the
model domain does not cover must be rejected instead of answered, and a model
limitation must stay distinguishable from a modeled device response. Without
them the adapter could satisfy a driver claim by inventing behavior neither the
driver nor the model owns.

Which power level a heater byte carries is `SHT45-HEAT-PWR-001`. Power is not
observable at the transport boundary, so these checks confirm the selection
reaches the bus, not the energy the device dissipates.

## What the checks do not establish

Passing means the covered driver claims remain compatible with the declared
model. It does not establish silicon behavior, physical timing, heater physics,
or application duty-cycle policy.

It also does not cover four devices. The comparison runs against the
independent model, and the model implements one behavior — the one the datasheet
states for the SHT4x without part qualification, per `SHT4X-FAMILY-SCOPE-001`.
Nothing here has been executed against a physical SHT40, SHT41, SHT43, or
SHT45, so coverage across the family rests on that documentary basis rather
than on execution.

The adapter is test-only and is not compiled into either production library.

## Execution coverage of the comparison

The named checks can pass while leaving production lines unexecuted. The local
gate reports that remainder as model-conformance coverage: production driver
and model lines, functions, and regions walked while this suite ran. Because
no physical run exists to close the gap, that figure is the honest completeness
of the host-only evidence — not a quality score and not a threshold.

Unit-test coverage of the same files is implementation-tested evidence. It must
not be cited as if it measured this comparison.

The current percentages are produced by `cargo xtask ci`. They appear in the
printed `ci summary` and in `target/coverage/summary.txt`; they are not copied
here, where they would go stale.
