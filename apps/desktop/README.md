# Standalone Styrene Identity desktop

The Dioxus desktop shell consumes the same lifecycle services as `idctl`. It
supports catalog CRUD, backup export/verification/reprotection/restore, managed
artifact removal, and operation reconciliation through a bounded application worker.
It never launches a Mesh daemon or shells out to the CLI.

```sh
cargo run --locked -p styrene-identity-desktop --features desktop -- --store /absolute/path/to/store
```

Without `--store`, the app asks for an explicit absolute directory. Launch and
public catalog reads do not create an identity. Choose Lifecycle to create/adopt
one. Source and destination protection fields are separate, cleared after submission
or page changes, and never persisted in application settings. Rust-owned buffers
zeroize on drop; WebView/native input copies are outside that guarantee.

The application root owns queued operations and their results. Navigation does not
cancel backend work. A restarted app reads pending operation IDs without replaying
them; use Recovery with the appropriate protection inputs. Closing the process can
interrupt work, which is why the backend journals staging and commit boundaries.

## Shared page and host experiment

`styrene-identity-ui` exports one read-only `IdentityPage`, a typed `OverviewClient`,
and the concrete `IdentityExtension` lifecycle model. It has no Mesh dependency or
CLI execution path. Its stylesheet is scoped to the page; the host owns navigation,
session selection, theme tokens, and persistent enablement.

```sh
cargo run --locked -p styrene-identity-desktop --features desktop -- --extension-demo
```

This explicitly mocked host opens the fixture Identity page and exposes enable,
disable, navigation, session change, provider availability, and pending-operation
observation controls. It is not an installed plugin loader or actual Mesh integration.
The pure model tests cover restart preference input; real host preference persistence
remains with the Mesh owner.

## Build and evidence

Use the exact `dx 0.8.0-alpha.1` CLI:

```sh
# From apps/desktop
IDENTITY_BUILD_REVISION="$(git rev-parse HEAD)" dx build --package styrene-identity-desktop --platform desktop --features desktop
```

The desktop binary is feature-gated, so normal workspace tests do not require a
native renderer. Explicit desktop builds do. The first application target is macOS;
Linux desktop dependency/runtime issues and mobile platform adapters remain separate
acceptance lanes. The Linux CLI remains independent of GTK and WebView dependencies.

Native overview rendering was observed on the Apple host with a disposable catalog.
Automated native controls are currently blocked because `osascript` lacks Assistive
Access. SSR/model/service tests do not substitute for that permission-dependent
interaction, VoiceOver, or physical mobile acceptance.
