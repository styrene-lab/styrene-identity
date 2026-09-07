# File-backed catalog CRUD and recovery

## Implemented scope

`styrene-identity-lifecycle::mutations` implements file-backed create/adopt and
catalog update/select/forget. It is behind the optional `file-custody` feature,
enabled by the development CLI. The default backend remains public-read-only.
Mutations currently target Linux/Android and Apple platforms, and require a filesystem supporting the used
lock, exclusive-install, atomic-rename, and directory-sync operations. Device/token
custody and Mesh activation are not part of this implementation.

The CLI and future UI call the same blocking backend functions. Async hosts must
use a blocking worker and retain operation observation outside a mounted view.
The CLI runs synchronously; it does not spawn a daemon or background executor.

## Commands

Use `cargo run --locked -p styrene-identity-cli --` in place of `idctl` when running
from this checkout. Each mutation requires a caller-chosen `--request-id`:
1–48 ASCII letters, digits, hyphens, or underscores. Reuse it only to retry the
same request in the same store.

```sh
idctl --store /path/to/store --output json identity create \
  --name Operator --custody file --destination /path/to/identity.key \
  --request-id create-operator --passphrase-stdin < /path/to/protected-input

idctl --store /path/to/store identity update identity-create-operator \
  --name "Renamed operator" --if-revision 1 --request-id rename-operator

idctl --store /path/to/store identity select identity-create-operator \
  --if-revision 2 --request-id select-operator

idctl --store /path/to/store identity forget identity-create-operator \
  --if-revision 2 --request-id forget-operator
```

These revision numbers describe the sequence above on a new store. Read actual
revisions from results rather than hard-code them in automation.

| Command | Revision and identity rules | Effect |
|---|---|---|
| `identity create` | New identity; destination must be unused | Exclusive encrypted STID creation and catalog registration |
| `identity adopt --name <name> --custody <path> --expect-identity <id>` | Authenticate existing custody and verify canonical ID | New catalog reference; custody bytes unchanged |
| `identity update <entry-id> --name <name>` or `--clear-name` | `--if-revision` is the entry revision | Name only; preserves root, public identity, and custody references |
| `identity select <entry-id>` | `--if-revision` is the catalog revision | Local preferred entry only; no unlock or Mesh session change |
| `identity forget <entry-id>` | `--if-revision` is the entry revision | Removes entry and matching preference; preserves custody and recovery records |

Mutation targets must be exact entry IDs. Reads still accept unambiguous display
names. Creation/adoption allocate `identity-<request-id>` and
`custody-<request-id>`; operation ID is `op-<request-id>`. New entries start at
revision 1. Every committed catalog mutation increments the catalog revision.
Updates also increment the entry revision. Revision overflow fails with conflict.

Creation/adoption can initialize the store directory. Its parent and the custody
destination's parent must already exist. Custody paths are normalized through
their existing parent and must be outside the store. The parent must be owned by
the effective user and not writable by group/other principals. V2 recovery binds
its device/inode identity. Custody symlinks, directories,
oversized files, and malformed encrypted files are rejected. Existing 92-byte
legacy encrypted files remain adoptable through the library reader.

A new request to adopt an already registered canonical identity returns conflict;
it does not create duplicate entries. Retrying the original request is idempotent.
After forgetting the entry, a new adoption can register the retained custody again.
Additional same-identity attachments belong to the later custody-attachment API.

## Credential input

`--passphrase-stdin` reads one protected input stream from non-terminal stdin to
EOF, with a 4096-byte passphrase limit. It removes one final LF or CRLF and rejects
empty, oversized, or multiline input. The caller must close the input stream;
this explicit stream has a size bound, not a credential-source timeout.

No passphrase value is accepted in argv or environment variables. Terminal stdin
is rejected rather than read with echo enabled. Native/terminal prompt adapters
remain future work. `--non-interactive` never opens a prompt. The CLI owns a
zeroizing credential buffer; the backend borrows it for the operation.

Without credentials, a new create/adopt fails before initializing the store. A
completed retry needs no credentials. Pending recovery can require fresh protected
input; no credential is saved for a later process.

## Journal and commit ordering

The store contains:

```text
catalog.json                    public catalog schema v1
.mutation-lock                  permanent OS-lock file; do not unlink while in use
operations/op-<request-id>.json  private recovery record, journal schema v2 for new operations
```

New store/operation directories use mode 0700; new catalog, lock, journal, staging,
and custody files use 0600. Existing directory and adopted-custody permissions are
not rewritten. Mutation stores must also be private to the effective user. Unsafe
existing store/authority permissions are rejected. Private
operation directories and lock/journal files must exclude group/other access;
lock, journal, and catalog authority files must have one hard link.
The backend takes a nonblocking OS file lock for the whole mutation. A competing
writer receives `store_busy`; the OS releases the lock on process exit.
Cooperating CLI/UI writers must use this service. The store is trusted local
application storage, not a sandbox against malicious filesystem replacement.

1. Validate the request, current catalog, expected revisions, and credentials.
2. Build the intended catalog and result. For creation, generate one root and
   encrypt it using the existing STID format.
