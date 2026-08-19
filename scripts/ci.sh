#!/usr/bin/env sh
set -eu

expected_version="0.1.0-incubating.1"
driver_manifest="crates/sht45/Cargo.toml"
manifests="crates/sht45/Cargo.toml crates/sht45-model/Cargo.toml crates/sht45-conformance/Cargo.toml"
# Supported bare-metal targets. A no_std driver compiled only for the host has
# not been shown to compile for the targets it exists to serve.
supported_targets="thumbv7em-none-eabihf thumbv6m-none-eabi"

echo "check: formatting"
cargo fmt --all -- --check

# Read a field from a manifest's [package] table. `cargo pkgid` resolves through
# Cargo.lock, so it reports the locked version rather than the declared one and
# cannot see a manifest that has drifted.
package_field() {
    awk -v field="$2" '
        /^\[/ { in_package = ($0 == "[package]"); next }
        in_package && $1 == field {
            sub(/^[^=]*=[[:space:]]*/, "")
            gsub(/"/, "")
            print
            exit
        }
    ' "$1"
}

echo "check: lifecycle version and publication lock"
for manifest in $manifests; do
    package_name="$(package_field "$manifest" name)"
    actual_version="$(package_field "$manifest" version)"
    if [ -z "$package_name" ] || [ -z "$actual_version" ]; then
        echo "failed: could not read a package name and version from $manifest" >&2
        exit 1
    fi
    if [ "$actual_version" != "$expected_version" ]; then
        echo "failed: expected $package_name version $expected_version, found $actual_version" >&2
        exit 1
    fi
    if ! grep -Eq '^publish[[:space:]]*=[[:space:]]*false' "$manifest"; then
        echo "failed: $manifest must retain publish = false" >&2
        exit 1
    fi
done

echo "check: clippy"
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

echo "check: tests"
cargo test --locked --workspace --all-features

echo "check: supported target compilation"
for target in $supported_targets; do
    if command -v rustup >/dev/null 2>&1 &&
        rustup target list --installed 2>/dev/null | grep -qx "$target"; then
        cargo build --locked -p ph-sht45-hts --target "$target"
    else
        echo "skipped: target $target is not installed"
    fi
done

echo "check: documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps

# Cargo packages the committed tree and refuses a dirty one. That is the right
# default for a release, but it would make the routine gate unrunnable over
# ordinary uncommitted work, so fall back to the working tree and say so. The
# release process runs from a clean checkout, where this notice cannot appear.
package_flags=""
if command -v git >/dev/null 2>&1 && [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    echo "notice: working tree is dirty; package checks cover it, not the committed tree"
    package_flags="--allow-dirty"
fi

# `--list` alone prints a file list without building anything. Constructing the
# archive also runs cargo's verification build from the unpacked tree, which is
# what catches a file missing from the packaged set.
echo "check: package construction"
cargo package --locked $package_flags --manifest-path "$driver_manifest"

echo "check: package contents"
cargo package --locked $package_flags --manifest-path "$driver_manifest" --list

if command -v cargo-deny >/dev/null 2>&1; then
    echo "check: dependencies and licenses"
    cargo deny check
else
    echo "skipped: cargo-deny is not installed"
fi

echo "passed: routine local software gate"
