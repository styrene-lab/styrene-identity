# Adversarial Identity review: 2026-09-06

## Scope and evidence

Reviewed identity verification, encrypted-file handling, catalog mutation recovery,
and the proposed backup lifecycle. Source baseline:
`e1fcb0e014a9002cb82f45381c2096724047990f` plus this checkout's uncommitted
lifecycle/CLI work. Findings describe local source and tests, not confirmed
deployment exposure or a formal cryptographic audit.

Three rejection tests were executed before their fixes and failed:

- `identity_verify_rejects_identity_point_forgeries` accepted the forged signature.
- `adversarial_recovery_rejects_parent_directory_substitution` installed custody
  after its recorded parent pathname was redirected.
- `adversarial_recovery_rejects_unrelated_catalog_changes_in_journal` applied an
  unrelated preferred-identity change from an edited journal snapshot.

Severity below considers both prerequisites and consequences. Root/administrator
access and malicious same-UID code remain outside the local store's isolation
guarantee. The optional compiled UI extension is trusted in-process Rust, not a sandbox.

## Findings and disposition

### SEC01 — High: weak-key signature forgery in general Identity verification

`identity_verify` used permissive Ed25519 verification. With the compressed identity
point as both public key and signature R, and S = 0, the verifier accepted arbitrary
messages without possession of a private key. `PublicIdentity::verify` inherited
the behavior. The regression uses two different messages and the same forged signature.

This does not forge signatures for ordinary root-derived keys or recover their
secrets. It becomes consequential if a consumer accepts a weak self-asserted key
during registration, then treats valid signatures as evidence of possession or
authorization. Anyone can subsequently impersonate that weak-key identity.

**Fixed locally:** `identity_verify` now uses `verify_strict`. `PublicIdentity::verify`
also checks its public hash/key fields for consistency. The new
`SignedAttestation::verify_for(expected_identity, data)` combines canonical
attribution with strict signature verification. The expected identity must come
from an independent trusted context. Domain framing, replay protection, and
authorization remain consumer responsibilities.

Repository-authority verification already used strict verification and rejected
weak keys. Its profile bytes and rejection classes were not changed. The general
Identity verifier now rejects previously accepted weak-key inputs; release notes
and downstream acceptance must explicitly cover this security rejection change.
Consumers that admitted untrusted identities through the old verifier need to
revalidate those records. No published version range or deployed consumer exposure
has been established by this review.

Follow-up on 2026-09-07 verified the published 0.3.2 archive checksum and confirmed
the permissive verifier in that registry source. See
[the release inventory](release-inventory.md). This later evidence does not establish
which deployed consumers exercised the affected method.

The README extraction pin `7ce44fd8dac29299b88623ca0252e5f5cebcacfc` was also
inspected with `git show` and contains the permissive verifier. It must not be
presented as a security-fixed consumer pin. Registry versions and actual deployed
consumer dependency resolution still require their own inventory.

### SEC02 — High impact, local prerequisite: recovery pathname substitution

The original journal bound a pathname but not the filesystem directory it named.
After an interruption, renaming the parent and replacing it with a symlink caused
reconciliation to install encrypted custody in a different directory.

The immediate leak is encrypted recovery material, not plaintext. Second-order
effects include an additional offline-password target and an unexpected identity
file discovered by another application. A later password change would not protect
the previously copied ciphertext.

**Fixed locally:** v2 prepared custody journals bind the parent directory's device
and inode. Unix filesystem operations use held directory descriptors, no-follow
opens, and descriptor-relative staging/install/cleanup. Recovery rejects a changed
directory. Binding checks detect directory movement during the operation; a failed
post-install check reports uncertain effects rather than success.

V1 prepared custody records lack that binding. They cannot resume a pending custody
write automatically. If the catalog was already committed, journal completion can
still be finalized without reaccessing custody. Otherwise preserve the old record
and recovery material for an explicit migration. An inode binding is host/filesystem
specific; copying a pending store is not a supported portable identity restore.

### SEC03 — Medium within the store trust model: arbitrary after-snapshot replay

The old recovery validation checked a journal's structural consistency but did not
recompute the complete permitted catalog transition. Editing a rename journal's
after-snapshot could also change the preferred identity without a selection request.

**Fixed locally:** reconciliation recomputes create/adopt/update/select/forget from
the current before-state and declared request, then compares the entire intended
catalog. Unrelated changes fail before custody/catalog mutation. Existing before/after
digests remain crash/conflict checks, not cryptographic authentication of the store.

An attacker who controls all same-UID store files can still forge a consistent
history or roll back an entire store. A MAC key stored alongside that history would
not establish independent trust. Strong rollback detection would require a separate
trusted authority or monotonic state and is not implemented here.

Encrypted roots do not authenticate catalog intent. Full hostile store control can
still replace a declared request or change local identity-selection policy without
learning a root's password. Applications requiring hostile-at-rest integrity need
independently anchored intent and replay protection. The local filesystem checks
must not be presented as that guarantee.

### SEC04 — Medium: hidden recovery copies and accumulating identity history

Completed v1 journals retained the encrypted root and the whole resulting catalog.
This made journals another backup surface, preserving old password-protected roots
and repeated copies of unrelated identity metadata. Reprotecting the main file
alone would leave the old recovery copy decryptable with the old password. Root
compromise exposes all deterministic key families, including repository signing.
It also exposes future keys derived from that root. Rotating only a leaf epoch
does not recover from root compromise; root replacement and consumer trust
migration are required.

