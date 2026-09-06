# Integration context for an independent checkout

## Repository ownership

| Repository | Responsibility |
|---|---|
| `styrene-lab/styrene-identity` | Derivation, canonical Identity ID, signing contracts, custody adapters, records, encrypted identity backup |
| `styrene-lab/styrene-rs` | Daemon, TUI, mesh protocols, RNS/LXMF transport, persistence, runtime/profile orchestration, IPC |
| `styrene-lab/styrene-ui` | Dioxus applications, presentation, platform UI adapters |
| comms-lab coordination checkout | Host assignment, exact consumer revision pairs, integration evidence |

The Identity repository was extracted to permit independent development and a
future optional lifecycle-management plugin. Extraction did not create a plugin
loader, UI, or new custody implementation. [EXTRACTION.md](../EXTRACTION.md)
records source provenance. The MIT notices remain intact; unrelated monorepo
LICENSE-only commits were excluded from filtered history.

An agent working here can complete library changes and software validation without
any of the other checkouts. Consumer acceptance requires a separate handoff, not
assumptions about sibling paths or installed binaries.

## Distinguish the identifiers

| Term | Meaning |
|---|---|
| Root secret | Private 32-byte input to the derivation hierarchy; never a UI identifier |
| Styrene Identity ID | First 16 bytes of SHA-256 of the canonical Styrene Ed25519 signing public key; implemented by `IdentityId` |
| RNS identity hash | Reticulum identifier for transport identity keys; not a substitute for the Styrene Identity ID |
| LXMF delivery destination | Messaging-service address derived using RNS destination rules; not the root identity |
| Display name, short name, icon | Public metadata; their absence does not prevent RNS connectivity |
| Repository signer binding | Identity-issued authority for an epoch-indexed repository signing key; distinct from Git commit signing |

The RNS signing seed and canonical Styrene signing seed share the existing
hierarchy's alias. That does not make the RNS hash and Styrene Identity ID
interchangeable: they identify different public inputs and contracts.

An active daemon session, connected interface, reachable destination, valid
signature, recognized Identity binding, and Fleet authorization are separate
facts. The application must report evidence for each. Identity metadata cannot
be inferred from transport connectivity.

## Portable profiles and identity

The intended operator experience is to move an app, its mesh profile, and its
identity with clear recovery behavior. Identity owns encrypted identity backup
formats and validation. The mesh consumer owns interfaces, profile persistence,
runtime shutdown/startup, and application of the restored identity.

`IdentityVault` provides file initialization, unlock, backup, and encrypted
backup inspection/restore. Those primitives do not make a profile import atomic,
transfer platform credentials, migrate remote trust, or establish that a running
daemon uses the restored identity. The consumer must prove those outcomes.

## Cross-platform handoff

The current lab assigns Apple builds and device evidence to Wilson's MacBook;
Nucleus owns shared integration and Linux/Android evidence. This is coordination
context, not a requirement to access either host for library development. Resolve
current endpoints and checkout locations through the lab inventory when assigned
a consumer integration task; do not embed private machine paths here.

For shared custody, backup, or identity changes, include both Apple and Android
impact in the handoff. Record Identity SHA, consumer SHA(s), target, features,
artifact, and executed checks. Mark missing device lanes unexecuted. A Linux or
macOS software pass cannot close mobile device acceptance.
