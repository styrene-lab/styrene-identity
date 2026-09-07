# Identity CLI CRUD lifecycle

## Status and ownership

This document combines implemented file-backed catalog CRUD with proposed advanced
operations. ID01–ID08, ID30–ID36, and operation show/reconcile are implemented in the
development `idctl` binary and shared lifecycle backend;
see [CLI usage](../apps/cli/README.md) and the [catalog schema](catalog-schema.md).
The executable name remains subject to release review and does not rename the
Mesh `styrene` CLI. See [file-backed CRUD](file-custody-crud.md) for exact supported
commands, revision semantics, and recovery limits. Other mutations remain proposed.

The CLI and Identity UI are sibling clients of `styrene-identity-lifecycle`.
The backend owns catalog validation, identity binding, custody operations,
concurrency, and recovery. The CLI owns argument parsing, protected input,
rendering, and process exit. Neither frontend shells out to the other.

The coordination CLI matrix assigns advanced Identity lifecycle work to this
repository. Its I01–I06 cases cover Mesh identity reporting, metadata, backup,
restore, provider selection, and advanced lifecycle handoff. The ID-prefixed cases
below are Identity-owned acceptance IDs. They complement those Mesh cases; they
do not implement Mesh profiles, session activation, or remote Fleet authorization.

## Objects and CRUD meaning

| Object | Stable identifier | Create | Read | Update | Delete |
|---|---|---|---|---|---|
| Identity catalog entry | Local `entry_id`; separate canonical `IdentityId` | Generate or adopt existing custody | List/show/evidence | Local metadata and preferred provider | Forget catalog reference |
| Custody attachment | Local `custody_id` bound to an entry | Create/enroll or attach existing custody | Presence/capabilities/verified binding | Reprotect or add replacement custody | Detach reference or explicitly destroy supported custody |
| Derived-key reference | Local `key_ref_id` plus purpose/label/epoch | Register a deterministic derivation descriptor | Public key/fingerprint/provenance | Metadata or a new version with explicit derivation input | Forget reference or delete a selected export |
| Encrypted backup artifact | Backend artifact ID plus content digest | Authenticated export | Format inspection and authenticated verification | Create a new artifact under new protection | Delete explicitly managed local artifact |
| Lifecycle plan/record | Plan ID/revision or immutable record digest | Plan and issue supported actions | Inspect/verify/status | Revise uncommitted plan; issue successor record | Discard draft plan; committed records remain immutable |
| Operation | Backend operation ID | Admit a mutation | Show/wait/reconcile | Cancel where supported; advance phases | Retention policy, not an arbitrary delete command |

An identity's canonical ID and root are not editable metadata. Replacing the root
creates a new identity. Updating a derivation label or epoch creates a new key
version. Deleting a deterministic-key reference or export cannot prevent rederivation.
Local catalog retirement does not revoke remote authorization.

Catalog entry, custody presence, authentication, lifecycle status, and runtime
binding are separate facts. Locked custody is not an absent identity. An empty
successful inventory is valid only when the catalog was actually read.

## Common invocation and targeting

Proposed common options:

```text
idctl --store <catalog-directory> --output human|json --non-interactive <command>
```

`--store` selects local application storage, not a Mesh profile or daemon endpoint.
Define platform defaults before implementation; tests always pass an explicit
temporary directory. Do not silently fall back to the current directory when
platform storage cannot be resolved. Public reads do not create the catalog.

Mutations target an explicit entry/custody/key/artifact ID. Human names are lookup
hints: ambiguous names fail before execution. Backend-issued overview selection
tokens are ephemeral and are not persistent catalog identifiers.

Existing-identity cryptographic or custody operations carry `--expect-identity`.
Catalog metadata edits carry `--if-revision`; omitted fields retain their values,
and clearing uses an explicit `--clear <field>`. Before admission, resolve and bind
IDs and revisions under the backend's concurrency rules. Selecting a local default
does not retarget an already admitted operation or alter an active Mesh session.

