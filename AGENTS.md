# Working in styrene-identity

This independent repository is the source authority for Styrene identity
cryptography, derivation, custody adapters, signed records, and identity backup
formats. Canonical origin: `https://github.com/styrene-lab/styrene-identity`.
The planned product also includes a shared lifecycle backend, CLI, standalone
Identity UI, and optional Mesh integration. See `docs/product-architecture.md`.
The backend and CLI implement public reads and Unix file-backed catalog CRUD with
durable recovery. See `docs/file-custody-crud.md`. Advanced custody and UI workflows
remain pending. The root library is the only default workspace member.
Work from this checkout; no sibling checkout or comms-lab installation is required.
Preserve existing work and inspect `git status --short` before edits.

## Read order

1. [README.md](README.md): public API and feature overview.
2. [CONTRIBUTING.md](CONTRIBUTING.md): toolchain, checks, fixtures, and handoff.
3. [Security model](SECURITY.md): secret handling and accepted limitations.
4. [Integration context](docs/integration-context.md): ownership and identity terminology.
5. [Plugin boundary](docs/plugin-boundary.md): accepted direction and next design gate.
6. [Product architecture](docs/product-architecture.md) and [release workflow](RELEASE.md):
   proposed application boundaries, SemVer policy, and release gates.

For derivation or wire changes, also read [the specification](docs/styrene-identity-spec.md),
[compatibility policy](COMPATIBILITY.md), and affected conformance tests.
The specification contains historical design proposals; its implementation-status
note distinguishes those proposals from current behavior.
[Extraction provenance](EXTRACTION.md) explains historical source references.

## Source map

| Area | Files |
|---|---|
| Features and public exports | `Cargo.toml`, `src/lib.rs` |
| Read-only application overview | `src/overview.rs`, `docs/read-only-overview.md`, `tests/overview_consumer.rs` |
| Lifecycle backend and public catalog | `crates/styrene-identity-lifecycle/`, `docs/catalog-schema.md` |
| CLI and process acceptance | `apps/cli/`, `docs/cli-lifecycle.md` |
| Derivation and canonical identity | `src/derive.rs`, `src/identity.rs`, `src/identity_id.rs` |
| Custody interface and selection | `src/signer.rs` |
| Encrypted storage and portable recovery | `src/file_signer.rs`, `src/vault.rs` |
| Platform and token adapters | `src/keychain_signer.rs`, `src/android_keystore_signer.rs`, `src/yubikey_signer.rs` |
| Public keys, export, discovery | `src/pubkey.rs`, `src/export.rs`, `src/format.rs`, `src/discover.rs` |
| SSH and certificates | `src/ssh_agent.rs`, `src/pki.rs` |
| Repository authority | `src/repository_signing.rs`, `tests/repository_signing_*.rs` |
| Runtime certificates and transitions | `src/records/` |
| Conformance inputs and generators | `tests/test-vectors.json`, `tests/vectors/`, `examples/generate_*.rs` |

## Invariants

- Preserve released derivation labels, salts, canonical encodings, and negative
  rejection classes. A changed profile requires explicit versioning and migration.
- Canonical identity signing and legacy Git commit signing share a seed.
  Repository authority uses its own epoch-indexed family and signed binding.
- Preserve legacy encrypted-file readers and non-overwriting creation/restore.
  Never use an operator's real identity as a test fixture.
- Do not log roots, derived private keys, PINs, passphrases, or decrypted backups.
  Zeroization wrappers do not guarantee that all caller copies are erased.
- `SignerTier` classifies a backend; it does not attest hardware custody.
  Current adapters release a root into process memory. See the security model.
- `SignerChain::new` preserves order; `new_sorted` sorts by tier. Both select the
  first available signer and propagate its operation error without trying another.
  Neither validates that configured signers represent the same identity.
- Keep the Identity library and lifecycle backend independent of mesh runtime,
  transport, mesh IPC, Dioxus, and presentation state. Planned CLI/UI packages
  depend on shared backend operations; host integration belongs at the boundary.
- Keep published consumers on immutable Git revisions until a reviewed release.
  Do not publish iterative extraction work to crates.io or commit sibling paths.

## Evidence

Use the commands in CONTRIBUTING. Report the exact revision, features, target,
commands, and limits of each check. Compilation is not device or token acceptance.
Do not infer UI, plugin, revocation, or lifecycle orchestration from a Rust type's
presence. Update the local documentation when changing these boundaries.