3. Persist and sync a `prepared` journal before installing custody. The journal
   binds the normalized request, before-catalog digest, intended catalog, public
   result, and custody digest. Creation also retains the encrypted STID bytes.
4. Install new custody exclusively, or reauthenticate adopted custody. An existing
   create destination is accepted during recovery only if its bytes exactly match
   the journal's encrypted creation artifact.
5. Recheck catalog/custody evidence, atomically install the catalog, and sync it.
6. Persist a compact `completed` receipt containing the original result. V2 drops
   the encrypted creation bytes and complete catalog snapshot at this point.

Prepared journals contain no plaintext root or passphrase; their encrypted creation
artifact is recovery material. Completed v2 receipts no longer retain that copy.
Legacy v1 receipts can still contain it and are not silently stripped of potentially
irreplaceable recovery data. Forgetting preserves those legacy copies. Future
reprotection/destruction must account for them and for external backups. Unlinking
or replacing a file is not secure erasure of filesystem history.

Temporary writes use exclusive descriptor-relative staging, file sync, and
same-directory installation. No-replace rename avoids a two-hard-link installation
window that would conflict with single-link journal validation after a crash.
Reads use no-follow/nonblocking opens and size limits.
Catalog/journal replacement is atomic under the store lock. Catalog publication
and custody creation are separate commits; the journal records the recovery path.
Before/after catalog digests detect unexpected state; recovery also recomputes the
entire permitted transition from the before-state and declared request. A structurally
valid after-snapshot is not accepted as authority for unrelated catalog changes.

## Observe, retry, and reconcile

```sh
idctl --store /path/to/store --output json operation show op-create-operator
idctl --store /path/to/store --output json operation reconcile op-create-operator \
  --passphrase-stdin < /path/to/protected-input
```

`operation show` is a read: its outer envelope can be completed while the recorded
operation inside remains `prepared`. Prepared effects are conservative: catalog
and new-custody effects can be unknown. The query does not decrypt custody or
return private locators or encrypted recovery bytes. It reports `journal_version`
and `retains_recovery_material` so old retained copies remain visible.

Retry the original command with the same request ID, or run `operation reconcile`.
Reconciliation uses the retained request and never generates another root for an
admitted create. After a catalog commit, it can finalize without unlocking custody,
but v2 compaction checks that the matching ciphertext copy still exists at the bound
location. Missing custody leaves recovery material retained. A completed retry returns the historical result; it does
not recreate a forgotten entry or assert that custody still exists today. Completed
retries explicitly return `replayed: true`; false is not proof of current custody.

V1 pending custody writes have no directory identity binding and cannot resume
automatically. Already-committed catalog state can still finalize a v1 receipt
without custody access. Otherwise preserve the legacy record and encrypted material
for an explicit migration. Copying a pending store to another filesystem is not
a portable identity-restore transaction. See the
[adversarial review](adversarial-review-2026-09-06.md) for the threat model and limits.

Changing any normalized request field while reusing a request ID returns
`request_conflict`. A prepared operation blocks unrelated new mutations until it
is reconciled. Reads remain available. Each request ID yields a known operation
ID, so a caller can inspect it even if the original process produced no output.

| Outcome | CLI behavior |
|---|---|
| Invalid input/revision, wrong identity, occupied destination before admission | Failure with no catalog/custody mutation; structural store files may exist after validation progressed |
| Credential input missing/incorrect before admission | Exit 5; no identity created or adopted |
| Prepared operation interrupted or failed | Exit 9 with operation ID and `needs_reconciliation` when an envelope can be emitted |
| Custody installed, catalog failed | Retain custody and journal; recovery installs the originally intended catalog |
| Catalog installed, completion record interrupted | Recovery checks the custody copy before v2 compaction; no catalog replay or unlock is needed when that copy exists |
| Unexpected destination/catalog during recovery | Conflict remains pending; preserve current bytes rather than overwrite |
| Completed request replay | Return original result without mutation, even after later catalog changes |

If credentials are lost or external state cannot be reconciled, the journal remains
pending. This slice has no cancel/abandon/force operation or automatic deletion.
Do not edit or remove journals to bypass recovery. Diagnostic investigation must
preserve custody and the recorded request before defining a repair.

The store retains up to 1024 journal records, including completed ones, and then
refuses new requests. There is no automatic pruning; retention/export policy must
be implemented before larger sustained usage. Each record has a bounded size.

Ctrl-C/process termination can prevent an envelope from being written. It does
not prove rollback. An output failure after a mutation result returns exit 9;
use the known request-derived operation ID to inspect the outcome. There is no
detached executor or automatic restart replay.

## Evidence boundary

Tests cover catalog CRUD, stale revisions, same-request retry, changed-request
conflict, legacy adoption, wrong credentials/identity, exclusive destinations,
cross-process locking, and recovery at each durable boundary. They also exercise
a filesystem failure after custody creation and inconsistent journal receipts.

These are disposable local software tests. They do not establish physical power-loss
behavior, network-filesystem guarantees, hardware custody, Mesh activation, or UI
subscription lifecycle. The full backup, provider, and UI contracts remain separate.
