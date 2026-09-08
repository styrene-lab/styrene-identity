# Styrene UI integration inventory

## Evidence boundary

Inspected the local `styrene-lab/styrene-ui` checkout read-only on 2026-09-06.
HEAD was `e9cb875c19cc7039488b4737bf968d4df35c6f41`. The checkout had tracked
changes and untracked files, including mobile session, presentation, state,
packaging, and test work. Observations below describe that working tree; HEAD
alone does not reproduce all inspected content. No files were changed there.

Its manifests declare backend revision
`cdeda5c80d7cacf1811d325239e97d4c980ba773`. This inventory did not run Cargo
resolution, tests, builds, or installed-application validation. Before handoff,
obtain a reviewed immutable UI revision and recheck the relevant source and lockfile.

Paths below are relative to the UI repository. They are source references, not
sibling dependencies required by Identity development.

## Findings and design consequences

| Evidence | Observation | Identity design consequence |
|---|---|---|
| `Cargo.toml:3–12,26–27` | Desktop, shared mobile, iOS/Android launchers, state, platform, app, and Apple bridge are workspace members; Dioxus is pinned to `=0.8.0-alpha.1` | Use Dioxus as the proposed Identity presentation baseline; coordinate the exact version at integration time |
| `crates/styrene-ui-app/Cargo.toml:10–14` | Presentation depends on Dioxus, state, and platform contracts | Follow the separation of presentation, typed services, and Rust state rather than importing the Mesh application wholesale |
| `apps/desktop/Cargo.toml:15–25` | Desktop uses the Dioxus desktop renderer and public IPC/session dependencies | Standalone Identity can use a desktop shell while substituting the Identity backend for Mesh services |
| `apps/mobile/Cargo.toml:13–41` | Shared mobile composition includes daemon target dependencies and native bridges | Reuse architectural patterns, not the mobile application package as Identity's backend |
| `apps/mobile-ios/Cargo.toml:1–15`, `apps/mobile-android/Cargo.toml:1–12` | Hosts have explicit versions and thin launch dependencies | Independently versioned platform launchers are an existing ecosystem pattern |
| `docs/mobile-support.md:3–20` | Documented host targets are iOS 17 and Android API 28 minimum/API 35 build target | Plan cross-platform contracts now; these are Mesh targets, not yet an Identity support promise |
| `Cargo.toml:60–61` | Workspace patches Wry to `vendor/wry` | Any native UI reuse needs a renderer/patch compatibility review; adding Dioxus alone does not reproduce host behavior |

The root UI `AGENTS.md` still describes desktop as excluded. Its current manifest
includes desktop. For this inventory, the executable workspace membership is
authoritative. This documentation discrepancy remains with the UI owner.

## Existing identity call path

`apps/mobile/src/session.rs` exposes these concrete candidate migration points:

| Consumer call | Observed behavior | Boundary to preserve |
|---|---|---|
| `confirm_identity_creation` (239–244) | Queues a generation-tagged bootstrap request | Explicit creation and stale-request rejection |
| `restore_identity` (246–256) | Sends opaque document bytes and protection through bootstrap | UI transports the artifact; backend authenticates and restores |
| `export_portable_identity_backup` (329–340) | Sends an export request through a bounded response channel | Typed session failure and encrypted artifact output |
| Bootstrap (600–648) | Calls `MobileNode::identity_presence`, restores before boot, rechecks presence, then boots | Custody restore and Mesh runtime activation remain separate phases |

These calls currently reach `styrened` APIs, including
`MobileNode::restore_identity_before_boot`. The reviewed manifests have no direct
`styrene-identity` dependency. This is a consumer migration through the daemon
boundary, not simply a UI import substitution. The daemon's resolved Identity
source and deeper custody calls still require a separate inventory.

The public UI request methods carry a generation, but not an expected canonical
Identity ID. A generation detects stale work; it does not establish identity
binding. The observed restore path rechecks custody presence before boot; this
alone does not establish that the running session uses an expected Identity ID.
Retain these as acceptance requirements for the provider/host migration.

