# Dependency policy and release profiles

Checks use the resolved target/feature graph, excluding development dependencies.
The complete lockfile audit remains evidence, but is not equivalent to the graph
of every shipped artifact.

## Required checks

Run `cargo deny --locked --exclude-dev --target <triple> --manifest-path <manifest>`
with the candidate's features and `check advisories licenses sources`. Keep raw
`cargo audit --json` output alongside the profile results.

- Core/default and repository-signing library profiles must pass without vulnerability ignores.
- CLI/file-custody profiles must pass without vulnerability ignores.
- macOS desktop uses the exact pinned Dioxus version and a target-filtered check.
- Linux desktop remains unaccepted until the GTK/Wry dependency and runtime lanes
  are reviewed. Headless Linux library/CLI checks do not establish desktop acceptance.

## Recorded decisions

- **RUSTSEC-2024-0436 (`paste`)** is an unmaintained-proc-macro notice, not a reported
  runtime vulnerability. Its use is transitive through pinned Dioxus image codecs.
  The notice is explicitly allowed in `deny.toml`; renderer dependency migration
  must revisit it. No vulnerability advisory is suppressed by this allowance.
- **RUSTSEC-2023-0071 (`rsa` 0.9.10)** is classified as non-applicable to the locked
  StyreneAgent surface after source review and regression testing. It emits/signs
  only Ed25519, rejects RSA requests before custody access, and rejects both plain
  and constrained private-key imports. The dependency's parser may decode a
  client-supplied key, but no server-held RSA key is generated, retained, signed
  with, or decrypted with. This does not claim that the upstream vulnerability is fixed.
  The exception is limited to rsa 0.9.10 through ssh-key 0.6.7 and ssh-agent-lib
  0.5.2, expires after 2026-12-07, and must be revisited if those versions or the
  exposed operation surface change. The dependency script verifies these versions,
  reruns the rejection/import test, and checks that RSA is introduced only through
  the allowed wrapper. Only that guarded SSH invocation generates a temporary
  exception configuration. Direct `cargo deny`, other release profiles, and raw
  `cargo audit` still report the advisory if their graphs contain the affected RSA crate.
- BlueOak-1.0.0 is the existing minicbor license and is permitted for distribution.
- MPL-2.0 is used by the unchanged `option-ext` dependency in the desktop graph.
  Distribution must include the source/version notice below. The application does
  not modify that dependency's source files.

The SSH dependency check requires `jq` in addition to Cargo and cargo-deny.

## Distribution notices

Retain the project MIT license and generate a dependency license listing for each
release target. Include this file with desktop/CLI distribution evidence.

`option-ext` 0.2.0 source is available at
<https://crates.io/crates/option-ext/0.2.0> and its versioned crate archive at
<https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate> under MPL-2.0.
Other dependency source versions and checksums are pinned in the accompanying
Cargo.lock and generated license inventory. System frameworks are not redistributed
as source by these packages.
