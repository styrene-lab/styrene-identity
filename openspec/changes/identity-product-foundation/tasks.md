# Product foundation tasks

## 1. Release policy and ownership decisions
<!-- specs: release-policy -->

- [ ] Inventory registry ownership/versions, Git tags, exact consumer pins, and existing release artifacts.
- [ ] Approve `RELEASE.md` policy, choose first-release numbering, MSRV support, approvers, tag namespace, and prerelease distribution.
- [ ] Implement release notes and evidence templates, API/dependency checks, package verification, and immutable publication/retry automation.
- [ ] Exercise a candidate release without publication; verify version classification and interrupted-publication handling.

## 2. Workspace and shared backend
<!-- specs: lifecycle-product -->

- [ ] Review the operation catalog with the provider-contract change and inventory existing CLI/UI consumer implementations for migration.
- [ ] Approve package names, storage/concurrency/recovery contracts, and library-only default members before changing manifests.
- [ ] Add the lifecycle backend and file-custody vertical slice with shared scenarios for creation, inspection, backup, restore, conflicts, and interruption.
- [ ] Verify minimal library consumers exclude application dependencies and existing compatibility fixtures remain unchanged.

## 3. CLI and standalone UI
<!-- specs: lifecycle-product -->

- [ ] Approve CLI name, command catalog, exit categories, JSON schema/version support, protected input, and export policy.
- [ ] Implement CLI adapters, help/examples/completions, noninteractive failure handling, and machine-output integration tests against shared backend scenarios.
- [ ] Confirm the inventoried Dioxus baseline, exact renderer/Wry compatibility, first target platforms, accessibility, platform-service ownership, and packaging.
- [ ] Implement standalone UI adapters and verify packaged create-inspect-backup-restore workflows without Mesh against the same backend outcomes.
- [ ] Test stale picker completions, share presentation without delivery confirmation, and app unlock without inferred custody evidence.

## 3a. Full CLI CRUD acceptance
<!-- specs: cli-lifecycle, lifecycle-product -->

- [ ] Review `docs/cli-lifecycle.md`: freeze executable name, catalog IDs/revisions, command intent fields, credential roles, JSON schema, and exit categories.
- [x] Implement ID01–ID03 public reads and real CLI process tests with explicit catalog absence and no implicit unlock.
- [x] Implement ID04–ID08 file-custody catalog CRUD, local preference, non-destructive forget, and revision conflict tests.
- [x] Define and implement the first mutation journal, ownership, retention, idempotency binding, and ID44/ID45 observation/reconciliation needed by those operations.
- [ ] Implement ID30–ID36 authenticated backup CRUD with non-overwrite, partial catalog failure, reprotection, and changed-artifact deletion cases.
- [ ] Before backup writes, persist recoverable staging ownership for existing-root ciphertext; test interruption, cleanup uncertainty, and preservation of a possible last recovery copy.
- [x] Implement the ID31–ID32 non-mutating backup inspection/verification slice, including wrong identity, tamper, and unauthenticated-header evidence.
- [ ] Implement supported ID09–ID16 custody operations; test wrong-identity attach, unsupported enrollment, reprotection interruption, and partial destruction cleanup.
- [ ] Implement supported ID20–ID28 key/record operations; test immutable derivation descriptors, explicit private export, and no revocation claims from local deletion.
- [ ] Implement supported ID40–ID43 plans and successor records with stale-plan rejection and explicit unresolved consumer trust work.
- [ ] Run the shared backend corpus and actual CLI subprocess cases for each shipped row, then expose the same typed outcomes to UI clients.

## 4. Optional Mesh integration
<!-- specs: lifecycle-product -->

- [ ] Agree summary/capability DTOs and one read-only request against UI design revision `a2baf72ecab685f324416f040879d356d53620b3`; record an immutable contract or mock-only status.
- [ ] Implement the Identity-side read-only client and mock host tests for incompatible registration, absence, unavailable/error/retry, and no implicit unlock or mutation.
- [ ] Verify scope cleanup, stale results after selection/disable, repeated enable/navigation, and restart with enable preference only against the UI-owner lifecycle scenarios.
- [ ] Before enabling one mutation, establish CLI/service acceptance, operation ID/outcome ownership, cancellation versus observation, and retained observation or blocked disable.
- [ ] Hand off immutable Identity revisions to the Mesh owners and collect host integration results without changing their worktrees here.

## 5. Supported product release
<!-- specs: release-policy, lifecycle-product -->

- [ ] Run supported library/backend/CLI/UI lanes and record source, dependency, feature, target, artifact, and platform evidence.
- [ ] Verify CLI machine-schema compatibility, plugin-contract compatibility, state downgrade rejection, and concurrent frontend operation outcomes.
- [ ] Review the first supported capability/platform matrix, recovery instructions, missing device lanes, and downstream migration evidence.
- [ ] Publish only after release approval; verify installed artifacts and update documentation from proposed to implemented for accepted surfaces.
