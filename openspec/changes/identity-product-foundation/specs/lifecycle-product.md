# Lifecycle product - Delta Spec

## ADDED Requirements

### Requirement: Frontends share an independent lifecycle backend

CLI, standalone UI, and Identity-side host integration must invoke shared backend
operations. The importable library and backend must remain independent of Mesh
and presentation dependencies.

#### Scenario: Library-only consumer
Given a consumer using only the existing Identity library feature set
When its supported minimal build runs
Then CLI, UI, and Mesh dependencies are absent from its resolved graph

#### Scenario: Frontend operation parity
Given equivalent inputs and disposable file custody
When the shared create-inspect-backup-restore scenario runs through each frontend
Then CLI and standalone UI produce equivalent backend outcomes and custody effects
And neither frontend implements independent cryptographic or backup-format logic

### Requirement: CLI supports predictable automation

The CLI must expose documented commands, versioned machine output, stable exit
categories, and explicit noninteractive behavior. Diagnostics and credentials
must not contaminate machine results.

#### Scenario: Noninteractive credential requirement
Given a command requires credentials and no permitted credential source is available
When it runs in noninteractive mode
Then it returns a documented authentication-required failure and exit category
And it does not wait for an interactive prompt

#### Scenario: Machine output with diagnostics
Given a command requests JSON output and emits a diagnostic
When the command completes
Then standard output contains a result conforming to the declared machine schema
And the diagnostic uses a separate channel without secret material

### Requirement: Identity UI operates without Mesh

The standalone UI must provide its supported lifecycle workflows without a Mesh
installation or daemon. Unsupported operations must be distinguishable from
successful local actions and remote trust effects.

#### Scenario: Standalone recovery
Given no Mesh installation or daemon and a valid disposable identity backup
When the operator restores through the standalone UI
Then supported local custody is restored through the shared backend
And no Mesh process is required or started

#### Scenario: Unsupported revocation
Given no integrated remote trust-update capability
When an operator requests remote revocation
Then the result identifies unsupported capability
And local deletion or record creation is not presented as completed remote revocation

### Requirement: Optional integration negotiates host compatibility

The Identity integration must declare contract versions and capabilities. The
Mesh host retains session/profile orchestration and remains usable without the
optional lifecycle component.

#### Scenario: Incompatible host contract
Given the host and Identity component have no supported contract version in common
When the host connects the component
Then integration reports incompatibility before identity mutation
And ordinary Mesh operation remains available

#### Scenario: Component absent
Given the optional Identity lifecycle component is not installed
When the Mesh host starts
Then supported Mesh operating controls remain usable
And advanced Identity operations are reported as unavailable

### Requirement: Shared state supports recovery and concurrency

Backend persistence must define schema versions, concurrent operation rules,
non-overwrite behavior, and recovery outcomes. A frontend must not infer rollback
or successful activation from an interrupted operation.

#### Scenario: Concurrent CLI and UI creation
Given CLI and UI target the same empty custody destination
When concurrent creation is attempted
Then at most one creation commits
And the other operation reports the conflict without replacing custody

#### Scenario: Unsupported local-state schema
Given stored application state uses a newer unsupported schema
When an older application opens it
Then it reports the incompatibility before mutation
And it preserves the stored state

### Requirement: Platform callbacks preserve operation evidence

Native document and authentication adapters must preserve request correlation
and distinguish presentation, completion, app unlock, and custody evidence.

#### Scenario: Stale document callback
Given a picker request belongs to an earlier identity selection generation
When its completion arrives after a new selection
Then the completion cannot start a restore for the newly selected identity

#### Scenario: Share interface presented
Given a platform adapter reports only that the share interface was presented
When the frontend displays the export outcome
Then it does not report confirmed artifact delivery

#### Scenario: App unlocked without custody evidence
Given device authentication unlocked the application but no provider operation ran
When the frontend displays custody status
Then app unlock is not reported as verified signing capability or hardware custody