`crates/styrene-ui-app/src/lib.rs` already exposes backup/restore event handlers
using `IdentityBackupProtection`. Those hooks can be adapted to lifecycle backend
requests after contract review. Existing Mesh operating controls should remain
available rather than being moved into the optional advanced component.

## Platform service semantics worth preserving

`crates/styrene-ui-platform/src/document_exchange.rs` provides:

- `OpaqueDocument`: bounded to 16 MiB with Debug-redacted contents (7–46).
- Picker/share completions correlated to request generations (54–99,124–138).
- Typed picker failures for cancellation, size, availability, and read failure (69–75).
- `DocumentShareOutcome::Presented`, explicitly not proof of completed sharing (110–115).

The transport's size bound does not replace Identity backup-format validation.
An outdated document callback must not affect a newly selected identity. A shown
share sheet must not be reported as a completed backup delivery.

`apps/mobile/src/platform.rs` routes document services through Android native
methods and the Apple bridge. `crates/styrene-ui-platform/src/lib.rs:20–26`
distinguishes authenticated, cancelled, unavailable, and failed device-authentication
outcomes. App unlock is separate from provider custody authentication and signing
evidence. Native prompt/picker adapters belong outside the lifecycle backend.

Whether to consume selected UI platform crates at an immutable revision or extract
a smaller shared service package requires owner review. Do not copy their types
into a second authority or introduce a repository dependency cycle. Identity-owned
backend contracts can be implemented by thin host-side adapters.

## Integration recommendation and remaining gates

### Committed host design handoff

The subsequent UI-owner design at commit
`a2baf72ecab685f324416f040879d356d53620b3` resolves first-slice composition.
Its `openspec/changes/identity-ui-extension/design.md` was verified with `git show`.
This is immutable design evidence, not implementation or runtime acceptance.

- One trusted optional Rust dependency compiled with the host Dioxus version.
- One page and private registration: extension ID/build version, host contract
  revision, and page ID/title/component. Reject mismatch before activation.
- A host-supplied typed client and session context, initially public/read-only.
  Use a mock for the spike; an unimplemented production provider reports unsupported.
- Host-owned navigation, theme, session selection, and enabled preference;
  extension-owned page content and temporary view state.
- Separate enablement, mount state, availability, and backend operation state.
  Closing a page does not stop a provider. Mounting never unlocks or mutates custody.
- Disable/session change disposes scopes and rejects late results. Restart restores
  enablement only, without secret drafts or mutation replay.
- Before mutations, retain operation observation independently of the page or
  block disable with a precise reason. No operation registry for the read-only spike.

Runtime installation, hot unload, generic SDKs, general slots, and runtime dependency
resolution remain deferred until dogfooding demonstrates a need. Trusted compiled
Rust is not a sandbox. The UI owner implements host lifecycle; Identity owns
agreement on service DTOs and subsequent backend operations.

### Initial inventory conclusions

The observations below predate that committed design; the handoff above now
selects component integration as the first slice.

Adopt shared Dioxus Identity views with a standalone shell and a thin Mesh host
adapter as the first design path. No plugin loader, discovery mechanism, or
version-negotiated extension API was found in the inspected Rust/manifests/docs.
This is evidence for component integration first, not proof of independently
loadable plugin support. Keep runtime/backend contracts renderer-independent.

The checkout supports narrowing these choices:

1. Dioxus presentation, with exact alpha-version and renderer compatibility reviewed.
2. Rust-owned state and lifecycle operations with typed native-service adapters.
3. Separate platform launchers and independently versioned application artifacts.
4. Generation-aware asynchronous operations plus the new expected-identity checks.

It does not resolve the Identity CLI name, first registry release number, Identity
platform support matrix, shared local catalog storage, or independently installed
plugin mechanism. Recommended first packaged Identity UI evidence is macOS on
the assigned Apple host; Linux, iOS, and Android remain explicit subsequent lanes
until their scope and acceptance are agreed.
