# idctl: Identity CLI

Development CLI for the shared Identity lifecycle backend. The package is
`styrene-identity-cli`; `idctl` is its current development executable name.
It is not published. It does not require a Mesh daemon or UI. Public reads and
Unix file-backed catalog CRUD are implemented.

## Run from this workspace

```sh
cargo run --locked -p styrene-identity-cli -- capabilities
cargo run --locked -p styrene-identity-cli -- --output json capabilities
cargo run --locked -p styrene-identity-cli -- --store /path/to/catalog-directory identity list
cargo run --locked -p styrene-identity-cli -- --store /path/to/catalog-directory identity show example-entry
cargo run --locked -p styrene-identity-cli -- backup inspect /path/to/backup.stid
```

Use `identity show <entry> --expect-identity <canonical-id>` to require matching
public identity metadata. The ID must be exactly 32 lowercase hexadecimal
characters. This checks hash consistency, not key possession or runtime binding.

`--store` is required for identity commands. Its directory must contain the
[public catalog schema](../../docs/catalog-schema.md). Reads never initialize it.
`capabilities` works without a store and advertises only the implemented operations.
See [file-backed CRUD and recovery](../../docs/file-custody-crud.md) for create,
adopt, update, select, forget, protected credential input, and operation recovery.
Forgetting removes catalog references; custody destruction remains unimplemented.

`backup inspect <file>` parses bounded metadata without credentials or a store.
`backup verify <file> [--expect-identity <id>] --passphrase-stdin` authenticates the
payload and returns the canonical identity, without modifying the artifact.
The current format header is not authenticated, even when the payload is valid.
Managed export, restore, reprotection, inventory, removal, and legacy recovery
migration are implemented. See [backup management](../../docs/backup-management.md)
for the two-role protection input and staging/compatibility protocol.

## Output and failures

Human-readable output is the default. `--output json` returns one schema-v1
envelope containing command, target, optional operation ID, state, effects, result,
and structured error. Reads report catalog unchanged and custody not accessed.
Mutations report typed custody/catalog effects and their durable operation ID.
Completed retries set `replayed: true`; receipts are not current custody evidence.
Parser failures use command `usage` and omit the unparsed target.
Parser and catalog errors do not echo raw parser input or file contents.

Public reads never prompt. Create/adopt accept `--passphrase-stdin` from a
non-terminal protected stream. `--non-interactive` never opens a prompt.
Human errors use stderr. JSON errors use the stdout envelope, without ANSI output.
`--help` and `--version` return normal text even when JSON output was requested.
Output failure without an operation ID exits 8; with an operation ID it exits 9. A broken output
stream may have no complete envelope and does not prove that a mutation failed.

| Exit | Current codes |
|---|---|
| 0 | Successful read, mutation, or completed request replay |
| 2 | `invalid_arguments`, `store_required`, `invalid_catalog`, `catalog_too_large`, `invalid_request`, `invalid_backup` |
| 3 | `catalog_uninitialized`, `entry_not_found`, `operation_not_found` |
| 4 | `catalog_unavailable`, `unsupported_schema`, `identity_unavailable`, `custody_unavailable`, `unsupported_operation`, `unsafe_storage` |
| 5 | Authentication required/failed before admission |
| 6 | Ambiguous name, identity mismatch, revision/destination/request conflict, busy store, or changed directory binding |
| 8 | Output failure or an unclassified service failure |
| 9 | Incomplete admitted operation, required reconciliation, or mutation-output failure; inspect effects and operation ID |

The full proposed [CRUD lifecycle](../../docs/cli-lifecycle.md) reserves the
remaining operation and exit categories. Consumers should use structured codes
rather than parse human messages. Output fields can grow additively within schema
v1; an unknown result state must not be treated as success.

## Checks

```sh
cargo test --locked -p styrene-identity-lifecycle -p styrene-identity-cli
cargo clippy --locked -p styrene-identity-lifecycle -p styrene-identity-cli --all-targets -- -D warnings
```

ID01–ID08 and operation recovery cases use disposable stores and bounded process
deadlines. They cover missing/empty/bad/newer catalogs, identity mismatch, unknown
and ambiguous selection, repeated process reads, non-mutation, JSON errors, and
human output. Mutation cases supply disposable protection through a pipe and test
restart/retry and cross-process locking. Native UI and hardware acceptance are separate.
