# Identity host integration and SSH follow-up

## Immutable consumer handoff

The desktop host adapter is pushed at UI revision
`455d77cd64183c66880c2b4f386f14e651697652`:
<https://github.com/styrene-lab/styrene-ui/pull/46>.

It consumes Identity `eaa5223ddafa28a5de88caa6e132bf6df1cc3eb1` and backend
`a04d2b572deda3aea4749fc1a9481ecff7397fec`. Implementation used an isolated UI
worktree based on `7b757aa804b23cb579e20d6ac161f84b791529ca`.

The host owns registration, navigation, persisted enablement, and generation
invalidation. The shared page uses a typed, read-only client. Connected live
sessions report unsupported because the current daemon contract supplies no
canonical Identity signing public key. Fixture sessions render explicitly labeled
public data. The page starts no custody or CLI operations.

On Rust 1.97.0, `aarch64-apple-darwin`, the UI passed:

- `cargo test --locked -p styrene-dx`: 171 passed, 4 ignored.
- `cargo test --locked -p styrene-dx --no-default-features`: 168 passed, 4 ignored.
- `cargo clippy --locked -p styrene-dx --all-targets -- -D warnings`.
- Formatting and whitespace checks.

The existing vendored Wry dependency emitted 17 warnings. The initial Clippy run
found new and pre-existing test `unwrap()` calls. Descriptive `expect()` calls
resolved them before the successful rerun.

Native controls remain unverified because macOS Accessibility permission was
denied. These tests do not establish runtime, mobile, Linux, or hardware acceptance.

## Backend request

Add a versioned, read-only canonical Identity overview contract associated with
the active session generation. Supply the canonical signing public key only when
the backend can establish that binding. Keep RNS and LXMF addresses separate.

Acceptance cases:

1. Reading the overview does not unlock custody or perform a private operation.
2. Missing canonical identity, unsupported providers, and provider failures remain
   distinguishable.
3. Reconnect and profile changes invalidate old results.
4. An expected canonical identity mismatch is rejected.
5. Custody availability does not imply successful authentication or hardware attestation.
6. Older consumers continue to decode the existing transport identity response.

The consumer must adopt a reviewed immutable backend revision and lockfile before
replacing its current unsupported response.

## SSH dependency decision

The source follow-up adds coverage for RSA private-key import rejection and a
guarded non-applicability exception. See `docs/dependency-policy.md`.

Checks on Identity base `6950862d7c7ff738d2962026ee82b1b8865af6ad` plus the changes
in this commit, Rust 1.97.0, `aarch64-apple-darwin`:

- `scripts/check-dependencies.sh ssh`: passed, including the focused signing and
  import rejection test, exact dependency versions, expiry guard, and wrapper ban.
- `scripts/check-dependencies.sh core`: passed without the RSA exception.
- `cargo deny --locked --exclude-dev --target aarch64-apple-darwin --features ssh-agent check advisories`:
  failed as expected with RUSTSEC-2023-0071. This verifies that the guarded
  exception does not suppress the ordinary advisory check.
- `cargo fmt --all -- --check`, `git diff --check`, and shell syntax checks passed.
- `CARGO_INCREMENTAL=0 cargo clippy --locked --all-targets --features repository-signing,ssh-agent,pki,age-format -- -D warnings`:
  passed.
- `CARGO_INCREMENTAL=0 cargo test --locked --features ssh-agent ssh_agent::tests -- --test-threads=2`:
  all 12 SSH agent tests passed.

The first expanded Clippy attempt timed out. Its retry stalled in the existing
incremental build directory and blocked a subsequent test attempt. After stopping
only the owned compiler processes, both commands passed with incremental compilation
disabled. No compiler cache was deleted and no diagnostic was suppressed.

The RC.1 tag and uploaded artifacts still point to `eaa5223...`. This follow-up is
not a new packaged release and does not establish registry publication approval.
