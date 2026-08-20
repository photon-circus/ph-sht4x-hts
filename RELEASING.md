# Releasing ph-sht4x-hts

This is the release contract for this repository. It implements Repository
Standards v0.1 section 17 and the Peripheral Driver Release and Evidence
Profile's software publication gate.

Publication is a maintainer decision and never a CI side effect. Registry
versions are permanent and cannot be overwritten.

## Current distribution state

The workspace is at `0.1.0-incubating.1` with `publish = false` on all three
manifests. Nothing has been published or tagged. The canonical gate asserts
both facts on every run, so an accidental version drift or an unlocked manifest
fails before it reaches a release.

Moving off that state is the deliberate act this document governs.

## What each dimension means

Three dimensions are reported independently and must never be collapsed:

| Dimension | Values | Current |
| --- | --- | --- |
| Distribution | unpublished, SemVer prerelease, ordinary release | Unpublished |
| Software maturity | Experimental, Incubating, Active, Maintenance, Archived | Incubating |
| Evidence | implementation-tested, model-conformant, physically observed, qualified | Model-conformant for every public operation; no physical evidence |

Publishing does not promote the lifecycle. Promoting the lifecycle does not
create evidence. Neither authorizes a hardware-support claim.

## Version rules

- The manifest version carries a prerelease identifier matching the lifecycle:
  `0.1.0-incubating.N` while Incubating.
- A later prerelease uses a higher numeric core, such as `0.1.1-incubating.1`.
  A lifecycle change must never decrease SemVer precedence.
- Removing the prerelease component is an intentional software-release
  transition governed by the ordinary-release gate below. It is not a
  hardware-qualification claim, and `0.1.0` is not an experimental publication
  merely because its major version is zero.
- Before 1.0 a breaking change increments the minor version. When compatibility
  impact is uncertain, take the larger defensible bump.
- All three lifecycle-controlled manifests move together; the gate enforces it.

## Gate for an Incubating prerelease

An Incubating crates.io prerelease requires truthful package documentation and
reproducible software evidence while retaining the full `-incubating.N` version
and Incubating lifecycle disclosure. The canonical gate must complete with zero
skipped or indeterminate checks.

Physical observation and hardware qualification are not prerequisites. Their
absence remains explicit in the packaged warning and limits only physical or
qualification claims.

## Gate for an ordinary release

Before publishing a version with no prerelease component, all of the following
must hold. Each is a fact about the repository, not an intention:

- A bounded public API, documented limitations, and accurate lifecycle and
  evidence status.
- Implementation-focused tests and supported-target compilation proportional to
  the driver.
- Nonempty unit and model-conformance coverage reports from the required
  `cargo-llvm-cov` tool. Their percentages are recorded, not thresholded.
- A passing `cargo xtask ci`.
- A changelog and this release process.
- A verified packaged artifact, assembled by an intentional maintainer action.

An ordinary release does **not** require a complete behavioral model, physical
evidence, hardware qualification, or `ph-hil` adoption — and must not be
described as establishing any of them.

## Release branches

A `release/<semver>` branch, including the prerelease component, is required
when several accepted pull requests must be assembled into one published
version. A release represented by a single independently verified pull request
may use the short path below.

For an assembled release:

1. Assemble accepted changes on `release/<semver>`.
2. Open the merge-back pull request as a draft early.
3. Preserve review history when routing accepted pull requests.
4. Keep later work off the release branch, and apply shared fixes
   upstream-first.
5. Close the changelog only after the release changes are assembled.
6. Run the full gate against the combined release, not against its component
   pull requests.
7. Record the evidence environment.
8. Inspect the exact package contents.
9. Tag the verified release commit.
10. Publish, then create the GitHub Release.
11. Merge the release branch back promptly.
12. Reopen `Unreleased` and delete the release branch.

The artifact being released, not its component pull requests, is what must be
verified.

## Steps

Run from a clean checkout of the commit to be released.

**0. Complete the public-distribution preflight.** Before the first public
publication, confirm that the repository is public, its unauthenticated source,
contract, issue, security, and license links resolve, the required Lifecycle and
Domain properties are set, and the bounded hosted `ci` workflow is available.
Recheck that the crate name is available and that registry credentials and
ownership are ready. These are administrative release facts; none creates or
requires physical evidence.

**1. Settle the changelog.** Move accumulated `## Unreleased` entries under a
`## X.Y.Z - YYYY-MM-DD` heading, dated in UTC. The section is a caller-facing
record, not a development diary. Preserve unresolved known limitations; do not
quietly drop them. Mark breaking changes `**Breaking:**`. A release introducing
a substantial capability carries a value statement immediately below the heading
saying why it was added, which limitation it addresses, what value it provides,
and what it costs. A list of APIs is not a value statement.

