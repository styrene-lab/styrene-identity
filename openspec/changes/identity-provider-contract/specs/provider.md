# Identity provider - Delta Spec

## ADDED Requirements

### Requirement: Read-only overview preserves identity and interaction boundaries

The first host-client operation must return public identity/capability information
or typed unavailability without implicit unlock, root acquisition, mutation, or
CLI execution. Missing canonical identity must remain explicitly absent.

#### Scenario: Public summary requires authentication
Given an adapter cannot provide public identity without acquiring its protected root
When the client requests a read-only overview
Then it reports public identity unavailable or authentication required
And it neither acquires the root nor prompts for authentication

#### Scenario: Only transport identity is known
Given the host knows an RNS hash but has no canonical Styrene Identity ID
When it requests a read-only overview
Then the canonical Identity ID remains absent with a typed reason
And the RNS hash is not substituted as a canonical Identity ID

#### Scenario: Overview belongs to an invalidated scope
Given an overview request was made before the host changed session or disabled the extension
When the old request completes
Then the host rejects the result for its current view
And it does not interpret cancelled observation as cancellation of backend work

### Requirement: Capabilities and evidence are explicit

The provider must describe supported operations, availability, custody/export
properties, and interaction evidence without treating a tier or hint as proof.

#### Scenario: Unsupported capability
Given a provider that does not support encrypted backup export
When a consumer requests encrypted backup export
Then the result identifies unsupported capability
And no backup artifact is returned

#### Scenario: Availability does not establish authentication
Given an available provider whose credential is locked
When a consumer requests signing without permitted interaction
Then the result identifies authentication required or locked custody
And it does not report successful signing or observed user verification

#### Scenario: Missing hardware
Given the selected provider's token is disconnected
When a consumer requests signing
Then the result identifies provider unavailability
And no other provider is invoked

#### Scenario: Unknown interaction evidence
Given an adapter that cannot observe user verification
When a supported operation succeeds
Then its result reports user verification as unknown
And it reports that adapter's actual host-memory custody properties

### Requirement: Sensitive operations bind the expected identity

Signing, derivation, export, and restore requests must name an expected canonical
Identity ID. The provider must check the operation's identity before returning
sensitive output or committing custody changes. Public descriptor hash checks
must be distinguished from proof of possession and runtime binding evidence.

#### Scenario: Wrong identity selected
Given a request bound to identity A and a provider holding identity B
When the provider executes the request
Then the result identifies identity mismatch
And no sensitive output or custody mutation is released to the consumer

#### Scenario: Identity changes after discovery
Given discovery reports identity A and the provider changes to identity B before signing
When the consumer requests signing for identity A
Then the operation does not return a successful signature attributed to identity A
And the result identifies identity mismatch

#### Scenario: Invalid signing output
Given a provider returns a signature that fails verification with the expected identity key
When the boundary validates the signing result
Then it reports operation failure
And it does not report successful identity-bound signing

### Requirement: Secret export is capability-specific

The general provider contract must not return root bytes. Private derived-key
export requires a separately declared and reviewed capability. Public descriptors,
errors, and interaction evidence must not contain credentials or private material.

#### Scenario: Root export is unavailable
Given a consumer using the general provider contract
When the consumer inspects its request and result types
Then no operation or result exposes a root secret

#### Scenario: Private derivation without export capability
Given a provider supports public derivation but not private-key export
When a consumer requests private derived bytes
Then the result identifies unsupported capability
And no private derived bytes are returned

### Requirement: Failures do not trigger implicit fallback

The boundary must return structured failures from the selected provider. Another
provider requires an explicit request retaining the expected identity and applying
the caller's custody policy. Cancellation must distinguish no effect, completed
effect, and unknown completion when those outcomes differ.

#### Scenario: Authentication failure
Given the selected provider rejects authentication and another provider is available
When the consumer requests signing
Then the result identifies authentication failure
And the other provider is not invoked

#### Scenario: Cancelled interaction
Given an operation awaiting authentication with no committed effect
When the operator cancels the interaction
Then the result identifies cancellation with no committed effect
And no alternate provider is invoked

### Requirement: The contract remains independent and compatible

An isolated consumer must exercise the provider boundary without mesh, IPC, or UI
dependencies. Existing signer behavior and released cryptographic contracts must
remain compatible while the new surface is introduced.

#### Scenario: Isolated consumer
Given a mock provider and an isolated consumer depending on the proposed Identity feature set
When the provider conformance suite runs
Then its dependency graph contains no mesh runtime or Dioxus dependency
And it exercises the identity, capability, failure, and restore scenarios

#### Scenario: Legacy caller compatibility
Given existing signer callers and committed compatibility vectors
When validation runs with the new provider surface enabled
Then the existing callers retain their signer selection behavior
And committed derivation and wire vectors remain unchanged
