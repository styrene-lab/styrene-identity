# Managed backups and recovery migration

## Supported operations

The CLI and shared `mutations::artifacts` service implement encrypted backup export,
inspection, verification, reprotection, restore, inventory, forgetting, deletion,
and legacy recovery migration. All writers use the same store lock as catalog CRUD.
They do not start Mesh, change a running session, or revoke remote trust.

| Command | Contract |
|---|---|
| `backup export <entry> --output-file <path> --expect-identity <id>` | Authenticate selected file custody, re-encrypt its root with separate backup protection, and install exclusively |
| `backup reprotect <input> --output-file <new-path> --expect-identity <id>` | Authenticate input and create a new artifact; preserve the original |
| `backup restore <input> --destination <path> --name <name> --expect-identity <id>` | Reprotect for destination custody, install without overwrite, and register a catalog entry |
| `backup inspect <input>` | Unauthenticated bounded format/digest inspection; no store required |
| `backup verify <input> [--expect-identity <id>]` | Authenticate payload and derive canonical identity; no store required |
| `backup list [--identity <id>]` | List active managed references, not a claim of current recoverability |
| `backup show <artifact>` | Administrative location, recorded digest, and observed matching/missing/changed/retired status |
| `backup forget <artifact> --expect-digest <sha256>` | Retire the managed reference; leave artifact bytes intact |
| `backup delete <artifact> --expect-digest <sha256>` | Remove only the recorded file object and digest; preserve replacements and custody |
| `backup verify-recovery <operation> [--expect-identity <id>]` | Authenticate retained recovery bytes from an old catalog journal |
| `backup migrate-recovery <operation> --output-file <path> --expect-identity <id>` | Export verified recovery under new protection, then retire the old recovery copy/intent |

Writers require `--request-id`: 1–32 ASCII letters, digits, hyphens, or underscores.
Artifact operation IDs are `backup-op-<request-id>`; created artifact IDs are
`backup-<request-id>`. Request IDs cannot collide with catalog-operation request IDs.
Restore reserves the internal catalog child ID `restore-<request-id>`.

`operation show` and `operation reconcile` route both catalog and artifact IDs.
A completed replay returns the original receipt with `replayed: true`; it does not
recreate a deleted artifact or claim that current credentials decrypt it.

## Protection roles

Inspection needs no credential. Verification and recovery verification use the
existing single-line `--passphrase-stdin` interface.

Export, reprotection, restore, and migration use `--protection-stdin`: exactly two
lines from non-terminal stdin, followed by EOF. The first is source protection;
the second is destination protection. Each is bounded to 4096 bytes and is held in
a zeroizing buffer. Neither role is silently reused for the other.

```text
source protection
destination protection
```

For export, source means the custody file and destination means the new backup.
For reprotection/migration, source means the old artifact/recovery record. For
restore, destination means the resulting custody file. Source input can be empty
when recovery no longer needs the source; destination protection is still required
to authenticate staged/installed output before completion. A new request requires
both roles. No credential is persisted in a ledger.

Use the original destination protection when resuming complete staged ciphertext.
Changing a password on a completed retry does not change an artifact. Request a
new reprotection operation with a new request ID instead.

## Storage protocol

```text
operations/.artifact-store-v3.json   forward-writer compatibility guard
artifacts/backup-op-<id>.json        bounded artifact ledger, schema v1
```

The guard is installed before the first artifact intent. Older catalog-only clients
reject its unsupported journal version before writing, rather than ignore pending
artifact work. Public catalog reads remain schema v1. Migrated catalog recovery
records use journal v3; ordinary v1/v2 records remain readable.

A write records the normalized request, bound destination directory, expected
identity, and a reserved staging name before creating ciphertext on disk. The
empty staging file's inode is durably recorded before its first ciphertext write.
Recovery never adopts nonempty unproven staging. A partial owned stage is retained
while a new owned generation is prepared from reauthenticated source material.
At most eight generations are retained per operation.

Before publication, staged ciphertext must authenticate to the expected identity.
Publication is a no-replace rename. The ledger can recognize a rename completed
before receipt persistence. Recovery rechecks directory identity, inode, digest,
and payload identity. Source disappearance does not prevent completion from valid
staged ciphertext when destination protection is available.

Restore registers custody through a recorded catalog child operation under the
same lock. Catalog registration and file installation have distinct outcomes and
can be resumed without replacing the restored root. A same-root existing target
must authenticate with destination protection. Restore cannot use its input as
its destination or silently add another custody attachment for an already registered
identity at a different path; use a new store or an explicit catalog/custody workflow.

## Removal and ownership

Managed artifacts cannot be adopted as live custody in the same store until their
backup reference is forgotten. Managed deletion rejects local catalog custody
references. The service cannot enumerate consumers in other stores or applications.

Deletion binds the artifact ID, original inode, and expected digest. It first moves
the target to a journal-owned quarantine name, checks the moved object, records
that phase, and only then unlinks it. A raced-in unrelated object is restored if
the original name is free; otherwise it is retained for reconciliation. Existing
replacement files are never overwritten or deleted. An already absent registered
location is reported separately from an unlink performed by this operation.

Staging cleanup occurs only while the verified destination remains present. File
removal is not secure erasure and does not revoke an identity or invalidate copies
outside this managed store. Ledgers/tombstones are retained for idempotency; automatic
pruning and independently authenticated whole-store history remain unimplemented.

## Legacy migration

First run `backup verify-recovery` with the old protection. Compare its identity
with independently retained evidence where available. Then run
`backup migrate-recovery` with explicit expected identity, new output path, and both
protection roles. The new artifact must be installed and authenticated before old
encrypted recovery bytes are removed from the journal.

An already committed source operation becomes a compact v3 completed receipt.
An uncommitted legacy intent becomes `superseded`, linked through
`recovery_migrated_to`; its original custody destination is not written. It cannot
be silently replayed. The exported backup can subsequently be restored through an
explicit destination/store workflow. Existing custody and external copies are not
erased by journal migration.

## Evidence and limits

Tests exercise separate protection roles, legacy recovery, partial staging,
source disappearance, restored catalog registration, conflicting destinations,
forged ciphertext digests, same-root retry, and raced quarantine replacement.
The ledger is trusted local application state, not a signed anti-rollback authority.
Platform/device acceptance remains separate from these software tests.