**2. Set the version.** Update all three manifests and `expected_version` in
`xtask/ci.ron`, then `cargo update --workspace` so `Cargo.lock` matches.

**3. Unlock publication.** Replace `publish = false` with `publish =
["crates-io"]` in `crates/sht4x/Cargo.toml` only. The model and conformance
packages stay unpublished.

The gate asserts `publish = false` in each lifecycle package whose
`require_unpublished` policy is true in `xtask/ci.ron`, and asserts the exact
`publish = ["crates-io"]` allow-list when that policy is false. This step must
therefore set the driver entry to false in the same commit. The model and
conformance entries remain true. The gate is meant to fail while the manifests
and intended distribution state disagree, so unlocking publication requires an
explicit policy edit rather than happening quietly or permitting another
registry.

**4. Bring the status disclosures up to the state you just created.** This comes
*after* the version and publication edits, not before: the root README and the
packaged README both embed the exact candidate version and state that the
manifest sets `publish = false`. Steps 2 and 3 make both statements false, and
the packaged README is what a reader on crates.io sees, so leaving them for a
later pass ships a package whose distribution claims contradict the package.

Update, in the same commit as steps 1 through 3:

- the version named in both status disclosures;
- the distribution line — unpublished, prerelease, or ordinary release, with the
  exact version;
- the model-conformance scope and physical-evidence status, which must state the
  actual evidence for the commit being released. Partial model coverage names
  covered and uncovered operations. A link to an unpackaged or private record
  does not satisfy this.

**5. Commit the release edits.** Steps 1 through 4 are one commit. The working
tree must be clean before anything is verified, tagged, or published: the gate's
package checks fall back to the working tree when it is dirty and say so, and
`cargo publish` refuses a dirty tree outright. Confirm with `git status
--porcelain` returning nothing.

**6. Verify.**

```sh
git rev-parse HEAD
cargo xtask ci
```

Record the first command's output as `<verified-commit>` immediately before
running the gate, along with the toolchain and host. Do not change HEAD after
this point; if it changes, restart at step 5. The gate exits unsuccessfully if
any check is skipped or indeterminate; only `result passed` with zero of both
is release evidence. Retain the printed `ci summary`, including the line,
function, and region totals (and `target/coverage/summary.txt`); they measure
software execution only and create no physical evidence. On a clean tree the
package checks print no notice; a notice here means step 5 was not finished.

**7. Inspect the artifact.**

```sh
cargo package --locked --manifest-path crates/sht4x/Cargo.toml --list
```

Confirm the packaged set contains the licence, README, and sources, and no
vendor material. The vendor datasheet is never committed and never packaged.
Record the generated `.crate` file's SHA-256 and carry that value into the
release record. Use `sha256sum` or PowerShell `Get-FileHash -Algorithm SHA256`
against the exact archive under `target/package`.

**8. Tag the verified commit.** Use the `<verified-commit>` recorded immediately
before the gate. Immediately before tagging, assert that HEAD still equals that
value and the worktree remains clean. Name the commit explicitly rather than
relying on `HEAD`. The tag is the full SemVer with a leading `v` and nothing
else; replace the placeholders below with the manifest version and verified
commit:

```sh
git tag -a v<version> <verified-commit> -m 'ph-sht4x-hts <version>'
git push origin v<version>
```

Verify that dereferencing `v<version>` resolves to `<verified-commit>` before
publication.

**9. Publish.**

Immediately before publishing, reassert the clean worktree, HEAD, and tag
identity, rerun package construction, and confirm that the archive SHA-256 is
unchanged. `cargo publish` repackages the current checkout, so these checks bind
that checkout to the artifact and tag that were reviewed.

```sh
cargo publish --locked --manifest-path crates/sht4x/Cargo.toml
```

**10. Create the GitHub Release** on that tag, containing the corresponding
changelog section and the coverage section of the retained `ci summary`, so a
tagged version discloses how complete its host-only evidence was. The
model-conformance totals are the relevant completeness figure; unit-test totals
are a different layer. **Mark it as a prerelease whenever the version has a
prerelease component.** The tag and the packaged version must match exactly
apart from the leading `v`. Include the recorded `.crate` SHA-256.

**11. Reopen `Unreleased`** in `CHANGELOG.md`.

## After publishing

Publication reserves the crate name, enables opt-in dependency evaluation, and
invites collaboration. It does not authorize a physical-device support claim
and does not elevate any evidence state. Update `SECURITY.md`'s supported
versions to name the published version.

## Yanking

Yank only when a published version is actively harmful — unsound behavior, a
security defect, or a wrong evidence claim in the packaged README. Yanking does
not delete the version. Record the reason in `CHANGELOG.md` and publish a fixed
version rather than relying on the yank alone.
