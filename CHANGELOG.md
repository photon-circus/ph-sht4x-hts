# Changelog

## Unreleased

### Added

- Retained the SHT45 heater-power proposition covering which of the six heater
  command bytes selects which documented power level. The public API previously
  exposed an ordinal power selection with no retained record of that fact at
  all. The proposition is recorded as **unverified**: its figures have not been
  read against the pinned datasheet, so the public `HeaterPower` documentation
  names the command byte each variant selects and cites the proposition for the
  power figures rather than asserting a wattage.

- Initialized the bounded Sensirion SHT45 driver repository with an unpublished Incubating scaffold.
- Retained the source-backed SHT45 serial-number propositions used to unblock
  later driver and model work, including the command execution timing for the
  response read.
- Retained the source-backed SHT45 T/RH measurement propositions for command
  selection, maximum timing, integer conversion, and one-shot measurement
  data, without claiming the operation is implemented or model-conformant.
- Added the implementation-tested serial-number read through abstract async I2C,
  including the device-required command delay, mandatory CRC validation, and
  distinct no-acknowledge errors.
- Added implementation-tested high, medium, and low repeatability T/RH
  measurement with Table 5 maximum delays, mandatory CRC validation, and
  uncropped integer millidegree and milli-%RH conversion.
- Added an independent unpublished behavioral model for SHT45 serial-number and
  one-shot T/RH readouts, with explicit OTP and conversion-tick inputs, maximum
  busy frontiers, one-shot consumption, distinct out-of-fidelity and transaction
  errors, and model-only trace tests.
- Added an unpublished host-only conformance package that adapts the public
  driver I2C trace to the independent model, verifies the unequal-word
  `0x1234_5678` serial result and transmission order, and checks that a corrupted
  response produces the driver's CRC error.
- Added host-only model conformance for high, medium, and low T/RH measurement,
  including shared relative-delay advancement, injected `0xBEEF` ticks,
  independently asserted command and maximum-delay mappings, adapter CRC
  corruption, and a busy-model check for a no-op delay.
- Retained the source-backed SHT45 soft-reset command, 1 ms idle-time bound, and
  measurement-abort propositions from Datasheet D1 Version 7.3, without claiming
  reset implementation or model conformance.
- Added model-only soft-reset behavior that aborts an in-flight measurement,
  preserves the explicit OTP serial, and returns to idle after the 1 ms reset
  busy interval while preserving busy-write error precedence, without claiming
  driver conformance or physical evidence.
- Retained source-backed SHT45 heater-pulse command, completion-bound, and
  sequencing propositions for bounded driver and model work without claiming
  driver implementation, conformance, or physical evidence.
- Added model-only behavior for all six SHT45 heater-pulse commands, including
  explicit conversion ticks, exact long and short busy frontiers, one-shot CRC
  frame consumption, soft-reset abort, and busy-write rejection, without
  claiming driver conformance or physical evidence.
- Added the implementation-tested public soft-reset operation, which writes
  `0x94`, waits 1 ms, performs no response read, and maps write NACK and bus
  errors without claiming model conformance or physical evidence.
- Added host-only public driver/model conformance coverage for soft-reset abort
  of an in-flight measurement, the routed 1 ms delay, and serial recovery;
  this does not establish physical reset timing or hardware evidence.
- Added the implementation-tested public heater-pulse operation for all six
  power/duration combinations, with complete long/short waits, mandatory CRC
  validation, integer T/RH readout, and distinct NACK/bus errors. Heater model
  conformance, application duty-cycle policy, and physical evidence remain
  unclaimed.
- Added host-only public driver/model conformance for all six heater pulses,
  including routed long/short delays, injected integer T/RH results, CRC
  corruption discrimination, a no-op-delay busy check, and public soft-reset
  abort with serial recovery. This does not establish heater physics,
  application duty-cycle policy, or physical evidence.
- **Breaking (model):** the behavioral model now rejects a command write that
  would discard an unconsumed response, returning the new
  `WriteWithPendingResponse` limitation instead of silently replacing an unread
  serial frame or completed measurement or heater data. The previous behavior
  chose an outcome for a transport sequence no retained source decides. Soft
  reset is not exempt: it aborts a busy action under `SHT45-RST-ABORT-001`, but
  nothing declares that it discards a completed response. A frame the model
  would not act on keeps its own error — malformed lengths, unsupported
  commands, and measurements without injected ticks are still reported
  separately, because such a write commits nothing and so discards nothing.


- Host-only conformance coverage sweeping all 65 536 sixteen-bit words through
  the model's response frame and the driver's CRC validation, establishing that
  the two independent derivations agree across the whole input domain rather
  than on the datasheet's single vector alone.

### Changed

- The behavioral model now derives CRC-8 by reducing four bits per table lookup
  rather than with the bit-at-a-time shift register the driver uses. The two
  implementations were previously byte-identical, so an implementation defect
  would have reproduced itself in the oracle and survived conformance
  comparison. Model output is unchanged.


