# ID01–ID03 implementation evidence

This initial checkpoint precedes the ID04–ID08 update below. Its implementation
limits describe the read-only slice at that time.

## Scope and source

Implemented the shared read-only catalog service and development `idctl` CLI.
Base revision: `e1fcb0e014a9002cb82f45381c2096724047990f` plus uncommitted
overview, planning, workspace, and application changes. No immutable handoff
revision or registry release is claimed.

Executed on 2026-09-06, Rust 1.97.0, host/target `aarch64-apple-darwin`.
The inherited Cargo target directory is outside this repository; builds resolve
product source from this workspace, not sibling worktrees or installed applications.

## Checks

| Command | Result |
|---|---|
| `cargo test --offline -p styrene-identity-lifecycle -p styrene-identity-cli` | 6 backend tests and 9 bounded CLI process tests passed |
| `cargo test --workspace --locked` | 195 unit/integration tests plus 3 doctests passed; 3 existing doctests ignored |
| `cargo test --locked --features repository-signing,ssh-agent,pki,age-format` | Root-only expanded lane: 220 unit/integration tests plus 3 doctests passed; 4 existing doctests ignored |
| `cargo check --locked --lib --no-default-features` | Passed |
| `cargo check --locked --lib --no-default-features --features repository-signing` | Passed |
| `cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml` | All 6 isolated minimal-consumer scenarios passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked -p styrene-identity-lifecycle -p styrene-identity-cli --all-targets -- -D warnings` | Passed |
| `cargo doc --locked --no-deps -p styrene-identity-lifecycle -p styrene-identity-cli` | Passed |
| `cargo tree --locked -p styrene-identity-lifecycle --edges normal` | Inspected: no Mesh, Dioxus, Clap, or file-signer dependencies in the backend graph |
| `cargo run --locked -p styrene-identity-cli -- --output json capabilities` | Completed with schema-v1 JSON advertising the three reads and mutations unsupported |

CI now explicitly runs application tests and warning-denied Clippy. Root library
remains the only default member. The isolated overview fixture retains its own
unchanged lockfile.
Both OpenSpec changes validate as implementing. Whitespace checks pass, and
committed cryptographic vector files remain unchanged.

## Dependency assessment

The backend reuses dependencies already present in the root graph and disables
Identity default features. CLI parsing adds Clap, locked to 4.5.60, with derive
and standard default features. Its manifest declares Rust 1.74 and MIT OR
Apache-2.0 licensing, compatible with the pinned toolchain and project license.
Clap and its supporting packages are confined to the CLI graph. The process-test
deadline helper uses the already-resolved `wait-timeout` crate as a dev dependency.
Offline Cargo resolution added application packages and CLI dependencies without
updating existing locked package versions. Dependency advisory scanning remains
a release-policy gate, not evidence collected in this slice.

## Limits

Catalog metadata is unauthenticated public state. Hash-consistent output does not
prove key possession, current custody, or runtime binding. The source reads no
identity key files. Tests use disposable catalogs and no prompts or Mesh services.

No create/adopt/update/delete commands, custody credentials, operation journal,
durable mutation IDs, UI host registration, or native-device workflow is implemented
by this slice. JSON output schema v1 and catalog storage v1 are distinct contracts.

Clean-tree package verification remains deferred until a reviewed commit, with no
`--allow-dirty` bypass. Linux/Windows, mobile, UI runtime, token/device acceptance,
registry publication, and consumer adoption were not executed.

## ID04–ID08 file-catalog mutation update

Implemented create/adopt/update/select/forget plus operation show/reconcile in
the optional backend `file-custody` feature, enabled by the CLI. The root library
remains the only default workspace member. Base SHA remains
`e1fcb0e014a9002cb82f45381c2096724047990f` plus uncommitted local changes.
Validation ran on 2026-09-06 with Rust 1.97.0, `aarch64-apple-darwin`.

| Command | Result |
|---|---|
| `cargo test --workspace --locked --quiet` | 209 unit/integration tests and 3 doctests passed; 3 existing doctests ignored |
| `cargo test --locked --features repository-signing,ssh-agent,pki,age-format --quiet` | 220 root unit/integration tests and 3 doctests passed; 4 existing doctests ignored |
| `cargo test --locked -p styrene-identity-lifecycle --no-default-features --quiet` | 6 public catalog tests passed; mutation module excluded |
| `cargo check --locked -p styrene-identity-lifecycle --no-default-features` | Passed |
| `cargo check --locked --lib --no-default-features --features repository-signing` | Passed |
| `cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml --quiet` | All 6 isolated overview cases passed |
| `cargo clippy --locked -p styrene-identity-lifecycle -p styrene-identity-cli --all-targets -- -D warnings` | Passed after resolving one collapsible-if lint |
| `cargo fmt --all -- --check` | Passed |
| `cargo doc --locked --no-deps -p styrene-identity-lifecycle -p styrene-identity-cli --features styrene-identity-lifecycle/file-custody` | Passed |
| `cargo tree --locked -p styrene-identity-lifecycle --no-default-features --edges normal` | Inspected: no file-signer, Mesh, Dioxus, or CLI dependency in the public-only backend graph |

The application corpus now includes 9 backend mutation tests, 6 catalog tests,
5 CLI CRUD/recovery tests, and 9 CLI read/process tests. Mutation tests inject
failures after durable intent, custody installation, and catalog installation.
They also exercise a real filesystem blocker after custody commit, corrupted
receipts, legacy adoption, changed requests/revisions, and cross-process locking.
CLI subprocesses use disposable identities, explicit piped protection, and deadlines.
No production fault-injection environment variable or command is exposed.

The feature reuses existing locked SHA-256, tempfile, file-signer, and zeroization
dependencies. No new registry package versions were required for this update.
File locking uses the pinned Rust standard library. The recovery journal retains
encrypted STID bytes for admitted creation, with no plaintext credentials or roots.

Final checks also passed: `cargo check --locked --lib --no-default-features` and
`cargo check --locked --features keychain,yubikey` (compilation only). Both OpenSpec
changes validate as implementing. Whitespace checks pass; committed cryptographic
vectors and the isolated overview consumer lockfile are unchanged.

Current implementation limits are documented in `docs/file-custody-crud.md`:
Unix file mutations, existing parent directories, retained bounded journals,
synchronous execution, and no automatic abandonment, pruning, or mutation replay.
Public reads remain available independently. Backup-management commands, custody
destruction, general operation cancellation/waiting, and UI activation remain pending.

This is local software evidence, not physical power-loss, network-filesystem,
hardware custody, mobile, or UI acceptance. Package verification still requires a
reviewed clean commit; no dirty-tree packaging or registry publication was performed.