Creation has no pre-existing expected ID. Return the generated canonical ID and
the created custody/entry IDs. Restore with an unknown ID uses authenticated
`backup verify` first; do not treat unauthenticated backup metadata as identity proof.

## Command and acceptance matrix

Mutation commands additionally accept `--request-id <idempotency-key>` where the
operation supports durable deduplication. The CLI prints the backend operation ID
once admitted. `--request-id` identifies a retry; `--if-revision` protects against
concurrent edits. Neither substitutes for `--expect-identity`.

ID01–ID08, ID30–ID36, and the show/reconcile subset of ID44–ID45 have backend and CLI tests.
Other rows, including wait/cancel, remain
**proposed / CLI not implemented / acceptance not run**. Library primitives noted
below are implementation inputs, not proof of command acceptance.
Use suffixes such as `.success`, `.conflict`, `.cancelled`, and `.restart` in tests.

### Catalog and identity CRUD

| Case | Proposed command | Backend operation and required result |
|---|---|---|
| ID01 | `capabilities` | Enumerate supported backend operations and unavailable reasons; no implicit provider unlock |
| ID02 | `identity list` | Read catalog; distinguish empty, missing/uninitialized, inaccessible, and incompatible schema |
| ID03 | `identity show <entry>` | Public overview, canonical ID evidence, custody references, metadata revision; missing ID stays absent |
| ID04 | `identity create --name <name> --custody file --destination <path>` | Generate fresh root, exclusive custody creation, catalog registration; no Mesh activation |
| ID05 | `identity adopt --custody <locator> --expect-identity <id>` | Authenticate/bind existing custody and register it; no root regeneration or custody rewrite |
| ID06 | `identity update <entry> --name <name> --if-revision <n>` | Patch local metadata, preserve canonical ID/root/custody; explicit clear supported for optional fields |
| ID07 | `identity select <entry> --if-revision <n>` | Set local preferred entry; persist preference without unlocking or switching Mesh sessions |
| ID08 | `identity forget <entry> --if-revision <n>` | Remove local reference only; preserve custody, backups, exports, and remote trust; clear matching preference |
| ID09 | `identity verify <entry> --expect-identity <id>` | Explicit authenticated binding/possession check when supported; report what was verified, separately from overview hash consistency |

`identity update` manages local Identity metadata. Publishing a Mesh display name
is a separate consumer operation (coordination I02), not a side effect.

### Custody CRUD and interaction

| Case | Proposed command | Backend operation and required result |
|---|---|---|
| ID10 | `custody list <entry>` / `custody show <custody>` | Presence, supported interaction/export operations, declared root exposure, last checked binding and freshness |
| ID11 | `custody attach <entry> --existing <locator> --expect-identity <id>` | Authenticate existing custody and bind it to this entry; wrong identity leaves references unchanged |
| ID12 | `custody enroll <entry> --backend <kind> --expect-identity <id>` | Provision another backend only if it supports the same root/identity contract; otherwise unsupported, never silently create another identity |
| ID13 | `custody reprotect <custody> --expect-identity <id> --if-revision <n>` | Change protection without changing root; authenticate old/new protection and commit atomically with recovery evidence |
| ID14 | `custody prefer <entry> --custody <id> --if-revision <n>` | Change explicit local selection policy; validate same-identity binding, no silent lower-custody fallback |
| ID15 | `custody detach <custody> --if-revision <n>` | Remove attachment only; a preferred attachment requires an explicit replacement or leaves no provider selected |
| ID16 | `custody destroy <custody> --expect-identity <id>` | Explicitly delete supported stored custody with bound target and durable outcome; preserve unrelated stores and backups |

File signer and vault creation/restore are available library primitives. A general
reprotection transaction, catalog, and cross-adapter enrollment workflow are not.
YubiKey FIDO2 root derivation does not imply arbitrary same-root enrollment.
Device capabilities must determine whether ID12/ID13/ID16 are supported.