- Moved the conformance package's `embedded-hal`, `embedded-hal-async`,
  `ph-sht45-hts`, and `ph-sht45-hts-model` dependencies from `[dependencies]` to
  `[dev-dependencies]`. Its library target contains no conformance code and uses
  none of them; only the integration test does. The dependency graph now states
  that the driver and the model meet in a test rather than in a library.


- Replaced the driver's registry keywords with terms a reader would search for —
  `sht45`, `sensirion`, `humidity`, `i2c`, `embedded-hal-driver` — dropping
  `embedded` and `no-std`, which duplicated declared categories, and `hts`,
  which is an organization class token rather than a search term.
- Removed lifecycle and distribution status from the driver's manifest
  `description`, which now says what the crate provides. Status is carried by the
  packaged README disclosure that crates.io and docs.rs render, and by
  `publish = false`; the manifest was a second copy of the same facts to keep
  true, and status wording embedded in registry metadata is exactly what goes
  stale first.
- Removed `keywords` and `categories` from the model and conformance manifests,
  and the empty `[dependencies]` table from the model manifest. Those fields
  classify a crate for a registry neither package enters, and the model's
  dependency-free posture reads more clearly by omission.

### Documentation

- Disclosed that the measurement returned by a heater pulse is taken while the
  heater is energized. `heater_pulse` and `measure` share the `Measurement`
  type, so nothing in the type system records whether a result came from a
  heater-on or heater-off conversion. How far the two differ is heater physics
  and stays unretained and unclaimed, as does what either result implies about
  the surrounding air — that is a system-calibration question outside this
  repository. Documented on the operation, in the package README, and in the
  root README.

### Changed

- The local gate now compiles the driver for the `thumbv7em-none-eabihf` and
  `thumbv6m-none-eabi` bare-metal targets, reporting a distinct skip when a
  target is not installed. A `no_std` driver was previously only ever compiled
  for the host, which establishes nothing about the targets it exists to serve.
- The local gate now constructs the driver's package archive rather than only
  listing its contents, so cargo's verification build runs against the unpacked
  tree. `cargo package --list` exits successfully for a package whose source
  file has been excluded; construction does not. Over a dirty working tree the
  package checks cover that tree and print a notice saying so, so the gate stays
  runnable over uncommitted work without silently changing what it verified. An
  unreadable repository status is treated the same as a dirty tree, since cargo
  inspects the repository without the git CLI and would otherwise abort a gate
  that could not check.
- The declared version and publication lock now cover all three manifests
  instead of the driver alone, and are read from each `[package]` table rather
  than through `cargo pkgid`, which resolves through `Cargo.lock` and cannot see
  a manifest that has drifted from it.
- Every cargo invocation in the gate now uses `--locked`, making the committed
  `Cargo.lock` the resolved dependency set for verification.

### Documentation

- Recorded the verified bare-metal targets and what the local gate establishes.

### Added

- `Display` and `core::error::Error` for the driver's `Error`, so a caller can
  report a failure without matching every variant by hand. `source()` exposes
  the underlying transport error.

### Changed

- **Breaking:** the driver's `Error` is now `#[non_exhaustive]`, so a future
  variant is no longer a breaking change for downstream matches.
- Documented the `Error::Crc` fields, including which response word index `0`
  and `1` name for each operation, and enabled `deny(missing_docs)`.
- Named the measurement duration constants and expressed each heater bound as
  its pulse plus the trailing high-repeatability measurement, so the composite
  is visible rather than folded into one literal.
- Merged the duplicated CRC-validation loop in the driver, and the identical
  measurement and heater response arms in the model, into single
  implementations.
- Renamed the conformance test file from `serial_number.rs` to `conformance.rs`;
  it has covered every public operation since the measurement work.
- The serial-number conformance case now routes the driver's delay into model
  time and asserts its trace instead of using a no-op delay, and CRC corruption
  is exercised on both response words rather than only the first.
- Replaced the adapter's duplicate of a model-owned frame assertion with checks
  only the adapter can make: that a multi-operation transaction is rejected, and
  that a model limitation never reaches the driver as a claimed device NACK.

### Documentation

- Recorded the repository's load-bearing invariants and traps in `AGENTS.md`.

- Added `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, and
  `RELEASING.md`, written to the bar a public, crates.io-published repository is
  held to rather than to the minimum an unpublished private one currently
  requires. Each is repository-specific: the contribution guide carries the
  proposition rule and the evidence states that must not be collapsed, the
  security policy scopes itself to a `no_std`, allocation-free, unsafe-free
  driver on a board-local bus, and the release contract carries the version
  rules, the ordinary-release gate, and the publication steps.
- These documents change no status. The repository remains private, Incubating,
  and unpublished, and all three manifests retain `publish = false`.

### Known issues

- Model conformance covers every current public device operation; no operation
  is physically validated.
