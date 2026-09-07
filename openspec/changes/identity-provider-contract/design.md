# Identity provider contract design

## Status and evidence

The full provider design remains proposed. The first read-only Rust API and an
isolated mock consumer are implemented for UI-owner review; see
[the API handoff](../../../docs/read-only-overview.md). Production sources and
plugin integration remain pending. The implementation is based on repository
revision `e1fcb0e014a9002cb82f45381c2096724047990f` plus local changes.

| Existing owner | Constraint on this design |
|---|---|
| `src/signer.rs`: `IdentitySigner`, `SignerChain` | Roots enter process memory; chain selection does not establish identity equivalence |
| `src/identity_id.rs`: `IdentityId` | Canonical ID derives from the canonical Ed25519 public key |
| `src/derive.rs`: `KeyDeriver` | Existing purpose, label, and epoch derivations must remain stable |
| `src/vault.rs`: `IdentityVault`, `EncryptedIdentityBackup` | Restore authenticates before creation; same-root restore is idempotent; conflicts preserve custody |
| `src/records/`, `src/repository_signing.rs` | Record types and bindings do not implement lifecycle orchestration or consumer authorization |
| `docs/plugin-boundary.md` | Identity owns contracts and its lifecycle product; Mesh owns its runtime and host presentation |

## Consumer inventory gate

The local library supplies candidate primitives, not evidence of actual consumer
calls. Before finalizing requests, inventory each affected consumer at an immutable
revision. Record call sites, enabled features, root/private-key use, identity
selection, interaction handling, and backup/runtime ordering.

Candidate operation mapping:

| Consumer need to verify | Local primitive | Proposed treatment |
|---|---|---|
| Display or select identity | `IdentityId`, public identity helpers | Public descriptor and explicit expected identity |
| Unlock and perform signing | `IdentitySigner` | Operation-scoped interaction and identity-bound signing |
| Obtain purpose-specific key material | `KeyDeriver` | Typed purpose requests; explicit private-export capability if justified |
| Inspect/export/import identity backup | `IdentityVault` | Encrypted artifacts and typed restore outcomes |
| Apply identity to a running profile | No runtime owner here | Consumer transaction and independently supplied session evidence |

The [UI integration inventory](../../../docs/ui-integration-inventory.md) records
mobile creation, restore, and export call sites from an inspected dirty UI tree.
They currently call daemon `MobileNode` contracts and carry request generations,
not explicit expected Identity IDs. The daemon's deeper call sites and resolved
Identity dependency remain unverified; obtain immutable consumer evidence before
finalizing migration.
Do not infer that opaque signing can replace a transport's private-key import.
Any required private-key export must be identified and reviewed explicitly.

## Proposed request and result model

### First host-client slice

The UI-owner design at `a2baf72ecab685f324416f040879d356d53620b3` requests
one public/read-only overview before lifecycle mutations. Propose one typed
request using an opaque backend selection handle, optional expected canonical ID,
and host-owned scope correlation. A selector is not an authorization credential.

The result should contain available canonical ID/public key, explicit binding
evidence, provider availability, capability summaries, and typed unavailable or
error reasons. Missing canonical identity stays absent; an RNS hash or destination
must not be substituted. ID/public-key consistency does not prove possession or
running-session binding. Reading must not unlock or acquire a root simply to
populate a public summary; adapters lacking public information report unavailable.

The concrete DTOs, capability identifiers, selection-handle lifetime, and error
mapping in `src/overview.rs` now await joint review. Use an identified mock for the spike; production reports
unsupported until a real provider exists. Exclude root access, arbitrary daemon
requests, raw paths, global app state, and CLI execution from the client contract.

Activation and view scopes belong to the host. Dropping observation cancels a read
where supported or ignores its late result; it does not imply backend cancellation.
No operation registry is needed for this read-only slice. Before mutations, agree
an operation ID and independently observable outcome, or a precise blocked-disable
rule. Mutation/restore requirements below remain later gates.

### Full provider boundary

Names below are conceptual. Rust names, feature gates, and serialization remain
review decisions.

- A provider descriptor identifies the provider instance and lists capabilities,
  availability, and custody/export properties. Availability can be unknown.
- An identity descriptor contains the canonical public key and `IdentityId`.
  A binding result identifies what was checked, by whom, and for which operation.
  Hash consistency alone is not proof of key possession or an active daemon binding.
- Each sensitive request includes the expected Identity ID, requested operation,
  interaction policy, and operation correlation identifier. Provider identity is
  resolved within the operation rather than trusted from cached enumeration.