**Reduced for new operations:** completed v2 receipts contain neither the encrypted
creation artifact nor the full catalog snapshot. Compaction occurs after durable
custody/catalog completion and a check that matching custody ciphertext still
exists at the bound location. A further regression verifies that disappearance
after catalog commit preserves the recovery copy. Filesystem snapshots, backups, unlinked blocks, and
previously copied data are not securely erased by this change.

Existing completed v1 records are not silently rewritten or stripped of what might
be their last recovery copy. `operation show` reports `journal_version` and
`retains_recovery_material`. An explicit legacy-copy migration/retention workflow
remains necessary. Backup reprotection and destruction must inventory all managed
recovery copies before claiming their effects.

### SEC05 — Medium: authority files relied on assumed local permissions

Creation modes protected new files, but existing directories/files could permit
other principals to alter recovery or catalog authority. Multiple hard links to a
lock/journal also create ambiguous ownership and replacement behavior.

**Hardened:** mutation stores must be private to the effective user. Custody output
parents must be owned by that user and not group/other writable. Private operation directories and
private lock/journal files must exclude group/other access; authority files must
have one link. The catalog may be publicly readable but cannot be group/other
writable when used for mutation. Unsafe existing storage is rejected, not silently
chmodded. Read-only catalog data remains untrusted public metadata.

Descriptor-relative reads reject symlinks and nonregular files, use nonblocking
opens, and enforce size limits. The core `FileSigner::load` also now bounds reads
to the fixed encrypted-file size plus one byte. The legacy core file API still
expects caller-controlled paths; the stronger directory guarantees belong to the
lifecycle adapter rather than every `FileSigner` caller.

Exclusive installation uses no-replace rename on supported Linux/Android and Apple
targets. This avoids a link/unlink crash interval that could leave a legitimate
journal with two links and make the new single-link check reject recovery. Normal
error cleanup is synced; abrupt termination can still leave private staging files.
No automatic orphan pruning or secure-erasure guarantee is claimed.

### SEC06 — Integration risk: a historical success is not current custody evidence

A completed create retry after forgetting its entry returns the original result.
This is correct idempotency behavior, but consumers could read `created` as evidence
that custody or a running session exists now.

**Clarified in the API:** completed retries set `replayed: true`. Operation receipts
are historical. A consumer must query current state and verify the actual runtime
binding before reporting activation. A valid signature, hash consistency, restored
custody, selected profile, and remote authorization are separate facts.
Another consumer can observe an installed custody file before the catalog commits.
That follows from the commit order; actual Mesh activation behavior was not tested
in this review and remains a separate consumer acceptance case.

### SEC07 — Accepted format limitation: STID header is not authenticated

Stripping the five-byte STID header produces a readable legacy artifact with the
same authenticated payload and identity. This is not a weaker KDF or an arbitrary
root substitution: both readers use the existing encryption parameters. It does
mean format metadata cannot be presented as authenticated provenance.

**Exposed accurately:** `backup inspect` reports unauthenticated metadata.
`backup verify` reports authenticated payload and canonical identity, but
`format_header_authenticated` remains false. Tampering with encrypted payload
bytes fails authentication. A future header-bound format needs its own version,
migration, and conformance vectors; existing bytes were not silently changed.

## Additional crypto review notes

Source review also found that the allocating Argon2 convenience API did not wipe
its full block workspace. That memory contains password-derived material from
which the encryption key is finalized. The file signer now explicitly owns and
zeroizes that workspace and enables Argon2's supported intermediate zeroization.
This reduces heap residue after operations; it is not a demonstrated remote
exploit or a guarantee that all transient copies are erased.

- Fixed salts/labels and deterministic key families remain unchanged. Domain
  separation does not contain compromise of the shared root itself.
- File KDF buffers, generated roots, and decrypted plaintext now have RAII
  zeroization on error/unwind paths. Inspection of locked `chacha20poly1305` 0.10.1
  confirmed that its owned cipher key already zeroizes on drop. No claim is made
  that every compiler/library/caller copy is erased.
- Its decryption path verifies the authentication tag before applying the
  decryption keystream. The reviewed failure path therefore does not expose
  unauthenticated plaintext through the returned buffer.
- Argon2id work is intentionally expensive. Verification is blocking and must run
  on bounded workers in service/UI consumers. CLI protected input is size-bounded
  and waits for explicit EOF; it is not a credential-source timeout service.
- Neither public-key hash consistency nor constructing `PublicIdentity` validates
  enrollment authority. Strict verification must be paired with an independently
  selected expected identity and application policy.

## Backup progression

Implemented `backup inspect` and `backup verify` against the hardened reader, with
wrong-password, payload-tamper, wrong-identity, legacy-header, and non-mutation tests.
These commands require no catalog and emit no plaintext key material.

Export/restore/reprotection and managed-artifact removal remain planned. Their next
implementation must use the hardened transaction boundary and address retained v1
copies, distinct credential roles, historical receipts, and deletion-versus-revocation
semantics. No backup mutation or erasure guarantee is inferred from this read slice.

In particular, backup writers will handle already-used roots, unlike a new root
that has not yet been admitted. An interrupted staging write could otherwise leave
an untracked copy protected by an older or weaker password. Persist recoverable
staging ownership before writing that ciphertext, and define cleanup/reconciliation
without deleting a possible last recovery copy. A random temporary filename and
best-effort unlink alone are insufficient evidence for a completed cleanup claim.

## Handoff and verification limits

This is an uncommitted working-tree implementation. Consumer updates need a reviewed
immutable Identity revision, verification-rejection release notes, and exact consumer
tests. The UI/backend consumer inventory does not establish deployment exposure.
No registry publication, CVE assignment, UI/device acceptance, physical power-loss
test, or formal cryptographic proof was performed.