Avoid a generic `unlock` command that claims a durable unlocked session after its
process exits. Authentication is operation-scoped initially. A future persistent
unlock lease needs expiry, ownership, revocation, and restart semantics first.
App unlock, provider authentication, user presence, and signing success stay distinct.

### Keys, signatures, and public records

| Case | Proposed command | Backend operation and required result |
|---|---|---|
| ID20 | `key derive <entry> --purpose <p> [--label <l>] [--epoch <e>] --expect-identity <id>` | Validate exact versioned derivation descriptor; register/reference deterministic public result |
| ID21 | `key list <entry>` / `key show <key-ref>` | Descriptor, public fingerprint, binding evidence, export references; reading does not implicitly derive private material |
| ID22 | `key update <key-ref> --name <name> --if-revision <n>` | Edit local metadata only; cannot replace purpose, label, epoch, or key bytes in place |
| ID23 | `key export <key-ref> --format <format> --output-file <path> [--private] --expect-identity <id>` | Public export by default; private export requires explicit supported capability and exclusive protected destination |
| ID24 | `key forget <key-ref> --if-revision <n>` / `export delete <artifact>` | Remove a reference or one managed export; neither revokes the key nor prevents rederivation |
| ID25 | `sign <key-ref> --input <file> --domain <profile> --output-file <path> --expect-identity <id>` | Profile-specific signing with bounded input and explicit framing; no claim of arbitrary remote authorization |
| ID26 | `signature verify --input <file> --signature <file> --public-key <file> --domain <profile>` | Verify signature/profile and return typed invalid-signature failure; caller trust policy stays separate |
| ID27 | `record issue <entry> --kind <kind> --claims <file> --expect-identity <id>` | Issue supported repository binding/certificate record through reviewed policy; unsupported record issuers fail explicitly |
| ID28 | `record inspect <file>` / `record verify <file>` | Separate parsing, cryptographic validity, validity interval, issuer attribution, and caller trust decisions |

Purpose profiles define valid labels, epochs, encodings, and signing frames before
commands ship. Existing repository bindings and X.509 issuance can seed adapters;
the presence of generic runtime-record structs does not implement their issuers.
Committed signed records are immutable. Replacement is a new record, not edit-in-place.

### Backup and recovery CRUD

| Case | Proposed command | Backend operation and required result |
|---|---|---|
| ID30 | `backup export <entry> --output-file <path> --expect-identity <id>` | Authenticated encrypted export to unused destination; preserve source custody; record digest and protection format |
| ID31 | `backup inspect <file>` | Bounded format/metadata parse; explicitly unauthenticated, no claimed canonical identity |
| ID32 | `backup verify <file>` | Authenticate using protected input and return canonical identity evidence without plaintext output |
| ID33 | `backup restore <file> --destination <locator> --expect-identity <id>` | Authenticate first, restore without overwrite, then register catalog reference; same-root repeat is defined |
| ID34 | `backup reprotect <file> --output-file <new-file>` | Authenticate old protection, write a new encrypted artifact with new protection; retain original until explicit deletion |
| ID35 | `backup list <entry>` / `backup show <artifact>` | Inventory managed references/digests and existence; a stored reference alone is not proof of recoverability |
| ID36 | `backup forget <artifact>` / `backup delete <artifact>` | Forget reference or delete one verified managed artifact; no recursive deletion, no custody change |

Artifact IDs resolve to backend-managed destinations and expected digests. If a
path now contains different bytes, deletion fails with conflict. External backups
can be inspected/restored without being adopted as managed deletion targets.
Portable Identity backup does not include Mesh profiles, remote trust state, or
platform credential enrollment. Artifact reprotection does not rotate the root.

### Lifecycle planning and operation observation