- Successful results carry the resolved identity, operation-specific output, and
  interaction evidence. Signing results must be verified against the expected key
  before success is reported. Derived-key bindings need purpose-specific rules.
- Capability discovery distinguishes public derivation, signing, encrypted backup,
  restore, and any separately approved private-key export. Root export is absent.
- Presence and verification each distinguish requested, supported, and observed
  states. Unknown evidence stays unknown. Credentials enter through host callbacks
  or adapter mechanisms rather than diagnostic fields.

Start with an in-process Rust contract so an isolated consumer can test the
boundary. This does not decide eventual plugin execution or transport. New opaque
providers can report that deterministic derivation or portable backup is unsupported.

## Selection, errors, and cancellation

Bind the expected identity before releasing output or committing custody changes.
The implementation must avoid a check/use race when an adapter changes identity
between enumeration, authentication, and execution. Adapter strategy is a review
gate: a single acquired-root operation is possible for current software adapters;
opaque providers need an identity-bound operation handle or equivalent evidence.

Proposed error categories are unsupported capability, unavailable, locked or
authentication required, authentication failed, cancelled, identity mismatch,
invalid backup, identity conflict, custody unavailable, and operation failure.
Errors identify the failed phase and whether an effect occurred. Error strings
are diagnostic text, not machine policy. Preserve unknown completion after loss
of contact instead of claiming cancellation rolled back an operation.

The initial proposal performs one explicitly selected provider operation. It
does not automatically fall back. A caller can submit another request using the
same expected identity and an explicitly permitted custody policy. Existing
`SignerChain` semantics remain unchanged for legacy callers.

## Portable restore and consumer activation

Use the existing authenticated encrypted backup format. Parsing metadata alone
does not authenticate a backup. Before writing custody, authenticate the artifact
and verify its canonical identity against the request.

Proposed orchestration phases:

1. The consumer validates and stages its profile independently.
2. The provider authenticates the identity backup and checks destination custody.
3. The provider commits identity custody with exclusive creation, or reports that
   the same root is already present. Existing or inaccessible custody is preserved.
4. The consumer commits its profile and performs the required runtime transition.
5. The consumer verifies the running session's identity and records activation.

This sequence is not a cross-component atomic transaction. If a later phase fails,
report the completed phases and retained custody. Do not automatically delete a
restored secret. Retry must reconcile custody and active-session evidence before
continuing. A restart after custody commit must not be reported as activation.

Cancellation before commit prevents creation. Cancellation racing with commit
must report the actual effect, or an unknown outcome requiring reconciliation.
The consumer owns its durable journal, profile rollback, and runtime recovery.
The provider owns accurate custody outcomes. Journal format, commit boundaries,
and runtime binding evidence need joint consumer review.

## Compatibility and migration

Add the provider surface alongside `IdentitySigner`. A compatibility adapter may
acquire a root internally and must disclose that host-memory behavior. It must
not expose the root through the general provider result or reclassify custody.

Inventory root-dependent callers before choosing migration order. Migrate an
isolated mock consumer first, then hand off immutable Identity and consumer SHAs.
No signer deprecation or consumer migration is approved by this document.
Existing derivation vectors, record encodings, rejection classes, and legacy
backup readers remain unchanged.

## Open decisions required before implementation

1. Which actual consumer operations require private derived bytes, and which can
   use opaque signing? Define purpose-specific binding proofs and export policy.
2. What public Rust shape and feature gate preserve minimal builds? Decide typed
   requests versus capability-specific traits after the call-site inventory.
3. How are operation handles, concurrency, cancellation, and interaction callbacks
   represented across current adapters? Define handle lifetime and invalidation.
4. Which stable error categories and effect states can each adapter distinguish?
5. What evidence proves the runtime uses the restored identity? Define freshness,
   verifier ownership, and crash-recovery checkpoints with the consumer owner.
6. How are destination and backup credentials selected when protection differs?
   Current vault restore uses the configured provider; do not assume rewrapping.

## Validation

An isolated mock consumer will exercise wrong identities, unsupported operations,
interaction failures, identity changes during execution, and restore/restart
failures. Mock evidence is software-contract evidence only. Apple and Android
impact review and disposable-device acceptance remain separate handoff gates.

Run the repository's locked default, expanded-feature, and minimal checks when
implementation lands. Add focused scenario tests before broad validation. Verify
fixture immutability and dependency direction. OpenSpec validation checks artifact
structure; it does not establish API, software, or device acceptance.
