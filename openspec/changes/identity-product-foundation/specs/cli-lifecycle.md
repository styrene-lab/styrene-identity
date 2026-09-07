# Identity CLI lifecycle - Delta Spec

## ADDED Requirements

### Requirement: Backup writers own recoverable staging before ciphertext publication

Artifact operations must persist staging intent and file identity before writing
existing-root ciphertext. Source and destination protection are distinct. Recovery
must authenticate the expected output identity and preserve possible recovery copies.

#### Scenario: Ciphertext write interrupted
Given durable staging ownership and partially written ciphertext
When recovery reauthenticates the original source
Then a new owned staging generation is created
And the previous generation remains until verified output is installed

#### Scenario: Source disappears after complete staging
Given staged ciphertext matches its recorded digest and original custody disappears
When recovery receives correct destination protection
Then the expected identity is verified and the artifact is published without source custody

#### Scenario: Checksum fields tampered with another identity
Given staged ciphertext and digest fields were replaced with another root's encrypted bytes
When publication verifies the expected identity
Then identity mismatch is returned before publication

#### Scenario: Unrelated object races deletion
Given another process replaces a managed artifact between checking and quarantine rename
When deletion checks the quarantined inode
Then the unrelated object is restored or retained for reconciliation
And it is not unlinked

#### Scenario: Legacy recovery migration
Given a pending legacy catalog operation retains an authenticated root
When an explicit migration installs and verifies a newly protected backup
Then the old journal records the replacement artifact and removes its retained ciphertext
And an uncommitted source intent becomes superseded without writing its original destination

#### Scenario: Older writer encounters artifact-capable store
Given a store contains the version-three artifact compatibility guard
When a catalog-only older client scans operations before mutation
Then its unsupported journal-version handling prevents the write

### Requirement: Public catalog reads are explicit bounded and non-mutating

The CLI must provide capabilities, list, and show through the shared backend.
Capabilities must not need a store. Identity reads require explicit storage and
must distinguish missing, empty, invalid, unavailable, and unsupported catalogs.
They must not create storage, unlock custody, or start Mesh.

#### Scenario: ID01 capabilities without a store
Given no initialized Identity catalog or running Mesh daemon
When capabilities is requested
Then the CLI lists implemented operations and reports mutations unsupported
And it does not create a store or acquire custody

#### Scenario: ID02 initialized empty versus missing
Given one valid empty catalog and one missing catalog
When inventory is requested from each
Then the valid catalog returns an empty completed inventory
And the missing catalog returns catalog_uninitialized without creating files

#### Scenario: ID02 unsupported or oversized catalog
Given a catalog with a newer schema or more than the maximum allowed bytes
When inventory is requested
Then the CLI reports unsupported_schema or catalog_too_large respectively
And the catalog is unchanged

#### Scenario: ID03 checked public snapshot
Given a catalog contains a canonical ID and matching public key but no observed custody state
When identity show is requested with that expected ID
Then the result labels the public binding hash_consistent
And provider availability and root exposure remain unknown

#### Scenario: ID03 invalid binding anywhere in inventory
Given one catalog entry claims an ID inconsistent with its public key
When inventory is requested
Then the entire read fails with invalid_catalog
And neither partial success nor key-possession evidence is returned

#### Scenario: ID03 ambiguous name
Given two entries share a display name but have distinct entry IDs
When identity show uses that display name
Then the CLI returns ambiguous_name
And exact entry IDs remain independently selectable

### Requirement: Identity CRUD preserves canonical identity and owned resources

Catalog creation/adoption, reads, revision-checked metadata updates, local selection,
and forgetting must have distinct backend operations. Root replacement creates a
new identity. Forgetting removes references without destroying custody or trust.

#### Scenario: ID04 create collides with custody
Given the selected destination already contains custody
When identity creation is requested
Then creation fails with conflict without replacing custody
And no catalog entry claims a newly created identity

#### Scenario: ID06 metadata revision conflict
Given a catalog entry was updated after the caller read its revision
When a stale metadata update is submitted
Then it fails with revision conflict
And existing metadata, root, and canonical Identity ID remain unchanged

#### Scenario: ID07 local preference selection
Given two registered identities and a running Mesh session
When the CLI changes its local preferred identity
Then the catalog preference changes without changing the Mesh session
And the operation does not unlock custody

#### Scenario: ID08 forget identity
Given a registered identity with custody and a managed backup
When its catalog entry is forgotten
Then the local entry and matching preference are removed
And custody, backup bytes, and remote trust remain unchanged

### Requirement: Custody changes are explicitly identity-bound

Attachment, enrollment, reprotection, and destruction must name the expected
canonical identity and respect backend capabilities. Detachment and destruction
must remain separate operations.

#### Scenario: ID11 attach another identity
Given existing custody resolves to identity B
When a caller attaches it to an entry expecting identity A
Then the operation fails with identity mismatch
And custody and catalog attachments remain unchanged

#### Scenario: ID12 unsupported enrollment
Given a provider cannot enroll the existing root
When same-identity enrollment is requested
Then the result identifies unsupported capability
And no replacement identity is generated

