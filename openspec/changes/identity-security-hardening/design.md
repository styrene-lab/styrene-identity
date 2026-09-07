# Hardening design and compatibility

The source-backed findings and threat prerequisites are in
[the adversarial review](../../../docs/adversarial-review-2026-09-06.md).

## Verification

Use Ed25519-dalek strict verification for the general Identity verifier, as the
repository-authority verifier already does. Public identity object verification
also checks its hash/public-key consistency. `SignedAttestation::verify_for` takes
an independently expected canonical ID. It does not define application authorization
or replay policy.

This rejects inputs previously accepted by the permissive helper. It changes no
derivation labels, emitted signatures, or repository-signing profile bytes/error
classes. Consumers need an exact-revision rejection-change handoff and review of
any weak-key identities previously admitted. No registry version is selected here.

## Local storage

Use safe Rustix descriptor-relative Unix operations. Hold directory descriptors
through reads and atomic writes; reject symlinks/nonregular files and bound reads.
Mutation authority requires effective-user ownership, appropriate permissions,
and single-link files. Descriptor binding and device/inode pins are filesystem
conflict checks, not signed anti-rollback evidence or same-UID isolation.

Recompute the full intended catalog transition from the current before-state and
the declared request. A structurally valid journal after-snapshot cannot authorize
extra mutations. Shared store locking remains nonblocking for competing writers.

## Journal v2

New prepared custody operations pin their parent directory identity. Completed v2
receipts drop encrypted creation bytes and full catalog snapshots. Before discarding
a creation recovery copy, check that the matching encrypted custody copy still
exists in the bound directory. This requires no credential or plaintext root.
If custody disappeared, preserve the prepared journal and report reconciliation
required rather than discard recovery material.

Completed v1 records remain readable and report retained recovery material. Do not
silently erase potentially irreplaceable copies. V1 pending custody writes lack
the required pin and cannot resume automatically; already-committed catalog state
can finalize without another custody write. Migration tooling is still pending.
Whole-store rollback and filesystem snapshot erasure are not provided.

## Crypto memory and backup reads

Use RAII for generated roots, file KDF keys, and decrypted plaintext. Enable Argon2's
zeroization support and own its block workspace through `Zeroizing`; its convenience
allocator does not erase that full workspace. Transient/caller/OS copies remain
outside this guarantee. Parameters and wire bytes remain unchanged.

Expose ID31–ID32 through a shared backend. Read at most the supported encrypted-file
size. Authenticate one payload and derive its public identity from the same root.
Report `format_header_authenticated: false`, including after payload verification,
because stripping the current header preserves legacy compatibility. Header binding
requires a future versioned format, not reinterpretation of v1 bytes.

## Remaining release gates

Review exact consumer source revisions, revalidate imported identities where needed,
document the verifier rejection change, and run consumer acceptance before pinning.
Review legacy recovery-copy migration and artifact ownership before backup mutation
or destruction commands. Physical power-loss, device custody, and UI lifecycle
acceptance remain distinct from this software corpus.
