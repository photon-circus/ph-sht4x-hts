# Security policy

This policy controls over the
[organization-wide fallback](https://github.com/photon-circus/.github/blob/main/SECURITY.md)
for `ph-sht4x-hts`.

## Reporting a vulnerability

Report privately through this repository's **Security** tab, using **Report a
vulnerability**. Do not open a public issue, pull request, or discussion for a
suspected vulnerability.

If private reporting is unavailable to you, open a
[minimal contact request](https://github.com/photon-circus/.github/issues/new?title=Private%20security%20contact%20requested&body=Private%20security%20contact%20requested.)
in the organization standards repository containing only the sentence "Private
security contact requested." Do not name this repository or describe the issue
there.

Include enough context to reproduce and assess:

- Package version or commit, enabled features, toolchain, and target triple.
- The concrete `embedded-hal-async` I2C and delay implementations in use.
- Impact, prerequisites, and the smallest safe reproduction.
- Device, silicon, board, and MCU revisions, and the bus mode, for
  hardware-sensitive reports.
- Whether the evidence came from physical hardware, a model or simulation, a
  mock, source documents, or code analysis.
- Known mitigations, and whether the issue is already public.

Never include credentials, access tokens, private repository contents, or
redistribution-restricted vendor material in a report or artifact. That
includes the vendor datasheet, which this repository deliberately does not
commit.

## What is in scope

This crate is a `no_std`, allocation-free, `#![forbid(unsafe_code)]` device
driver. Its security-relevant surface is narrow, and reports against it are in
scope:

- Response handling that could panic, index out of bounds, or overflow on
  hostile or malformed bus data. The driver reads fixed six-byte frames; any
  input that escapes those bounds is a defect.
- CRC validation that could be bypassed, so corrupted data reaches a caller as
  a valid `Measurement` or serial number.
- Conversion arithmetic that could overflow or wrap for any 16-bit tick value.
- Anything that would make the crate allocate, require `std`, or introduce
  unsafe code contrary to its declared posture.
- Denial of service through an unbounded wait, or a device-required wait that a
  caller cannot cancel through the async boundary.

Untrusted-input reports are welcome even though I2C is a board-local bus.
A driver that misbehaves on a malformed frame is a defect whether the frame
came from an attacker or from a failing connector.

## What is out of scope

- Physical access, probing, glitching, or bus tampering.
- Confidentiality or authenticity of the I2C bus itself. The SHT45 offers
  neither, and this driver does not add them.
- Concrete bus, power, clock, or GPIO implementations, which belong to
  integration.
- Application-level policy: retry, escalation, rate limiting, heater duty-cycle
  limiting, and thermal safety are the caller's, not the driver's.
- Vulnerabilities in the SHT45 device or its datasheet, which are the vendor's.

A report we cannot act on is still worth filing; we will say so and, where we
can, name the layer that owns it.

## Supported versions

This package is Incubating and unpublished. The supported version is the tip of
`main`; there are no released versions to backport to. A fix ships as an
ordinary change under `## Unreleased` in `CHANGELOG.md`.

When the package begins publishing, the most recent published version is
supported and this section records that.

## Response expectations

This repository is maintained by one person, so acknowledgement is best-effort
rather than contractual. Reports are triaged in the order received, valid
findings are fixed on `main`, and reporters are credited in `CHANGELOG.md`
under `Security` unless they ask otherwise.

Coordinated disclosure is preferred. Where a fix requires an evidence state
this repository does not have — physical observation, for example — the
limitation is recorded honestly rather than a claim being upgraded to close the
report.
