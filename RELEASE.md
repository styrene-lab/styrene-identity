# Versioning and release workflow

## Status and authority

This is the repository-wide release policy for the
[product foundation change](openspec/changes/identity-product-foundation/proposal.md).
Current CI and commands are documented in `CONTRIBUTING.md`. `scripts/validate.sh`,
`scripts/check-dependencies.sh`, and `scripts/package-candidate.sh` implement the
software and candidate-artifact gates. The release-candidate workflow uploads
evidence; it never publishes to crates.io as a side effect.

The first standalone candidate was `0.4.0-rc.1`. The follow-up candidate is
`0.4.0-rc.2`; application/backend/UI packages use independent `0.1.0-rc.2`
candidate versions. Registry inventory confirms published
Identity versions through `0.3.2`. See `docs/release-inventory.md`. These are Git-only
candidate versions until registry approval and remaining acceptance gates close.

The existing [repository-signing compatibility policy](COMPATIBILITY.md) remains
authoritative for its profile. Package SemVer cannot override immutable
cryptographic profiles or make persisted bytes safe to reinterpret.

## Separate compatibility surfaces

| Surface | Version and compatibility rule |
|---|---|
| Importable Rust packages | SemVer per package, including documented features and supported Rust versions |
| CLI automation | Versioned machine schema and documented exit categories; command compatibility recorded with CLI releases |
| Standalone application | Application version plus supported backend and local-state schema versions |
| Plugin/host contract | Explicit protocol or API compatibility range and capabilities, independent of application display version |
| Derivation, signed records, backups | Explicit profile/format versions with readers, vectors, and migration rules |

Packages version independently. An application release records an exact
tested dependency set rather than forcing crypto-only consumers to upgrade with
every UI change. Co-released packages can share a milestone, but each gets its own
version, notes, and compatibility assessment.

## SemVer classification

For packages at `1.0.0` or later:

- Patch: compatible corrections with unchanged public and persisted contracts.
- Minor: additive compatible APIs or capabilities; supported callers keep working.
- Major: removal or incompatible change to APIs, features, defaults, documented
  behavior, or supported environments.

For `0.y.z`, compatible additions and fixes increment `z`; incompatible public
changes increment `y` and reset `z`. Every release still documents compatibility.
Pre-1.0 status does not permit silent changes to cryptographic profiles or backups.
Stable `1.0` requires explicit review of the supported API, error model, feature
matrix, recovery contracts, and downstream migration evidence.

Raising a package's minimum supported Rust version is a compatibility event:
increment the minor version for stable packages, or `y` for pre-1.0 packages, and
announce it. The development toolchain pin can advance without raising MSRV only
when CI still validates the declared MSRV. Remove the current coupling between
latest-stable development policy and consumer MSRV as part of tooling work.

Assess exported types, trait changes, enum exhaustiveness, feature combinations,
machine fields, exit codes, and supported targets before classifying a change.
Security fixes still need this assessment. If they alter canonical bytes or
rejection classes, use an explicit new profile and migration rule.

## Release procedure

1. Inventory the source revision, dirty state, registry versions, tag namespace,
   supported consumers, and dependency licenses/advisories.
2. Classify each package and external contract change. Write release notes with
   migration steps, deprecated behavior, and validation limits.
3. Select unused versions. Update package versions, internal dependency ranges,
   and lockfiles together. Test publishable packages in dependency order.
4. Run format, lint, documentation, tests, minimal-feature, supported-target,
   dependency-policy, conformance, and package verification gates. Compare public
   APIs against the last supported release, using automated checks where supported.
5. Validate downstream consumers against the exact candidate revision. Preserve
   the latest/previous profile-bearing lanes required by `COMPATIBILITY.md`.
6. Build CLI/UI artifacts for declared platforms from the candidate revision.
   Record toolchains, features, target triples, dependency resolution, checksums,
   and platform signing/notarization evidence where applicable.
7. Review a release candidate and its evidence. Tag the validated immutable source
   with a package-qualified tag such as `styrene-identity-vX.Y.Z`; never move a
   published release tag. Decide prerelease registry versus Git-only distribution
   explicitly before the first candidate.
8. Publish verified packages in dependency order and attach verified application
   artifacts and release notes. Promotion must use the tested source/artifacts;
   a changed version or manifest requires renewed applicable validation.
9. Verify registry/package installation and artifact checksums, then hand off exact
   revisions and resolved versions to consumers. Record incomplete publication
   separately from successful packages so retries cannot replace published bytes.

The candidate automation does not authorize unattended registry publication.
Publishing credentials and application signing are scoped release configuration,
not prerequisites for ordinary library tests.

## Support and recovery

Use immutable Git revisions until reviewed registry releases are available. Do not
infer compatibility from a branch name or matching UI version. Maintain an explicit
supported package/host/platform matrix in each release's evidence.

Define supported CLI machine-schema and plugin-contract versions before their
first release. Negotiation must reject unsupported contracts before mutation.
Existing repository-signing support windows remain in `COMPATIBILITY.md`; they
are not automatically a support promise for every application or adapter.

For a defective release, issue a new version and document affected contracts.
Yank a crate when warranted rather than replacing it. A binary rollback must
check local-state and backup compatibility first. A root or trust transition
cannot be undone merely by installing an older executable.

## Required release evidence

- Package/application versions, full source SHA, immutable tag, and release notes.
- Resolved dependencies, toolchains, MSRV, features, targets, and exact commands.
- Fixture digests, API comparison, package verification, and consumer revisions.
- Artifact checksums and available signing/provenance evidence.
- Supported compatibility ranges, migration/recovery steps, and unexecuted lanes.

Compilation is not custody-device acceptance. Report software, host integration,
and physical-device results separately.
