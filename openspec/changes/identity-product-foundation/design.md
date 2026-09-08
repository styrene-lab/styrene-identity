# Product foundation design

## Architecture and release policy

The detailed proposed designs are
[product architecture](../../../docs/product-architecture.md) and
[release workflow](../../../RELEASE.md). These documents remain proposed until
review and implementation establish their guarantees.

The workspace and ID01–ID08 file-catalog implementation now live in
`crates/styrene-identity-lifecycle` and `apps/cli`. Root-only default members
preserve ordinary library commands. The strict public catalog v1 reader is
documented in `docs/catalog-schema.md`; create/adopt/update/select/forget and
operation show/reconcile use `docs/file-custody-crud.md`. Other custody operations
remain pending. Package versions `0.1.0` are development versions with publishing
disabled, not registry release decisions.

Keep the existing root `styrene-identity` package. Introduce a lifecycle backend
and thin frontend packages incrementally. The backend owns identity operations
and their effect/recovery model. The CLI owns parsing and output; the UI owns
presentation; the Mesh host owns session and profile activation.

Use independent package versions with exact tested release manifests. Version
machine output, plugin contracts, and stored formats explicitly. Existing crypto
profiles retain their compatibility rules regardless of package version.

## Relationship to provider work

The provider proposal is the lower capability boundary. The lifecycle backend
consumes it and adds application-level operations, identity selection, persistence,
and retry/recovery. Do not move UI state into the crypto library or turn the
provider interface into a general application controller.

Review both catalogs together before implementing types. Standalone identity
operations can progress using disposable file custody without waiting for a Mesh
plugin loader. Mesh integration needs actual host evidence and a separate handoff.

## Initial vertical slice

First implement file-backed creation, canonical identity inspection, authenticated
encrypted backup, and non-overwriting restore through the backend and CLI. Then
implement the same workflows in a standalone UI using the same backend scenarios.
Inspecting a malformed backup, cancelling a prompt, retrying a completed restore,
and resuming after partial completion are first-slice behaviors.

Expand to hardware custody, purpose-specific keys, certificate/binding inspection,
and lifecycle planning after those operations have reviewed capability contracts.
Actual rotation, retirement, and revocation need operation-specific trust effects.

## Decisions to close

The [UI integration inventory](../../../docs/ui-integration-inventory.md) narrows
the presentation direction to shared Dioxus views, thin launchers, and typed
platform adapters. Its source was a dirty UI checkout, so migration still requires
an immutable implementation handoff. The subsequent UI design at
`a2baf72ecab685f324416f040879d356d53620b3` selects one compile-time optional
page, private registration, and a typed read-only client. This is committed design
evidence, not implemented plugin machinery. Broader machinery remains deferred.

1. Inventory published versions, registry ownership, release tags, and exact consumer
   dependencies; choose the first unused standalone version and approve pre-1.0 policy.
2. Approve independent versioning, MSRV classification, tag naming, prerelease
   distribution, and release approver/automation permissions.
3. Choose CLI executable name, command catalog, machine-schema support window,
   exit categories, protected-input behavior, and explicit private-export policy.
4. Confirm the Dioxus baseline and exact renderer/patch compatibility, first
   supported desktop targets, mobile rollout, and packaging/signing ownership.
   Standalone operation must not require Mesh.
5. Agree summary/capability DTOs and the first read-only request with the UI owner.
   Preserve scope cleanup, stale-result rejection, and enablement-only persistence.
   Before mutations, agree operation IDs, independent observation, cancellation,
   and blocked-disable behavior. Runtime installation remains deferred.
6. Define identity catalog storage, concurrent CLI/UI access, transaction locking,
   schema migrations, recovery journals, and downgrade behavior.
7. Select CI tooling for API comparison, dependency policy, artifact provenance,
   release notes, and platform acceptance; document unsupported checks.

## Verification and rollout

Use shared backend tests plus CLI integration and packaged-UI acceptance tests.
Compare effect outcomes rather than only rendered labels. Exercise clean local
storage, existing custody, wrong credentials, absent adapters, cancellation,
concurrent operations, and interrupted recovery with disposable identities.

Retain library-only default/minimal CI alongside application lanes. Before release,
verify packages in dependency order, install candidate CLI artifacts, launch
candidate UI artifacts, and record exact source/target/feature evidence. Test host
compatibility against exact Mesh revisions. Device-specific custody acceptance
remains separate from software conformance.

Documentation validation establishes planning consistency only. Do not mark the
foundation complete until its supported surfaces and release gates are implemented.
