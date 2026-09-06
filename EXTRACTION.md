# Extraction from styrene-rs

This repository retains path-filtered history of `crates/libs/styrene-identity`
from `styrene-lab/styrene-rs` through source revision
`579ee533cf37f15f61e3d3661bb52635b0dd897d`. Filtered commits have new hashes;
original generator revisions in corpus provenance remain references to styrene-rs.

Production Rust source, generators and vector payloads are unchanged. The
provenance test resolves its historical crate prefix locally. The manifest makes
edition/MSRV explicit, isolates the workspace, and fixes the pre-existing placement
of SSH-agent dependencies beneath the Android-only dependency table. That feature
failed in the source baseline on macOS. No derivation or custody behavior changed.

`extraction.json` records the extraction-time digests at `7ce44fd8` for the 41
original crate files. It is a historical record, not a digest manifest for later
maintenance commits. The MIT license preserves original notices as a copied file; unrelated monorepo LICENSE-only commits are excluded from this repository history. The lockfile originates from
the backend lock and is pruned for the standalone dependency graph.

Rust 1.97.0 is pinned. Run:

```sh
cargo test --locked
cargo test --locked --features repository-signing,ssh-agent,pki,age-format
cargo check --locked --lib --no-default-features
cargo check --locked --lib --no-default-features --features repository-signing
cargo package --locked
```

Hardware access and native device lifecycle tests require explicit separate runs.
The CI hardware-feature check only compiles adapters. There is no publishing job.
Use a full Git revision in consumers until a reviewed crates.io release is ready.

The repository owns cryptography, derivation, custody adapters, and identity
backup formats. Mesh runtime/profile orchestration remains in styrene-rs. Plugin
management UI and a narrower provider boundary are subsequent integration work.
