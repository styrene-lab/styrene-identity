# Identity security contracts - Delta Spec

## ADDED Requirements

### Requirement: Identity verification rejects weak-key forgery and mismatched attribution

General Identity signature verification must use strict Ed25519 checks. Verification
of a public identity object must check its own hash/key consistency. High-level
attestation verification must bind an independently expected canonical identity.

#### Scenario: SEC01 identity-point forgery
Given the identity point as public key and signature R with signature S equal to zero
When verification is attempted on two different messages
Then both forged signatures are rejected
And valid root-derived signature fixtures remain accepted

#### Scenario: SEC01 inconsistent public identity object
Given a valid signature and a public object whose hash belongs to another key
When the object's signature verifier runs
Then it rejects the inconsistent attribution

#### Scenario: Independently expected signer
Given an attestation valid for identity A
When it is verified for independently expected identity B
Then the result is false even though its self-asserted key has a valid signature

### Requirement: Recovery constrains directory identity and declared catalog transitions

Recovery must reject changed custody-parent bindings and after-snapshots that exceed
the declared mutation. Local authority files must not be writable by other principals.

#### Scenario: SEC02 substituted parent
Given a prepared creation whose parent was renamed and replaced with a symlink
When reconciliation runs
Then it rejects the changed binding without installing custody at the replacement

#### Scenario: SEC03 injected preference change
Given a rename journal was edited to also change the preferred identity
When reconciliation runs against its original before-state
Then the unintended transition is rejected without changing the catalog

#### Scenario: SEC05 writable authority file
Given a catalog authority file is group or other writable
When a mutation uses it
Then storage validation fails before custody/catalog mutation

### Requirement: Compact receipts preserve recovery and expose historical state

Completed v2 receipts must drop full catalog history and unnecessary encrypted
creation copies only after the custody copy is observed at the bound location.
Historical replay must be explicit. Legacy copies must not be silently erased.

#### Scenario: Completed v2 creation
Given matching encrypted custody and its committed catalog entry
When journal completion is recorded
Then the receipt contains neither a full catalog snapshot nor encrypted creation bytes

#### Scenario: Custody disappears before compaction
Given the catalog committed but custody disappeared before journal finalization
When reconciliation attempts completion
Then it preserves encrypted recovery material and reports failure requiring reconciliation

#### Scenario: Legacy completed receipt
Given a completed v1 receipt retaining encrypted creation material
When operation show or completed-request replay runs
Then retained recovery material is reported and preserved

#### Scenario: Unbound legacy pending custody
Given a pending v1 custody operation with no directory identity pin
When reconciliation would require a custody write
Then it fails without writing custody or discarding the recovery record

### Requirement: Backup evidence distinguishes payload authentication and format provenance

Backup reads must be bounded and non-mutating. Verification must derive identity
from the authenticated payload and honor an expected canonical ID. Inspection
and header classification must not be presented as cryptographic authentication.

#### Scenario: ID31 unauthenticated inspection
Given a structurally valid encrypted file
When backup inspect runs without credentials or a catalog
Then metadata is returned with payload authentication false and no claimed identity

#### Scenario: ID32 stripped legacy header
Given the same encrypted payload with and without its STID header
When both forms authenticate with the correct protection
Then both report the same canonical identity
And neither claims authenticated format-header provenance

#### Scenario: ID32 tampered payload or wrong expected identity
Given an altered authenticated payload or a request expecting another identity
When backup verify runs
Then it reports authentication failure or identity mismatch respectively
And the source file and custody remain unchanged
