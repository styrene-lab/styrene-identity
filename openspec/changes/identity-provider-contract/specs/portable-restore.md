# Portable identity restore - Delta Spec

## ADDED Requirements

### Requirement: Restore authenticates before non-overwriting commit

The provider must authenticate a supported encrypted backup and verify its expected
identity before custody creation. It must preserve existing custody. A verified
same-root destination must produce an idempotent already-present result.

#### Scenario: Malformed backup
Given an invalid encrypted backup
When a consumer requests restore
Then the result identifies invalid backup
And destination custody is unchanged

#### Scenario: Backup authentication fails
Given a structurally valid backup and incorrect protection credentials
When a consumer requests restore
Then the result identifies authentication failure
And no destination custody is created

#### Scenario: Backup contains another identity
Given an authenticated backup for identity B and a request expecting identity A
When the consumer requests restore
Then the result identifies identity mismatch
And destination custody is unchanged

#### Scenario: Existing identity conflict
Given destination custody holds a different root
When a consumer requests restore
Then the result identifies identity conflict
And existing custody remains byte-for-byte unchanged

#### Scenario: Inaccessible destination
Given existing destination custody cannot be authenticated
When a consumer requests restore
Then the result identifies custody unavailable
And existing custody remains byte-for-byte unchanged

#### Scenario: Same-root retry
Given destination custody already holds the authenticated backup root
When a consumer retries restore
Then the result identifies already present
And destination custody is not rewritten

#### Scenario: Concurrent destination creation
Given another operation creates destination custody after restore validation
When the provider attempts exclusive creation
Then the result does not report a successful replacement
And the concurrently created custody remains unchanged

### Requirement: Restore effects and activation evidence remain distinct

The provider must report custody outcomes independently from consumer profile
commit and runtime activation. The consumer must verify the running identity
before reporting activation. Partial completion must be recoverable without
automatic deletion of restored custody or overwriting an existing identity.

#### Scenario: Cancellation before commit
Given a validated backup with no custody commit started
When the consumer cancels restore
Then the result reports cancellation with no committed effect
And destination custody is not created

#### Scenario: Cancellation races with commit
Given custody creation is in progress
When the consumer cancels restore
Then the result reports the actual custody effect or unknown completion
And it does not claim rollback without evidence

#### Scenario: Profile commit fails after restore
Given identity custody was restored and the consumer profile commit fails
When the consumer reports the import outcome
Then the outcome records restored custody and failed profile commit separately
And it does not report runtime activation or delete the restored custody

#### Scenario: Restart after custody commit
Given a process stopped after custody commit but before activation verification
When the consumer resumes the import
Then it reconciles existing custody and runtime state before reporting completion
And a same-root retry does not overwrite custody

#### Scenario: Running identity does not match
Given restore succeeded for identity A and the running session reports identity B
When the consumer verifies activation
Then activation is reported as unverified or failed
And restore success is not presented as proof of the running identity