#### Scenario: ID13 reprotection preserves identity
Given authenticated file custody and supported reprotection
When its protection is updated
Then the committed custody opens with the new protection and has the same canonical identity
And interruption produces a recoverable outcome instead of unreported custody loss

#### Scenario: ID16 destruction has partial cleanup
Given custody destruction succeeds but catalog cleanup fails
When the backend returns the operation result
Then the CLI reports partial completion and the destroyed-custody effect
And reconciliation does not regenerate the root

### Requirement: Key and record CRUD preserves immutable cryptographic inputs

Key metadata edits and reference/export removal must not reinterpret derivation
inputs or claim revocation. Signed records are immutable; revisions create new
records. Unsupported issuance/trust workflows must fail explicitly.

#### Scenario: ID22 change derivation input through metadata
Given a registered key reference for a fixed purpose and epoch
When a metadata update attempts to replace the epoch
Then the backend rejects that update
And a new key version requires an explicit derivation or lifecycle operation

#### Scenario: ID24 remove exported key
Given a managed key export and retained root custody
When the export is deleted
Then only the selected export is removed
And the result does not claim that rederivation or remote use is prevented

### Requirement: Backup lifecycle distinguishes authentication and artifact ownership

Inspection must not claim authentication. Restore must bind an authenticated
identity and preserve non-overwrite behavior. Reprotection writes a new artifact.
Deletion must check managed artifact identity before removing bytes.

#### Scenario: ID31 inspect unauthenticated backup
Given a structurally valid backup that has not been decrypted
When backup inspection runs
Then its output labels metadata as unauthenticated
And it does not report a verified canonical identity

#### Scenario: ID33 restore succeeds before catalog failure
Given authenticated custody restoration succeeds but catalog registration fails
When the CLI reports restore
Then it returns partial completion with retained-custody effects and recovery instructions
And retry reconciles that custody without generating a new identity

#### Scenario: ID34 artifact reprotection
Given an authenticated backup and an unused output destination
When backup reprotection completes
Then the new artifact recovers the same identity using new protection
And the source artifact is preserved

#### Scenario: ID36 changed backup deletion target
Given a managed artifact path now contains bytes with a different digest
When backup deletion is requested
Then deletion fails with conflict
And the current bytes remain unchanged

### Requirement: CLI outcomes support bounded automation and safe retries

The CLI must render versioned JSON, documented exits, explicit effect states, and
typed failures. Noninteractive credential input is bounded. Mutation retries use
operation-specific idempotency and reconciliation rather than blind replay.

#### Scenario: Missing noninteractive credentials
Given a mutation needs credentials and no protected input source is available
When it runs noninteractively
Then it terminates with authentication-required output and a nonzero documented exit
And it does not hang or mutate custody

#### Scenario: Idempotency key reused for another request
Given an idempotency key was admitted for one target and normalized request
When another request reuses it with a different target
Then the backend rejects it with conflict
And it does not execute the new mutation

#### Scenario: ID44 observation timeout
Given an operation has not reached a terminal outcome
When a bounded operation wait expires
Then the CLI reports timeout with operation ID and known effect state
And it does not claim backend cancellation or success

#### Scenario: ID42 local retirement with remote work pending
Given a retirement plan includes local intent and unsupported remote trust updates
When supported local steps complete
Then the result identifies local completion and unresolved remote work separately
And it does not claim global revocation

### Requirement: File catalog mutations preserve a recoverable identity after admission

Create/adopt and catalog edits must serialize cooperating writers, bind request
IDs to normalized inputs, and persist recoverable intent before custody commit.
Creation recovery must retain the originally admitted encrypted identity. Completed
request replay must return a historical result without repeating side effects.

#### Scenario: ID04 interrupted before custody installation
Given a create operation has durably recorded its encrypted identity but not installed custody
When another process reconciles the operation with the correct protection
Then it installs the originally recorded encrypted identity without generating another root
And it records the intended catalog entry exactly once

#### Scenario: ID04 custody installed before catalog failure
Given custody installation succeeded and a filesystem error blocks catalog commit
When the operation reports failure
Then it returns an operation ID and conservative effects requiring reconciliation
And custody and the recovery record remain available for a later retry

#### Scenario: ID04 interrupted after catalog commit
Given catalog commit succeeded but the journal is still prepared
When the operation is reconciled
Then completion is recorded without reapplying the catalog or reacquiring custody

#### Scenario: ID08 completed create replay after forget
Given a completed create was followed by forgetting its catalog entry
When the original create request is retried with the same request ID
Then the historical create result is returned
And the forgotten entry is not recreated

#### Scenario: Concurrent mutation owner
Given another process holds the store mutation lock
When a second mutation is submitted
Then it reports store_busy without changing custody or the catalog

#### Scenario: Inconsistent recovery receipt
Given a recovery record's claimed result disagrees with its intended public catalog identity
When reconciliation is requested
Then the record is rejected before custody or catalog mutation
