# Identity 0.4.0-rc.1 candidate status

## Software gate

`scripts/validate.sh` passed on 2026-09-07 with Rust 1.97.0,
`aarch64-apple-darwin`, two test threads:

- Workspace: 245 unit/integration tests and 3 doctests passed; 3 doctests ignored.
- Expanded root profile (`repository-signing,ssh-agent,pki,age-format`): 224 tests
  and 3 doctests passed; 4 doctests ignored.
- Minimal core/repository-signing checks, the public-only lifecycle check, and all
  6 isolated overview-consumer tests passed.
- Workspace and expanded-profile Clippy passed with warnings denied.
- Formatting, workspace rustdoc, and macOS Keychain/YubiKey compilation passed.
- Committed cryptographic vector files were unchanged.

The first script invocation stopped at formatting; formatting was corrected before
the complete successful run. No failed test or dirty-package bypass was used.

## Release profiles

Target-filtered dependency checks passed for core, repository-signing, CLI, and
macOS desktop. The existing BlueOak dependency license and unchanged MPL-2.0
dependency are explicitly accounted for. The unmaintained `paste` proc-macro notice
is recorded as such, not presented as a patched vulnerability.

The optional SSH graph still reports RUSTSEC-2023-0071 for its transitive RSA
dependency. A focused test proves that RSA sign requests are rejected before
custody access; no RSA private operation is provided here. No blanket vulnerability
ignore or unqualified clean-audit claim was added.

## Product and host evidence

The standalone UI and mock host were built and observed on macOS 26.6.2. Exact
development artifact/process/capture evidence is in `docs/ui-runtime-evidence.md`.
Typed UI service parity exercises create → export → restore with the shared backend.
Mock-host tests cover compatible registration, absence, enable/disable, stale scopes,
and independent pending-operation observation.

Native-control automation is blocked by missing Assistive Access. Actual Mesh host
adoption, native keyboard/focus/accessibility interaction, Linux desktop, mobile,
and physical-token/device acceptance remain unexecuted or externally owned lanes.

## Candidate versus release

The candidate workflow packages immutable source, CLI/native artifacts, checksums,
toolchain metadata, dependency results, and source notices. It does not publish to
crates.io. Registry publication remains gated on the advisory decision, consumer
acceptance, and release approval. The first profile-bearing previous-release lane
is unavailable because registry 0.3.2 predates that profile.
