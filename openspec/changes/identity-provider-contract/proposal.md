# Identity provider contract

## Intent

Define a narrow provider boundary for Identity consumers before implementing an
optional lifecycle-management component. Existing `IdentitySigner` callers can
retrieve roots. They cannot infer identity equivalence, interaction evidence, or
hardware custody from signer tiers or availability hints.

Status: proposed for review. These requirements describe future behavior, not
current API guarantees. The accepted product direction remains in
[the plugin boundary](../../../docs/plugin-boundary.md).

The companion [product foundation change](../identity-product-foundation/proposal.md)
places this provider boundary beneath a shared lifecycle backend used by the CLI,
standalone Identity UI, and optional Mesh integration. This change remains focused
on capability and custody contracts; the companion owns application and release work.

## Scope

- Public identity, capability discovery, availability, and interaction evidence.
- Expected-identity checks for signing, derivation, and encrypted backup operations.
- Structured failures, cancellation, and explicit provider selection.
- Identity restore outcomes and the consumer-owned activation/recovery boundary.
- An isolated mock consumer and a migration plan for existing signer callers.

Plugin packaging, ABI, process isolation, discovery transport, UI, remote
revocation, and runtime implementation are outside this change. Existing
derivation profiles and backup bytes remain authoritative.

## Success criteria

- Actual consumer calls are inventoried against exact consumer revisions before
  the operation surface is approved.
- Requests and results distinguish claimed identity, checked binding, provider
  availability, observed interaction, and active-session evidence.
- No general provider operation exports a root or silently changes identity.
- Mock-consumer tests cover every delta scenario without mesh or UI dependencies.
- Migration preserves existing signer behavior and committed compatibility vectors.
- Review resolves the open decisions in `design.md` before public API implementation.
