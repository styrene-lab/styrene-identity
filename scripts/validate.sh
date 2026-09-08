#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
threads="${IDENTITY_TEST_THREADS:-2}"
cargo fmt --all -- --check
cargo test --workspace --locked -- --test-threads="$threads"
cargo test --locked --features repository-signing,ssh-agent,pki,age-format -- --test-threads="$threads"
cargo check --locked --lib --no-default-features
cargo check --locked --lib --no-default-features --features repository-signing
cargo check --locked -p styrene-identity-lifecycle --no-default-features
cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml
cargo clippy --workspace --locked --all-targets -- -D warnings
cargo clippy --locked --all-targets --features repository-signing,ssh-agent,pki,age-format -- -D warnings
cargo doc --workspace --locked --no-deps --features repository-signing,ssh-agent,pki,age-format
if [[ "$(uname -s)" == Darwin ]]; then cargo check --locked --features keychain,yubikey; fi
git diff --exit-code -- tests/test-vectors.json tests/vectors
