#!/usr/bin/env sh
set -eu

echo "check: formatting"
cargo fmt --all -- --check

echo "check: lifecycle version and publication lock"
package_id="$(cargo pkgid -p "ph-sht45-hts")"
actual_version="${package_id##*@}"
if [ "$actual_version" != "0.1.0-incubating.1" ]; then
    echo "failed: expected package version 0.1.0-incubating.1, found $actual_version" >&2
    exit 1
fi
if ! grep -Eq '^publish[[:space:]]*=[[:space:]]*false' "crates/sht45/Cargo.toml"; then
    echo "failed: crates/sht45/Cargo.toml must retain publish = false" >&2
    exit 1
fi

echo "check: clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo "check: tests"
cargo test --workspace --all-features

echo "check: documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

echo "check: package contents"
cargo package --manifest-path "crates/sht45/Cargo.toml" --list

if command -v cargo-deny >/dev/null 2>&1; then
    echo "check: dependencies and licenses"
    cargo deny check
else
    echo "skipped: cargo-deny is not installed"
fi

echo "passed: routine local software gate"
