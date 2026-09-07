# Release policy - Delta Spec

## ADDED Requirements

### Requirement: Compatibility surfaces are versioned explicitly

Releases must classify package API, MSRV, CLI machine output, plugin contracts,
and stored/cryptographic formats separately. Package SemVer must not authorize
reinterpretation of released profile bytes.

#### Scenario: Breaking pre-1.0 API change
Given a reviewed incompatible change to a package at version 0.y.z
When a release version is selected
Then y is incremented and z is reset
And release notes identify the migration requirement

#### Scenario: Changed cryptographic bytes
Given a correction changes released canonical bytes or rejection classes
When a release is prepared
Then it includes a new explicit profile and compatibility rule
And a package version bump alone does not satisfy release validation

#### Scenario: Minimum Rust version increases
Given a package raises its declared MSRV
When a release is classified
Then the release announces the MSRV change
And it increments the minor version for stable packages or y for pre-1.0 packages

### Requirement: Releases use immutable validated evidence

Each package/application release must identify immutable source, dependency
resolution, feature/target checks, artifacts, and validation limits. Registry and
tag inventory must precede first-release version selection.

#### Scenario: First standalone release
Given the manifest contains an inherited package version
When the first standalone candidate version is chosen
Then registry versions and existing tags have been inventoried
And the selected version has not already been published

#### Scenario: Application-only release
Given a UI change requires no library release
When the UI is released
Then release evidence identifies its exact tested backend and library dependencies
And the library version is not changed solely to match the UI version

#### Scenario: Candidate changes after validation
Given a validated candidate whose source or manifest subsequently changes
When publication is requested
Then applicable validation runs against the changed candidate before publication
And published tags and artifacts are not replaced in place

#### Scenario: Interrupted multi-package publication
Given one dependency package published successfully and a later package failed
When publication is retried
Then completed publications are verified and retained
And the retry does not replace their published contents
