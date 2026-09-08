# Identity provider contract tasks

The first read-only contract and mock consumer are implemented. Full provider
operations and joint UI acceptance remain pending.

## 1. Inventory and review the operation boundary
<!-- specs: provider, portable-restore -->

- [ ] Record exact consumer revisions, call sites, feature sets, root use, and runtime/import ordering against the candidate map in `design.md`.
- [ ] Resolve the six open design decisions, including purpose-specific binding, export capabilities, interaction, effect states, and runtime evidence.
- [ ] Review requests/results and migration with consumer owners; record accepted decisions and update the delta scenarios before public API implementation.

## 2. Identity-bound provider behavior
<!-- specs: provider -->

- [x] Implement and validate the read-only overview API plus an isolated default-features-disabled mock consumer for binding, unavailability, and stale-result behavior.

- [ ] Agree and test the first read-only overview DTOs with the UI owner, including absent canonical identity, no implicit root acquisition, and invalidated observation scopes.

- [ ] Add the reviewed provider types and feature gates alongside `IdentitySigner` without mesh or UI dependencies.
- [ ] Implement capability, availability, interaction evidence, and structured failure behavior for a mock provider.
- [ ] Implement expected-identity checks and signature verification with tests for wrong identity and identity changes after discovery.
- [ ] Implement explicit selection, cancellation, and secret/export boundaries; test missing hardware, unknown evidence, unsupported export, and authentication failure without fallback.
- [ ] Add a legacy adapter whose root acquisition remains internal and whose custody evidence accurately reports process-memory exposure.

## 3. Portable restore and recovery boundary
<!-- specs: portable-restore -->

- [ ] Adapt existing vault operations to reviewed identity-bound requests and typed custody-effect outcomes without changing backup bytes.
- [ ] Test malformed backups, failed authentication, wrong identity, conflicting/inaccessible custody, same-root retry, and concurrent creation using disposable fixtures.
- [ ] Add a mock consumer transaction with phase-failure injection; cover cancellation, unknown completion, profile failure, restart, and mismatched running identity.
- [ ] Document consumer-owned journaling, rollback, activation verification, and retained-custody recovery against the reviewed transaction contract.

## 4. Conformance and consumer handoff
<!-- specs: provider, portable-restore -->

- [ ] Add an isolated mock-consumer workspace and ensure the normal validation path executes every delta scenario.
- [ ] Run locked default, expanded-feature, and minimal checks from `CONTRIBUTING.md`, plus focused provider checks and formatting, Clippy, and rustdoc for the changed surface.
- [ ] Verify dependency direction, unchanged committed vectors, legacy signer selection, legacy backup readers, and non-overwriting restore.
- [ ] Update public API and plugin-boundary documentation to distinguish implemented behavior from remaining packaging and runtime design.
- [ ] Record exact Identity/consumer SHAs, commands, features, targets, Apple/Android impact, and unexecuted device lanes for migration review.
