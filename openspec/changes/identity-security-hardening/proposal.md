# Identity verification and recovery hardening

## Intent

Close the demonstrated weak-key signature forgery and recovery-state substitution
issues before expanding backup mutations. Reduce unintended retention of root
recovery material and distinguish cryptographic evidence from application state.

The [adversarial review](../../../docs/adversarial-review-2026-09-06.md) records
prerequisites, reproductions, fixes, and residual risks. This change is implemented
locally for review; it is not a release or deployment-acceptance claim.

## Scope

- Strict, expected-identity-aware verification and consistent public identity views.
- Owned key/KDF/plaintext buffer zeroization and bounded encrypted-file reads.
- Descriptor-relative storage, directory binding, and complete transition validation.
- Journal v2 compact receipts with explicit legacy-retention behavior.
- Non-mutating backup inspection and authenticated verification (ID31–ID32).

Backup export/restore/reprotection/deletion, legacy-store migration tooling, remote
trust repair, and untrusted-plugin isolation remain separate implementation work.

## Success criteria

- Previously failing forgery and recovery-substitution cases are rejected.
- Valid root-derived signatures and committed cryptographic vectors remain valid.
- Compaction does not discard the last observed recovery copy when custody is absent.
- Unsafe storage and unsupported legacy pending custody recovery fail without replacement.
- Backup verification authenticates the payload and expected ID without claiming header provenance.
- Release and consumer handoff explicitly record tightened signature rejection behavior.
