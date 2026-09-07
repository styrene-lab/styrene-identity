# Adversarial hardening verification

## Revision and environment

Baseline: `e1fcb0e014a9002cb82f45381c2096724047990f` plus the uncommitted
Identity/lifecycle/CLI changes in this checkout. Rust 1.97.0, host/target
`aarch64-apple-darwin`, checked on 2026-09-06. The shared Cargo target directory
is an artifact location, not a sibling product-source dependency.

The historical README pin `7ce44fd8dac29299b88623ca0252e5f5cebcacfc` was
inspected with `git show` and also contains the permissive general verifier.
No registry-version or deployed-consumer exposure was established.

## Reproductions before mitigation

| Command | Observed failure |
|---|---|
| `cargo test --locked identity_verify_rejects_identity_point_forgeries -- --nocapture` | Forged identity-point signature accepted |
| `cargo test --locked -p styrene-identity-lifecycle --features file-custody adversarial_ -- --nocapture` | Both parent-substitution and unrelated-journal-transition rejection tests failed |

These cases were rerun after fixes and passed. Further tests cover inconsistent
public-object attribution, independently expected attestation identity, compact
receipts, missing custody after catalog commit, legacy retention and pending
migration, unsafe permissions, descriptor binding, and exclusive single-link install.

## Final software checks

| Command | Result |
|---|---|
| `cargo test --workspace --locked --quiet` | 226 unit/integration tests and 3 doctests passed; 3 existing doctests ignored |
| `cargo test --locked --features repository-signing,ssh-agent,pki,age-format --quiet` | 223 root unit/integration tests and 3 doctests passed; 4 existing doctests ignored |
| `cargo test --locked -p styrene-identity-lifecycle --no-default-features --quiet` | 7 public catalog/filesystem tests passed; mutation and backup-crypto modules excluded |
| `cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml --quiet` | All 6 isolated consumer scenarios passed |
| `cargo check --locked --lib --no-default-features` | Passed |
| `cargo check --locked --lib --no-default-features --features repository-signing` | Passed |
| `cargo check --locked --features keychain,yubikey` | Passed; compilation only |
| `cargo clippy --workspace --locked --all-targets -- -D warnings` | Passed after correcting a rustdoc list-spacing lint |
| `cargo clippy --locked --all-targets --features repository-signing,ssh-agent,pki,age-format -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo doc --workspace --locked --no-deps --features styrene-identity-lifecycle/file-custody` | Passed |

The new backup corpus proves inspection versus authentication, wrong identity and
credentials, altered payloads, header-stripped legacy compatibility, and unchanged
source files. CLI subprocess tests exercise both commands, including inspection
without a store or credential source.

The application tests cover Unix-domain special-file/symlink rejection on this
host. A Rustix FIFO helper was unavailable on Apple, so that fixture was replaced
with a Unix-domain socket; Linux FIFO runtime acceptance is not claimed.

## Dependency and contract effects

- Rustix 1.1.3 supplies safe descriptor-relative Unix operations (`fs`, `process`).
  Its declared MSRV is 1.63 and license is `Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT`. It was already present in the lockfile; no package version
  update was needed. It is a lifecycle-backend dependency, not a new core dependency.
- Argon2 0.5.3 now enables its `zeroize` feature, and the file signer owns the block
  workspace in a zeroizing wrapper. Existing parameters and format bytes remain
  unchanged. The lockfile gains the existing zeroize dependency edge.
- Source inspection of locked ChaCha20Poly1305 0.10.1 confirmed tag verification
  before plaintext transformation and zeroization of its owned key on drop. This
  is source evidence, not a memory-forensics or formal verification result.
- No derivation vectors, repository-signing corpus bytes, or rejection classes
  changed. General Identity verification intentionally tightens rejection behavior.
- Journal v2 and replay/retention fields are documented separately from cryptographic
  profiles. Legacy v1 recovery copies remain readable and are not silently erased.

## Limits

No formal crypto proof, memory-forensics experiment, physical power-loss test,
network-filesystem acceptance, Windows/Linux/mobile runtime run, or physical-token
acceptance was performed. Same-UID malicious code and whole-store rollback are
not prevented by filesystem checks or local receipt validation.

Clean-tree package verification, security-fixed immutable consumer pins, registry
publication, consumer enrollment revalidation, and legacy migration tooling remain
release/handoff work. Backup mutation/destruction commands remain planned; only
ID31–ID32 advance in this hardening slice.
