# Identity development checkpoint

## Immutable source

- Core hardening: `629d0a0c55abd59d12d29e8af6f58b89fb1ecac5`.
- Lifecycle backend, CLI, and read-only overview contract:
  `4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a`.
- Review branch: `feat/identity-lifecycle-foundation`.

These commits preserve the implementation exercised by the
[hardening verification](../../openspec/changes/identity-security-hardening/verification.md).
The historical verification describes the pre-commit working-tree checkpoint;
the source now has immutable revisions. This is development software, not a
registry release or device-acceptance declaration.

## Consumer contract

For the first UI slice, use `styrene_identity::overview` with default features
disabled. `CatalogSnapshot` in `styrene-identity-lifecycle` implements the public
overview source without unlocking custody. Allocate a fresh host scope when the
session, selection, source snapshot, or enablement changes.

General Identity verification now rejects weak-key signatures and inconsistent
public objects. `SignedAttestation::verify_for` binds an independently expected
canonical ID. Repository-authority canonical bytes and rejection classes remain
unchanged. Review any consumer enrollment path that accepted external public keys
through the permissive verifier.

New journal v2 custody operations bind their output directory and compact only
after confirming the expected custody copy. Legacy v1 pending custody records
require explicit migration before another custody write. Completed receipts are
historical; `replayed` is not evidence of a running Mesh session.

## UI handoff

Host design authority remains `styrene-ui` commit
`a2baf72ecab685f324416f040879d356d53620b3`, one optional compiled page and a
typed read-only client. The local UI checkout was rechecked at
`0be5aca6ccbe60da4385694758b0411051f77f0b` and remains dirty with active mobile
work. No files in that checkout were changed by this handoff.

Standalone Identity UI and mock-host acceptance will be implemented here. Actual
Mesh host adoption requires its owner to update manifests/lockfiles together and
record exact Identity/backend/UI revisions. Missing provider support remains an
explicit unavailable result; it must not fabricate identity evidence.

## Remaining gates

Clean-tree `cargo package --locked` passed at documentation checkpoint
`764a9f349310bdeee720ab93fe25fe8b5ad2d66d` on Rust 1.97.0, `aarch64-apple-darwin`.
The package check reported the now-inventoried yanked spin dependency; follow-up
dependency changes and advisory decisions are recorded in
[the release inventory](../release-inventory.md). Backup writes, legacy migration,
standalone UI, host dogfooding, and release automation continue as separate stages.
Registry publication and cross-platform/device acceptance are not implied by these
commits or the historical package version `0.3.2`.