| Case | Proposed command | Backend operation and required result |
|---|---|---|
| ID40 | `lifecycle plan <entry> --action rotate-key|replace-identity|retire --expect-identity <id>` | Produce a reviewable plan with affected local references, new derivation inputs, supported steps, and unresolved trust updates |
| ID41 | `lifecycle show <plan>` / `lifecycle update <plan> --if-revision <n>` | Inspect or revise an uncommitted plan; invalidate obsolete approvals/input digest |
| ID42 | `lifecycle apply <plan> --if-revision <n> --expect-identity <id>` | Admit only implemented steps with matching plan digest/revisions; report local completion separately from pending consumer trust work |
| ID43 | `lifecycle discard <plan>` | Delete a draft only; committed records and admitted operations cannot be erased through draft deletion |
| ID44 | `operation show <op>` / `operation wait <op> --timeout <duration>` | Observe phases and effects; bounded wait; unavailable/expired record is not success |
| ID45 | `operation cancel <op>` / `operation reconcile <op>` | Cancel only if supported for the current phase; reconcile unknown effects without replaying mutations blindly |

Retirement records local intent; it does not destroy custody or revoke remote
trust. A compromise/revocation workflow requires a defined issuer, authorization,
distribution, and consumer acknowledgment contract before it can report completion.
There is no generic `identity revoke` success path without that integration.

## Transaction and recovery rules

Mutations use backend-issued operation IDs and caller-supplied idempotency keys
where supported. An idempotency key is bound to the normalized non-secret request,
target, expected identity, and revisions. Reusing it for a different request is a
conflict. Credentials are not journaled or hashed into retained diagnostic data.

Proposed progression:

```text
admitted -> validating -> awaiting-input -> committing -> completed
                         |                 |
                    cancelled/failed   partial/needs-reconciliation
```

Not every operation needs every phase. Public reads have no durable operation
registry. Local mutations initially run to a terminal result in the invoking
process. Do not offer detached `--submit-only` work until an execution owner and
durable observation contract exist.

The operation record stores target IDs, expected revisions, public artifact
digests, phase, effects, and recovery instructions. Define its schema, retention,
access permissions, and crash-safe writes before mutation implementation. A
retained operation ID does not itself keep work running after process exit.

Acquire the appropriate catalog/custody locks, validate expected revisions, and
record intent before an irreversible effect. Exclusive creation protects custody
and export destinations. Atomic replacement is permitted only for an explicitly
reviewed update such as reprotection, never as a restore shortcut.

| Failure point | Required observable outcome |
|---|---|
| Validation/authentication before commit | No effect; destination unchanged |
| Custody created but catalog write fails | Partial result with custody locator available through protected local recovery; retry registers verified custody, never generates another root |
| Export committed but artifact registration fails | Report retained artifact and digest; reconcile instead of overwrite |
| Custody destroyed but catalog cleanup fails | Report destruction separately; reconcile stale references, never recreate custody |
| Concurrent change | Conflict; preserve other operation's data |
| Connection/process interruption at commit | Unknown effect until reconciliation; no automatic mutation retry |

Deletion is resource-specific. Reject operations against a held local lease or
an admitted conflicting operation. This service cannot detect all external Mesh
users of exported keys: destruction reports local effects, not global inactivity.
Deleting custody cannot promise secure erasure of filesystem snapshots or copies.
Forgetting an already absent reference can return `already_absent` only when the
service can establish the target; an unknown name or inaccessible store is not an
idempotent success.

## Automation, input, and output

All commands support `--output json`. Emit exactly one versioned terminal envelope
on stdout for the normal invocation mode, with diagnostics/progress on stderr.
Reserved streaming output, if added, needs its own explicit mode and schema.
Do not send binary/private artifacts to stdout in JSON mode; use a destination.

Proposed envelope fields:

```json
{
  "schema_version": 1,
  "command": "backup.restore",
  "target": {"entry_id": "local-entry", "expected_identity_id": "canonical-id"},
  "operation_id": "operation-id",
  "state": "completed",
  "effects": {"custody": "already_present", "catalog": "registered"},
  "result": {"identity_id": "canonical-id"},
  "error": null
}
```

Values above are descriptive placeholders, not conformance fixtures. Queries may
have a null operation ID. Public IDs and operation outcomes belong in results;
credentials, roots, private derived bytes, and decrypted backup data do not.
Future schema additions are additive; consumers tolerate unknown fields. Unknown
enum values must not be treated as success. Incompatible changes need a new schema.

