# Identity lifecycle plugin: direction and next gate

Status: design direction accepted; plugin/provider integration is not implemented.
This document preserves the product decisions needed by an agent in this checkout.
It does not define an approved ABI or authorize a particular packaging mechanism.

## Product split

The mesh application needs a small operating surface: show the active Identity
and its binding evidence, select an existing identity/profile, unlock when needed,
and expose supported portable import/export operations with clear outcomes.

Full lifecycle management belongs in an optional Identity management component:
create identities, manage custody backends, derive purpose-specific keys, inspect
bindings and certificates, and plan rotation, recovery, and retirement. Hardware
setup and advanced verification controls belong there. The mesh must remain usable
without installing those advanced controls.

Identity owns reusable contracts. The mesh consumes those contracts and owns its
runtime orchestration. Presentation can depend on Identity capabilities; Identity
must not depend on the mesh application or Dioxus. Plugin packaging, execution
isolation, discovery, and independent ownership policy remain decisions to make.

## Existing primitives and missing orchestration

| Existing here | Still needs integration/design |
|---|---|
| `IdentityId`, public keys and derivation | Active-node binding evidence exposed to an operator |
| `IdentitySigner`, custody adapters, `SignerChain` | Narrow provider interface with explicit capabilities and identity binding |
| `IdentityVault`, encrypted backup parsing/restore | Portable profile + identity transaction and running-session reconciliation |
| Runtime certificate and lifecycle transition records | Issuance policy, trust distribution, rotation/revocation workflows |
| Repository authority bindings and conformance corpora | Consumer authorization policy and rollout |

Do not expose `IdentitySigner::root_secret()` directly as the general plugin API.
It is a root-exporting interface. Current adapters perform software derivation and
signing after retrieving that root. An opaque hardware signing provider would
need a different capability contract and compatibility assessment.

## Verification settings must express evidence

Separate these questions in the provider result and UI:

- Which canonical Identity ID and public key does this operation use?
- Was that binding cryptographically checked, and by whom?
- Is the provider present, unlocked, or actually able to complete the operation?
- Was user presence or user verification requested, supported, and observed?
- What secret enters host memory? Which material can be backed up or exported?
- What happens on cancellation, authentication failure, disconnection, or restart?

A tier name is not proof of any answer. Apple Keychain stores a retrievable root;
Android Keystore wraps a root with an AES key; YubiKey FIDO2 derives a root returned
to the process. These are not on-device Styrene Ed25519 signing implementations.
Credential-manager Tier C is an enum value, not an implemented adapter.

`SignerChain` skips backends whose availability hint is false. It selects the
first remaining backend and returns that backend's result, including errors.
It neither retries operation failures nor checks same-identity equivalence.
Any proposed fallback must bind the expected Identity ID and explicitly define
whether a lower-custody path is permitted. It must not silently change identity.

## Lifecycle semantics to settle

“Generate derived key” normally computes deterministic bytes for a purpose and
label/epoch. Repeating those inputs returns the same key. Deleting a local export
does not revoke that key or prevent its rederivation. Rotation requires a defined
new input or root and a trust update appropriate to the consuming protocol.

Distinguish removing an app reference, deleting a stored secret, retiring an
identity, and revoking a remote authorization. Editing a display name changes
metadata; replacing a root changes identity. Recovery must state which bindings,
profiles, and device credentials it restores and which need re-enrollment.

## Next gate

Before implementing plugin UI or moving more code:

1. Inventory the minimal mesh operations against actual consumer calls.
2. Propose provider requests/results for public identity, signing/derivation,
   capability discovery, availability, user interaction, and structured failures.
3. Specify expected-identity checks, secret/export boundaries, and fallback policy.
4. Define portable import ordering, cancellation, non-overwrite behavior, rollback,
   and verification of the identity actually used by the running session.
5. Add an isolated mock consumer proving the boundary without a mesh/UI dependency.
6. Review the contract and a migration path for existing `IdentitySigner` callers.

Acceptance must cover wrong-identity selection, unsupported capabilities, locked
or missing hardware, cancellation, authentication failure, malformed backups,
existing-identity collisions, and restart/recovery. Hardware claims require device
evidence. A contract test cannot attest hardware custody or enforce remote revocation.
