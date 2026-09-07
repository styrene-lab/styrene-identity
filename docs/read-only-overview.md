# Read-only Identity overview client

## Implementation and handoff status

`styrene_identity::overview` implements the first in-process read-only contract.
It is available with default features disabled and adds no dependencies to the
library. The source trait and DTOs are ready for UI-owner review against the
extension design at `a2baf72ecab685f324416f040879d356d53620b3`.

No production custody source or UI adapter is connected. The isolated consumer
uses explicit synthetic public-key fixtures. This API is not a serialized host
protocol, plugin loader, or published release. An immutable Identity handoff
requires a reviewed commit before external consumers adopt it.

The lifecycle backend now provides `CatalogSnapshot`, a source of validated
public catalog metadata with no custody I/O during overview reads. See the
[catalog schema](catalog-schema.md). Its data is a snapshot, not live provider
availability or authenticated key possession. Host scope changes are required
when replacing snapshots.

## API map

| Type or function | Responsibility |
|---|---|
| `IdentitySelection` | Backend-issued, client-local opaque selector; not a path or authorization |
| `OverviewScope` | Host-issued observation generation; refresh on activation and selection changes |
| `OverviewRequest` | Selection, scope, and optional expected canonical Identity ID |
| `IdentityOverviewSource` | Supplies existing public information without custody access or interaction |
| `read_identity_overview` | Executes one source read and enforces the expected canonical ID |
| `PublicIdentitySummary` | Private fields preserve ID/public-key hash consistency through checked constructors |
| `PublicIdentityStatus` | Available public summary or typed unavailability, without transport-address substitution |
| `IdentityOverview` | Public identity, availability, declared root exposure, and capability summaries |
| `OverviewCompletion::into_result_for` | Returns an outcome only for the current complete request; `None` means disabled |
| `OverviewError` | Unavailability, mismatch, invalid public binding, cancellation, or operation failure |

Unknown availability and root exposure remain unknown. Capability summaries are
declarations, not authorization or evidence that an operation succeeded.
`PublicIdentitySummary` establishes hash consistency only. It does not validate
Ed25519 point encoding, prove key possession, or establish runtime binding.
Runtime binding remains separately verified host evidence.

For signed attestations, `SignedAttestation::verify_for` checks strict signature
validity and an independently expected canonical identity. An overview's hash
consistency is not a substitute for that check or consumer authorization.

## Host use

1. Obtain a selector from the backend's public catalog. Do not derive it from a
   storage path or assume its numeric value identifies an Identity authority.
2. Allocate a new observation scope when enabling or changing selection.
3. Construct an `OverviewRequest`; set `expected_identity` when the canonical ID
   is known independently. Leave it absent for discovery.
4. Call `read_identity_overview` with a real public-only source or an explicit mock.
5. Consume completion against the current request, not a captured obsolete request.
   Pass `None` after disabling. Discard stale outcomes, including errors.

Do not reuse selectors for different identities during a client's lifetime. Do
not reuse scope values while an earlier request could complete. Replace the scope
when replacing a source instance, even if its selectors have the same values.
The API cannot infer the host's current navigation state or prevent incorrect
token reuse. Host lifecycle tests must exercise these rules.

The read does not request authentication. A source that cannot obtain public
metadata without a root returns unavailability or authentication required.
When an expected ID is supplied, missing identity information produces an error
rather than an apparently bound successful overview. Without an expected ID,
typed absence can be displayed alongside other public provider information.

There is no automatic fallback. Implementations must honor the selected provider.
The trait excludes mutation methods and credentials, but trusted implementation
code can violate its documented behavior. This is not an execution sandbox.

## Before adding mutations

Page unmount and disabled observation do not cancel backend operations. This
slice has no operation registry. Before adding one lifecycle mutation, agree
its CLI/service acceptance, operation ID, independent outcome route, cancellation,
and restart policy. The host must retain observation after disable or block disable
with a specific reason. Do not generalize that machinery from a read-only request.

## Conformance and limits

`tests/overview_consumer.rs` exercises the public API. The isolated workspace at
`tests/minimal-overview-consumer/` reuses the same corpus with default features
disabled. Its separate committed lockfile is intentional consumer evidence.
CI runs this workspace explicitly; it is not a root workspace member.

The six tests cover matching public identity with unknown custody, mismatched
identity without retry, unavailable discovery versus expected-identity failure,
stale/disabled observations, unknown selection, and inconsistent claimed binding.
They do not attest a real source's secret handling, host subscription cleanup,
navigation, cancellation of native interaction, or hardware custody.

Run:

```sh
cargo test --locked --test overview_consumer
cargo test --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml
cargo tree --locked --manifest-path tests/minimal-overview-consumer/Cargo.toml --edges normal
```

The isolated graph must exclude Mesh, Dioxus, CLI, and file-signer dependencies.
The existing Apple target dependencies still appear on macOS; minimal builds are
not `no_std` or a claim of platform independence for every existing module.
