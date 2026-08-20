extern crate std;
use super::*;

fn advance_serial_reference_wait(model: &mut Sht4xModel) {
    // Literal oracle for `SHT4X-SN-WAIT-001`; do not derive this from the
    // implementation constant whose correctness these tests exercise.
    model.advance_ns(10_000_000);
}

#[test]
fn models_the_serial_reference_wait_as_a_limitation_then_returns_the_frame() {
    let mut model = Sht4xModel::new(0xbeef_beef);
    let mut response = [0; 6];
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    model.advance_ns(9_999_999);
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::SerialReadBeforeReferenceWait)
    );
    model.advance_ns(1);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
}

#[test]
fn models_high_measurement_busy_frontier_and_frame() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(8_299_999);
    assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
    model.advance_ns(1);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();

    assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
}

#[test]
fn models_medium_and_low_measurement_frontiers() {
    for (command, duration_ns) in [
        (MEASURE_MEDIUM_COMMAND, 4_500_000),
        (MEASURE_LOW_COMMAND, 1_600_000),
    ] {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
        let mut response = [0; 6];

        model.write(DEFAULT_ADDRESS, &[command]).unwrap();
        model.advance_ns(duration_ns - 1);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    }
}

#[test]
fn models_all_heater_frontiers_and_frame() {
    for (command, duration_ns) in [
        (0x39, 1_100_000_000),
        (0x2f, 1_100_000_000),
        (0x1e, 1_100_000_000),
        (0x32, 110_000_000),
        (0x24, 110_000_000),
        (0x15, 110_000_000),
    ] {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];

        model.write(DEFAULT_ADDRESS, &[command]).unwrap();
        model.advance_ns(duration_ns - 1);
        assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
        model.advance_ns(1);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
        assert_eq!(
            model.read(DEFAULT_ADDRESS, &mut response),
            Err(Error::MeasurementDataUnavailable)
        );
    }
}

#[test]
fn retains_split_delay_progress_and_consumes_measurement_once() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(4_000_000);
    model.advance_ns(4_300_000);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::MeasurementDataUnavailable)
    );
}

#[test]
fn rejects_writes_while_busy_without_replacing_the_measurement() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(4_000_000);
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WriteWhileBusy)
    );
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_LOW_COMMAND]),
        Err(Error::WriteWhileBusy)
    );

    model.advance_ns(4_300_000);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(
        response,
        [
            0x12,
            0x34,
            crc8([0x12, 0x34]),
            0x56,
            0x78,
            crc8([0x56, 0x78]),
        ]
    );
}

#[test]
fn aborts_measurement_with_soft_reset_and_returns_to_idle() {
    let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(1_000_000);
    model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
    model.advance_ns(SOFT_RESET_NS - 1);
    assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
    model.advance_ns(1);
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::ReadBeforeCommand)
    );

    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn aborts_heater_with_soft_reset_and_preserves_serial() {
    let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[HEATER_LONG_COMMANDS[0]])
        .unwrap();
    model.advance_ns(1_000_000);
    assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
    model.advance_ns(SOFT_RESET_NS - 1);
    assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
    model.advance_ns(1);
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::ReadBeforeCommand)
    );

    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn soft_reset_is_the_only_busy_write_exception() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(1_000_000);
    assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]),
        Err(Error::ResetWhileBusy)
    );
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WriteWhileBusy)
    );
}

#[test]
fn rejects_non_reset_writes_while_heater_is_busy() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

    model
        .write(DEFAULT_ADDRESS, &[HEATER_SHORT_COMMANDS[0]])
        .unwrap();
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WriteWhileBusy)
    );
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
        Err(Error::WriteWhileBusy)
    );
    assert_eq!(model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]), Ok(()));
}

#[test]
fn busy_state_precedes_malformed_write_validation() {
    for pending_command in [MEASURE_HIGH_COMMAND, SOFT_RESET_COMMAND] {
        let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
        model.write(DEFAULT_ADDRESS, &[pending_command]).unwrap();

        for bytes in [&[][..], &[SERIAL_NUMBER_COMMAND, 0x00][..]] {
            assert_eq!(
                model.write(DEFAULT_ADDRESS, bytes),
                Err(Error::WriteWhileBusy)
            );
        }
    }
}

