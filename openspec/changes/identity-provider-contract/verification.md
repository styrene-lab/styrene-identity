# Read-only slice verification

## Source and environment

Base Identity revision: `e1fcb0e014a9002cb82f45381c2096724047990f`, plus
uncommitted overview implementation and planning changes. There is not yet an
immutable consumer handoff revision.

Executed on 2026-09-06 with Rust 1.97.0, host/target
`aarch64-apple-darwin`. The environment inherits a Cargo target directory at the
outer coordination checkout; that artifact location is not a source dependency.
No sibling source or installed daemon was used for these checks.

## Executed checks

| Command | Result |
|---|---|
| `cargo test --locked --test overview_consumer` | 6 passed |
| `cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml` | Same 6 scenarios passed with default features disabled |
| `cargo tree --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml --edges normal` | Inspected: no Mesh, Dioxus, CLI, or file-signer dependencies |
| `cargo test --locked` | 180 unit/integration tests and 3 doctests passed; 3 existing doctests ignored |
| `cargo test --locked --features repository-signing,ssh-agent,pki,age-format` | 220 unit/integration tests and 3 doctests passed; 4 existing doctests ignored |
| `cargo check --locked --lib --no-default-features` | Passed |
| `cargo check --locked --lib --no-default-features --features repository-signing` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets --features repository-signing,ssh-agent,pki,age-format -- -D warnings` | Passed |
| `cargo clippy --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml --all-targets -- -D warnings` | Passed |
| `cargo doc --locked --no-deps --features repository-signing,ssh-agent,pki,age-format` | Passed |
| `cargo check --locked --features keychain,yubikey` | Passed; compilation only |

The isolated fixture lockfile was generated offline. Its dependencies resolve
independently from the root lockfile, exercising an actual minimal consumer graph.
OpenSpec validation passed (`identity-provider-contract`: implementing;
`identity-product-foundation`: planned). `git diff --check` passed. Committed
vector files and the root `Cargo.lock` are unchanged.

## Limits and next gate

Package verification is deferred until a reviewed clean commit, as required by
`CONTRIBUTING.md`; no `--allow-dirty` bypass was used. Registry publication, API
SemVer comparison, Linux, mobile, physical-token, and UI runtime acceptance were
not executed. Mock conformance does not prove that a real source avoids secret
access or that a host correctly disposes subscriptions.

The UI owner must review the concrete DTOs and implement host registration/scope
lifecycle against an immutable Identity revision. Production public-summary
sources, interaction evidence, signing, and mutation recovery remain later work.
