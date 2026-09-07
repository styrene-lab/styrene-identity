# Changelog

## 0.4.0-rc.1 — standalone Identity candidate

Git-only candidate. Registry publication, actual Mesh host adoption, and the
remaining native/device lanes are not implied by this version.

### Security and compatibility

- Reject weak-key Ed25519 signature forgeries through strict general Identity
  verification. Published 0.3.2 source contains the permissive verifier; deployment
  exposure depends on consumer verification/enrollment paths.
- Check public-object hash/key consistency and add
  `SignedAttestation::verify_for(expected_identity, data)`.
- Zeroize owned KDF workspaces, keys, roots, and decrypted buffers; bound malformed
  encrypted-file reads. This is not a guarantee about all caller/library/OS copies.
- Preserve derivation labels, generated key/signature bytes, STID/legacy readers,
  and repository-signing conformance/rejection classes.
- Declare Rust 1.97 / edition 2024 for the standalone workspace. Consumers must
  assess the change from registry versions that did not declare this toolchain.

### Application foundation

- Add the read-only overview contract and isolated minimal consumer.
- Add file-custody catalog CRUD, revision checks, request-bound retries, and
  descriptor-relative recovery with explicit historical receipts.
- Add managed backup export, inspect/verify, reprotection, restore, inventory,
  reference forgetting, deletion, and legacy recovery migration.
- Record ownership before staging existing-root ciphertext; verify output identity
  and preserve possible recovery copies across interruption and cleanup.
- Add the `idctl` development CLI with structured JSON, exit categories, protected
  input, and operation discovery/reconciliation.
- Add the shared Dioxus page, concrete one-page host model, standalone desktop
  shell, bounded application worker, and fixture-only extension dogfood mode.

### Migration and limits

- Artifact-capable stores install a version-three forward-writer guard. Older
  catalog-only writers must not be used against those stores.
- Completed v2 receipts contain no root recovery ciphertext. Legacy retained
  copies require explicit migration; uncommitted migrated intents are superseded,
  not falsely reported as successful custody creation.
- Backup reprotection creates a new artifact and does not revoke a root or erase
  copies elsewhere. Whole-store hostile rollback is not prevented by local ledgers.
- The optional SSH graph retains RUSTSEC-2023-0071 in a transitive RSA dependency.
  Only Ed25519 is served, and RSA requests are rejected before custody access;
  release acceptance of that optional graph remains explicit.
- Native macOS overview/fixture rendering was observed. Automated native lifecycle
  controls are blocked by Assistive Access; Linux desktop, mobile, physical-token,
  and actual Mesh integration acceptance remain separate.

Application/backend/UI packages carry independent `0.1.0-rc.1` candidate versions.