#[test]
fn reset_accumulates_split_delay_and_works_when_idle() {
    let mut model = Sht4xModel::new(0x1234_5678);
    let mut response = [0; 6];

    model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
    model.advance_ns(400_000);
    assert_eq!(model.read(DEFAULT_ADDRESS, &mut response), Err(Error::Busy));
    model.advance_ns(600_000);
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::ReadBeforeCommand)
    );

    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn rejects_a_write_that_would_discard_a_pending_serial_response() {
    let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
        Err(Error::WriteWithPendingResponse)
    );

    // The rejected write commits nothing: the serial frame is still there.
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn rejects_a_write_that_would_discard_ready_measurement_data() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(HIGH_MEASUREMENT_NS);
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_LOW_COMMAND]),
        Err(Error::WriteWithPendingResponse)
    );

    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(
        response,
        [
            0x12,
            0x34,
            crc8([0x12, 0x34]),
            0x56,
            0x78,
            crc8([0x56, 0x78]),
        ]
    );
}

#[test]
fn rejects_a_write_that_would_discard_ready_heater_data() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0xbeef, 0xbeef);
    let mut response = [0; 6];

    model
        .write(DEFAULT_ADDRESS, &[HEATER_SHORT_COMMANDS[0]])
        .unwrap();
    model.advance_ns(110_000_000);
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WriteWithPendingResponse)
    );

    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);
}

#[test]
fn soft_reset_does_not_escape_the_pending_response_boundary() {
    // Soft reset aborts a busy action under SHT45-RST-ABORT-001. Nothing in
    // the sources declares that it also discards an unconsumed response, so
    // that sequence stays an explicit model limitation rather than an
    // inferred device behavior.
    let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);

    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]),
        Err(Error::WriteWithPendingResponse)
    );
}

#[test]
fn a_completed_reset_leaves_no_response_to_discard() {
    // Reset returns no payload, so a command after the reset interval is an
    // ordinary idle write and must not be caught by the pending-response
    // boundary. The public soft-reset conformance trace depends on this.
    let mut model = Sht4xModel::new(0x1234_5678);
    let mut response = [0; 6];

    model.write(DEFAULT_ADDRESS, &[SOFT_RESET_COMMAND]).unwrap();
    model.advance_ns(SOFT_RESET_NS);
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn busy_state_precedes_the_pending_response_boundary() {
    let mut model = Sht4xModel::new(0).with_measurement_ticks(0x1234, 0x5678);

    model
        .write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND])
        .unwrap();
    model.advance_ns(HIGH_MEASUREMENT_NS - 1);
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WriteWhileBusy)
    );
}

#[test]
fn a_frame_the_device_would_not_act_on_keeps_its_own_error() {
    // None of these would discard the pending serial frame, so none of them
    // may be reported as `WriteWithPendingResponse`.
    let cases: [(&[u8], Error); 3] = [
        (
            &[],
            Error::InvalidWriteLength {
                expected: 1,
                actual: 0,
            },
        ),
        (
            &[SERIAL_NUMBER_COMMAND, 0x00],
            Error::InvalidWriteLength {
                expected: 1,
                actual: 2,
            },
        ),
        (&[0x2c], Error::UnsupportedCommand(0x2c)),
    ];

    for (bytes, expected) in cases {
        let mut model = Sht4xModel::new(0x1234_5678).with_measurement_ticks(0xbeef, 0xbeef);
        let mut response = [0; 6];
        model
            .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
            .unwrap();

        assert_eq!(model.write(DEFAULT_ADDRESS, bytes), Err(expected));

        // And the response it could not have discarded is still there.
        advance_serial_reference_wait(&mut model);
        model.read(DEFAULT_ADDRESS, &mut response).unwrap();
        assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
    }
}

#[test]
fn missing_ticks_are_reported_over_a_pending_response() {
    // A measurement with no injected ticks is a model-input error. The model
    // cannot act on it, so it cannot discard the pending frame either.
    let mut model = Sht4xModel::new(0x1234_5678);
    let mut response = [0; 6];
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();

    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
        Err(Error::MissingMeasurementTicks)
    );

    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response, [0x12, 0x34, 0x37, 0x56, 0x78, 0x7d]);
}