Proposed exit categories, to freeze with the first CLI schema:

| Exit | Meaning |
|---|---|
| 0 | Requested read or mutation completed successfully, including a verified idempotent no-op |
| 2 | Usage or input validation failure |
| 3 | Target not found |
| 4 | Unsupported/unavailable capability or incompatible schema |
| 5 | Authentication required, authentication failed, or authorization denied; distinguish structured codes |
| 6 | Revision/destination/identity conflict; distinguish structured codes |
| 7 | Observation timeout; include operation ID and current effect knowledge |
| 8 | Operation failed with known effects |
| 9 | Partial completion or outcome unknown; reconciliation required |
| 130 | Operator interruption/cancellation; include effect state when output can still be emitted |

A terminated process may emit no envelope. Missing output never proves no effect.
Ctrl-C ends observation; it does not prove rollback. For local operations, use the
backend's phase-specific cancellation policy and retain recovery state if commit
was reached. `operation wait` timing out does not cancel the operation.

Secrets are read through protected terminal input or explicitly selected protected
input channels (for example inherited descriptors), never plaintext argv or a
built-in environment-variable provider. Old/new/backup/destination credentials
have distinct roles; do not silently reuse one for another. Bounded input, EOF,
missing TTY, and cancellation have typed outcomes. UI adapters supply the same
backend credential roles through native services.

Destruction commands bind explicit target and expected identity; the concrete verb
expresses destructive intent. A prompt may restate that target interactively, but
there is no blanket `--force` that bypasses conflicts or non-overwrite rules.
Before implementation, freeze each command's exact required intent fields rather
than making automation depend on a yes/no prompt.

## Implementation order and exit criteria

The first end-to-end CRUD walkthrough is:

1. Create a disposable file-backed identity and capture entry ID, custody ID,
   canonical ID, and catalog revision from JSON output.
2. List and show it without unlocking; explicitly verify its binding if needed.
3. Update its display metadata using the returned revision; show the same canonical ID.
4. Select it locally and confirm that no Mesh session was started or switched.
5. Export an encrypted backup, inspect its metadata, and authenticate its contents.
6. Restore to an unused destination expecting the captured canonical ID; retry and
   confirm the defined same-root no-op behavior.
7. Forget the original catalog entry and prove its custody and backup still exist.
8. Adopt that original custody again with the expected canonical ID.
9. Explicitly destroy only the disposable original custody, then reconcile its
   catalog references. Confirm restored custody and backup remain intact.

Each destructive step uses its own bound target; this is an acceptance sequence
for disposable fixtures, not an automatic cleanup recipe for operator identities.

### Implementation slices

1. **Public reads and process contract:** ID01–ID03, JSON/exit/input parsing, catalog
   schema and explicit absence. Adapt the existing overview; no mutation registry.
2. **Complete local identity CRUD:** ID04–ID08 with file custody, metadata revision
   conflicts, preferred selection, forget semantics, operation journal, and recovery.
3. **Portable recovery:** ID30–ID36; wrong protection, tamper, occupied destination,
   same-root retry, catalog failure, reprotection, and artifact deletion conflict.
4. **Custody and key management:** ID09–ID16 and ID20–ID28, capability-gated. File
   reprotection precedes hardware enrollment; device acceptance stays separate.
5. **Lifecycle plans and observation:** ID40–ID45 as needed by implemented mutations;
   preserve external trust-update gaps and verify restart/reconciliation behavior.

Each slice needs backend unit/conformance tests and real CLI subprocess tests
using temporary catalogs and disposable identities. Assert argv-to-request
translation, stdout schema, exit status, actual bytes/effects, restart behavior,
and cleanup. Retain ID case prefixes. UI projection tests reuse the same backend
outcomes; the UI does not invoke the CLI. The first Mesh mutation remains gated
on its CLI/service cases and independent observation after page close/disable.
