# Release inventory: 2026-09-07

## Registry and Git

The crates.io API lists `styrene-identity` versions 0.1.0, 0.1.1, 0.2.0, 0.3.0,
0.3.1, and 0.3.2. Latest is 0.3.2, published 2026-05-01 by `cwilson613`; the
owners endpoint lists that account. Registry metadata still points to styrene-rs.
The independent Git repository has no remote tags as of this inventory.

The 0.3.2 archive was read from the canonical static.crates.io distribution URL.
Its SHA-256 matches the registry metadata:

```text
ebe24b322ff5d7b5d4aec64424668100e0cc839fef412d791d8849fb182d2796
```

The published `src/identity.rs` uses permissive `VerifyingKey::verify` and compares
`PublicIdentity::verify_hash` against its cached hash field. This confirms affected
registry source, not deployment reachability. The local hardening fixes both strict
verification and actual public-key attribution. Earlier published versions have
not yet been inspected individually.

## Checkpoint

The first clean checkpoint is `764a9f349310bdeee720ab93fe25fe8b5ad2d66d`, with
implementation at `4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a` and core hardening
at `629d0a0c55abd59d12d29e8af6f58b89fb1ecac5`.
`cargo package --locked` passed from the clean checkpoint on Rust 1.97.0,
`aarch64-apple-darwin`, packaging and compiling the root library. This is not
registry publication or application/device acceptance.

## Dependency audit

`cargo audit --json` used advisory DB revision
`5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` (updated 2026-09-02).
Initial findings:

| Dependency | Finding | Follow-up |
|---|---|---|
| spin 0.9.8 | Yanked | Updated to compatible 0.9.9 |
| anyhow 1.0.102 | RUSTSEC-2026-0190, mutable downcast unsoundness | Updated to 1.0.103 |
| rand 0.8.5 / 0.9.2 | RUSTSEC-2026-0097, custom-logger reentrancy | Updated to 0.8.6 / 0.9.3 |
| rsa 0.9.10 | RUSTSEC-2023-0071, private-operation timing leak | Unresolved; no patched version in the advisory |

RSA is pulled through ssh-agent-lib/ssh-key. StyreneAgent emits and signs Ed25519
keys only; it does not hold RSA private keys. Review the exact reachable parser and
operation surface before a release exception. Do not claim an unqualified clean
audit or silently add an ignore rule. Default CLI/file-custody operation and the
optional SSH profile need distinct dependency evidence.

After the compatible updates, the same audit DB reports only RUSTSEC-2023-0071,
with no informational or yanked warnings. The audit exit remains nonzero; no
ignore rule was added. The updated graph passed
`cargo test --workspace --locked --features repository-signing,ssh-agent,pki,age-format --quiet -- --test-threads=2`
on `aarch64-apple-darwin`: 266 unit/integration tests plus 3 doctests passed,
4 doctests ignored. An earlier run hit the harness's 120-second command limit;
the complete bounded-concurrency run finished within the 300-second limit.

## First release candidate direction

Use a Git-only `0.4.0-rc.1` library candidate after the remaining work and checks.
This crosses the pre-1.0 compatibility boundary from the published 0.3 line,
including explicit toolchain/MSRV and verifier-rejection changes. Application
packages version independently. Recheck registry versions before assigning the
candidate; this document does not reserve or publish a version.

The prior registry 0.3.2 does not contain the repository-signing profile, so it
cannot substitute for a previous profile-bearing release lane. That lane remains
unavailable until two profile-bearing releases exist.

Registry publication still requires the completed release evidence, a resolved
advisory decision, and release approval. Independent host/device lanes and actual
Mesh consumer acceptance remain separate from local package compilation.

## Candidate implementation update

Core/default, repository-signing, CLI, and macOS desktop target-filtered dependency
checks now pass under `deny.toml`. The optional SSH graph remains separately gated
by the RSA advisory. Desktop adds the explicitly recorded unmaintained `paste`
notice and an MPL-2.0 source notice; see `docs/dependency-policy.md`.
The current candidate versions and final software results are recorded in
`docs/release-candidate-status.md`.