#[test]
fn requires_explicit_measurement_ticks() {
    let mut model = Sht4xModel::new(0);

    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[MEASURE_HIGH_COMMAND]),
        Err(Error::MissingMeasurementTicks)
    );
}

#[test]
fn serial_read_is_stable_after_another_command() {
    let mut model = Sht4xModel::new(0x1234_5678);
    let mut first = [0; 6];
    let mut second = [0; 6];
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut first).unwrap();
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut second).unwrap();
    assert_eq!(first, second);
}

#[test]
fn owns_the_crc_vector() {
    let mut model = Sht4xModel::new(0xbeef_beef);
    let mut response = [0; 6];
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    advance_serial_reference_wait(&mut model);
    model.read(DEFAULT_ADDRESS, &mut response).unwrap();
    assert_eq!(response[2], 0x92);
}

#[test]
fn crc_remainder_table_matches_the_polynomial_it_claims() {
    // The table is data, so it is worth deriving independently of itself.
    // Entry i must be i << 4 divided by 0x31 over four most-significant-bit
    // shifts.
    for (index, entry) in CRC_NIBBLE_REMAINDERS.into_iter().enumerate() {
        let mut remainder = (index as u8) << 4;
        for _ in 0..4 {
            remainder = if remainder & 0x80 != 0 {
                (remainder << 1) ^ 0x31
            } else {
                remainder << 1
            };
        }
        assert_eq!(remainder, entry, "table entry {index}");
    }
}

#[test]
fn answers_on_whichever_documented_address_it_was_given() {
    // The address is a part-number property, so a model fixed at one address
    // could not discriminate a driver that ignored its own.
    for address in ADDRESSES {
        let mut model = Sht4xModel::at(address, 0xbeef_beef).unwrap();
        let mut response = [0; 6];

        model.write(address, &[SERIAL_NUMBER_COMMAND]).unwrap();
        advance_serial_reference_wait(&mut model);
        model.read(address, &mut response).unwrap();
        assert_eq!(response, [0xbe, 0xef, 0x92, 0xbe, 0xef, 0x92]);

        let other = if address == ADDRESSES[0] {
            ADDRESSES[1]
        } else {
            ADDRESSES[0]
        };
        assert_eq!(
            model.write(other, &[SERIAL_NUMBER_COMMAND]),
            Err(Error::WrongAddress {
                expected: address,
                actual: other,
            })
        );
    }
}

#[test]
fn refuses_to_model_a_device_at_an_undocumented_address() {
    // `SHT4X-I2C-ADDR-001` retains three addresses. Answering anywhere else
    // would invent a device the sources do not place there.
    for address in [0x00, 0x43, 0x47, 0xff] {
        assert!(matches!(
            Sht4xModel::at(address, 0xbeef_beef),
            Err(Error::UnsupportedAddress(reported)) if reported == address
        ));
    }
    for address in ADDRESSES {
        assert!(Sht4xModel::at(address, 0xbeef_beef).is_ok());
    }
}

#[test]
fn rejects_non_modeled_transactions() {
    let mut model = Sht4xModel::new(0xbeef_beef);
    let mut response = [0; 6];
    assert_eq!(
        model.write(0x45, &[SERIAL_NUMBER_COMMAND]),
        Err(Error::WrongAddress {
            expected: DEFAULT_ADDRESS,
            actual: 0x45
        })
    );
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[0x2c]),
        Err(Error::UnsupportedCommand(0x2c))
    );
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut response),
        Err(Error::ReadBeforeCommand)
    );
    model
        .write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND])
        .unwrap();
    assert_eq!(
        model.read(DEFAULT_ADDRESS, &mut [0; 5]),
        Err(Error::InvalidReadLength(5))
    );
}

#[test]
fn reports_malformed_command_frames_distinctly() {
    let mut model = Sht4xModel::new(0xbeef_beef);
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[]),
        Err(Error::InvalidWriteLength {
            expected: 1,
            actual: 0,
        })
    );
    assert_eq!(
        model.write(DEFAULT_ADDRESS, &[SERIAL_NUMBER_COMMAND, 0x00]),
        Err(Error::InvalidWriteLength {
            expected: 1,
            actual: 2,
        })
    );
}
