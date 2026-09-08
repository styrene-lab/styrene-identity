# Identity product architecture

## Direction and implementation status

This repository owns the importable Identity library and the planned Identity
lifecycle product: a shared application backend, a CLI, a standalone UI, and an
optional integration that supplements the Styrene Mesh UI.

The standalone CLI and UI must support identity work without a mesh installation
or running daemon. The same backend operations must serve both interfaces.
The Mesh application retains its small operating surface and runtime ownership.

The Identity library, file-backed catalog/backup backend, CLI, shared Dioxus page,
standalone desktop shell, and mock host are implemented. Actual Mesh integration
and advanced custody boundaries remain proposed in the
[product foundation change](../openspec/changes/identity-product-foundation/proposal.md).
The [provider contract change](../openspec/changes/identity-provider-contract/proposal.md)
defines a prerequisite capability and custody boundary.

## Proposed dependency direction

```text
Identity CLI ────────────────┐
Standalone Identity UI ─────┼──> Identity lifecycle backend ──> Identity library
Mesh Identity integration ──┘                 │
                                            └──> provider contracts/adapters

Mesh shell ──> Mesh runtime/profile operations (owned by styrene-rs)
```

The Identity library and lifecycle backend must not depend on Dioxus, CLI parsing,
the Mesh runtime, or its IPC. Presentation and host bridges depend on backend
contracts. A plugin transport, if selected later, belongs at that outer boundary.

## Workspace shape and remaining packages

The root library, `crates/styrene-identity-lifecycle`, `crates/styrene-identity-ui`,
`apps/cli`, and `apps/desktop` are workspace
members. The root library is the only default member, preserving library-only
commands. Application packages are development-only (`publish = false`) until
release review; they use versions independently from the root library.

| Package | Responsibility | Distribution |
|---|---|---|
| `styrene-identity` at the current root | Crypto, IDs, records, formats, custody primitives | Importable library; preserve existing package identity |
| `styrene-identity-lifecycle` | Identity selection, operation policy, workflows, typed outcomes, recovery state | Importable application backend |
| `styrene-identity-cli` | Commands, human output, versioned machine output, interaction adapter | Installed binary; command name to confirm |
| Identity UI package(s) | Shared lifecycle views and standalone application shell | Standalone application and reusable presentation where supported |
| Identity plugin bridge | Host capability negotiation and Identity/host result translation | Packaging mechanism pending host contract review |

Add members incrementally. Avoid moving the existing root crate merely to make
directories uniform. Library-only builds must remain independent of application
dependencies. Default workspace members and CI lanes must preserve that property.
In-repository member paths are legitimate workspace dependencies; published
manifests require versioned resolution. External consumers must not use sibling
checkout paths.

A shared Dioxus UI is the proposed presentation baseline, based on the
[local UI integration inventory](ui-integration-inventory.md). Use Identity-owned
views with a standalone shell and thin Mesh host adapters. The inspected Mesh
workspace pins Dioxus `=0.8.0-alpha.1` and patches Wry; review exact renderer
compatibility before selecting dependencies. Rust state and typed platform-service
separation already exist there. UI implementation remains pending. The committed
UI-owner design selects one compile-time optional page with minimal registration
and a typed read-only client. Independent loading and generic extension machinery
are deferred until dogfooding demonstrates a need.

## One lifecycle application backend

Frontend code translates input into typed backend operations and renders typed
results. Identity selection, validation, expected-identity checks, custody policy,
backup handling, and operation effects belong in the backend or lower library.
Frontends must not implement independent identity-file readers or derivations.

The backend is an embeddable Rust library. A daemon is not required for standalone
work. The initial [file-custody CRUD service](file-custody-crud.md) uses an OS store
lock, request-bound operation IDs, and a durable recovery journal. Wider provider
and UI operation-lifetime contracts remain pending.

| Operation family | Shared behavior to establish |
|---|---|
| Inventory and selection | List identities/providers, inspect canonical IDs and evidence, select an explicit target |
| Creation and custody | Create, enroll supported adapters, unlock per operation, report actual custody capabilities |
| Keys and signatures | Public derivation, purpose-specific signing, explicitly supported private export |
| Inspection | Inspect and verify bindings, certificates, and encrypted backup metadata |
| Recovery | Authenticated export/import, destination conflict handling, recovery progress and retry |
| Lifecycle planning | Explain rotation/retirement effects and required consumer trust updates |
| Local removal | Distinguish removing a reference from deleting stored custody |

Rotation execution requires a defined family/epoch and a supported trust-update
workflow. Deleting an export does not revoke a deterministic key. Remote
revocation and retirement cannot be reported as complete from a local record alone.
Features without implemented semantics must report unsupported capability.

## CLI contract

The [full CLI CRUD lifecycle](cli-lifecycle.md) defines proposed command families,
backend operations, stable acceptance IDs, transaction outcomes, and delivery
order. It distinguishes catalog deletion, custody destruction, key references,
backup artifacts, and immutable lifecycle records.

The CLI is a first-class automation surface. Establish command names, stable exit
categories, versioned JSON output, help, examples, and completion support. Human
text can evolve independently of machine fields. Standard output carries the
requested result; diagnostics and interaction use separate channels.

Every command must define noninteractive behavior. Missing protected input must
produce a structured failure rather than wait indefinitely. Credentials must not
be supplied through command-line arguments or printed in diagnostics. Review
terminal input, protected input streams, and platform prompts explicitly.
Private output requires an explicit export operation and destination policy.

The CLI and UI use the same backend error/effect model, including cancellation,
partial completion, inaccessible custody, and restart recovery. A shared scenario
suite must establish parity before an operation ships in either interface.

## Standalone UI and Mesh integration

The standalone application owns identity navigation and lifecycle presentation.
It can create, inspect, and recover disposable identities with no Mesh process.
Its settings and identity references need a versioned storage/migration policy.
It must not silently adopt a Mesh profile or replace a Mesh session identity.

The Mesh integration supplies lifecycle capabilities through the reviewed provider
contract. The host retains profile selection, session changes, networking, and
verification of the identity actually used by its runtime. The Identity component
returns custody outcomes, not claims of successful Mesh activation.

The first integration follows the committed design in the
[UI inventory](ui-integration-inventory.md#committed-host-design-handoff): trusted
Rust compiled with the host Dioxus version and checked for compatibility before
activation. Start with one read-only overview and a mock client. Agree public
summary/capability DTOs before adopting a real provider. Mounting never unlocks or
mutates custody. Runtime capability discovery is not a dynamic plugin resolver.
Mesh must remain usable when the optional lifecycle component is absent.

## Delivery sequence

1. Establish release policy and actual consumer/host inventory.
2. Review provider contracts and the backend operation catalog together.
3. Add the lifecycle backend and a CLI vertical slice for file-backed creation,
   inspection, authenticated backup, and non-overwriting restore.
4. Prove the same slice in a standalone UI using the shared scenario corpus.
5. Integrate the optional Mesh surface against exact host revisions.
6. Expand platform custody and lifecycle operations with separate device evidence.

Library maintenance releases can proceed independently of application milestones.
The first product release does not imply that every proposed custody or lifecycle
operation is available.
