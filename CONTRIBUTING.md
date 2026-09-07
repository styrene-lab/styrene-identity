# Development and validation

This crate is a standalone Cargo workspace. `rust-toolchain.toml` pins Rust
1.97.0; `Cargo.toml` specifies edition 2024 and Rust 1.97. Use the committed
lockfile. No parent workspace, daemon, UI, or lab host is needed for software tests.

The root library remains the default workspace member. The read-only lifecycle
backend and CLI are additional members with explicit CI lanes. Run their checks:

```sh
cargo test --locked -p styrene-identity-lifecycle -p styrene-identity-cli
cargo check --locked -p styrene-identity-lifecycle --no-default-features
cargo clippy --locked -p styrene-identity-lifecycle -p styrene-identity-cli --all-targets -- -D warnings
```

`cargo test --workspace --locked` includes both application packages. A root-only
`cargo test --locked` does not prove their process contracts.

The CLI enables the backend's optional `file-custody` feature. Combined application
tests exercise mutation recovery on Unix. A backend-only build with no default
features retains the public read surface without file-custody dependencies.

## Before editing

```sh
git status --short
git remote -v
rustup show
```

Read [AGENTS.md](AGENTS.md) for ownership and invariants. Inspect feature gates in
`Cargo.toml` and `src/lib.rs` before adding imports. Minimal builds are supported;
`--no-default-features` is not a claim of `no_std` support.

## Software checks

Run from the repository root. These commands match the Linux/macOS contract CI:

```sh
cargo test --locked
cargo test --locked --features repository-signing,ssh-agent,pki,age-format
cargo check --locked --lib --no-default-features
cargo check --locked --lib --no-default-features --features repository-signing
cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml
```

For public API documentation changes:

```sh
cargo doc --locked --no-deps --features repository-signing,ssh-agent,pki,age-format
```

Before a profile-bearing release, also satisfy every gate in
[COMPATIBILITY.md](COMPATIBILITY.md), including formatting, Clippy, rustdoc, and
dependency policy. The current CI does not automate all of those release gates.
Useful local formatting and lint commands are:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --features repository-signing,ssh-agent,pki,age-format -- -D warnings
```

Do not reformat unrelated source to hide existing failures. Record pre-existing
failures separately. After committing reviewed changes, package from a clean tree:

```sh
cargo package --locked
git diff --exit-code
git diff --cached --exit-code
test -z "$(git ls-files --others --exclude-standard)"
```

CI rejects tracked and untracked mutations from tests or packaging. Do not bypass
this guard with `--allow-dirty`, fixture regeneration, or ignored test failures.

## Platform and hardware evidence

On macOS, CI also runs:

```sh
cargo check --locked --features keychain,yubikey
```

Apple Keychain requires an Apple target. Android Keystore code is gated on an
Android target; a host build with its feature does not compile that adapter.
Android checks need an Android Rust target and the consumer's SDK/NDK setup.
Avoid `--all-features` as a substitute for an explicit platform matrix.

Token, Keychain, and Keystore acceptance requires separate device runs. Record
OS/device, adapter, identity binding, interaction policy, lock/unlock behavior,
missing-device behavior, and recovery outcome. Use disposable identities. An
ignored test, emulator, or compile check does not prove physical-token behavior,
Secure Enclave signing, StrongBox use, or biometric enforcement.

## Fixtures and compatibility

`tests/test-vectors.json` pins derivation outputs. Repository-signing positive
and negative corpora live under `tests/vectors/repository-signing-v1/`.
The provenance tests verify their digests and generator source references.
The minimal consumer under `tests/minimal-repository-signing-consumer/` exercises
an isolated dependency surface; its local test dependency is not a deployment pin.

Normal tests consume committed vectors. Never regenerate a vector just to make a
failure pass. For an intentional new profile, review the profile and migration
first, then run the corresponding generator in `examples/` and record exact
commands, generator revision, and digests. Historical provenance can refer to
styrene-rs; see [EXTRACTION.md](EXTRACTION.md). Do not rewrite those references to
pretend the generators originated here. `extraction.json` describes the extraction
revision, not the current maintenance tree.

## Consumer handoff and publication

The proposed repository-wide [release workflow](RELEASE.md) covers package SemVer,
CLI automation, application distribution, and plugin compatibility. Its automation
and first-release inventory remain pending in the product foundation OpenSpec.

1. Commit the reviewed change and retain its validation results.
2. Provide the full Identity Git SHA, API/feature impact, and any migration need.
3. Have each consumer update its manifest and lockfile together.
4. Verify resolved Cargo sources, including member/target dependencies and patches.
5. Run affected consumer tests against that exact revision.

A floating branch, package version string, or successful crate build does not
establish which Identity source a consumer used. The initial extracted Git pin
is recorded in README. New pins need their own evidence.

There is no crates.io publishing job. Registry publication and version selection
are a separate release decision; do not publish merely to make a consumer import
work. Git commit signatures do not establish Styrene repository authority.
