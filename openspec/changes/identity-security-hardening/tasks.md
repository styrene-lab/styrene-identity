# Security hardening tasks

## 1. Reproductions and local mitigation
<!-- specs: security-contracts -->

- [x] Reproduce weak-key forgery, parent redirection, and unintended catalog-transition replay with failing tests.
- [x] Implement strict verification, public-object binding checks, and expected-identity attestation verification.
- [x] Add descriptor-relative filesystem operations, authority permission checks, directory pins, and full transition validation.
- [x] Implement compact v2 receipts with custody-presence checks, legacy-retention reporting, and explicit historical replay.
- [x] Add bounded, non-mutating backup inspection/verification with header-evidence distinctions.
- [x] Complete broad validation of the final changes, including the Argon2 workspace zeroization update and conformance fixtures.

## 2. Consumer and release handoff
<!-- specs: security-contracts -->

- [ ] Review the tightened verifier behavior and record the immutable Identity revision for consumer adoption.
- [ ] Replace the historical README extraction-pin examples with a reviewed security-fixed source pin once that revision exists.
- [ ] Inventory affected consumer verification/enrollment calls and run exact-revision acceptance, including formerly admitted weak-key records if any.
- [ ] Define and validate explicit legacy pending-journal/recovery-copy migration before advancing existing stores through a release.
- [ ] Apply the hardening constraints to backup mutation/retention workflows; retain separate UI, platform, and device evidence.
