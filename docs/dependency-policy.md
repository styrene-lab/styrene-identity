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
- **RUSTSEC-2023-0071 (`rsa` 0.9.10)** remains an unresolved release gate for the
  optional SSH graph. StyreneAgent only emits/signs Ed25519 and now rejects RSA
  requests before custody access; a regression test proves that boundary. The raw
  audit remains nonzero. Do not silently accept this as an unqualified clean graph.
- BlueOak-1.0.0 is the existing minicbor license and is permitted for distribution.
- MPL-2.0 is used by the unchanged `option-ext` dependency in the desktop graph.
  Distribution must include the source/version notice below. The application does
  not modify that dependency's source files.

## Distribution notices

Retain the project MIT license and generate a dependency license listing for each
release target. Include this file with desktop/CLI distribution evidence.

`option-ext` 0.2.0 source is available at
<https://crates.io/crates/option-ext/0.2.0> and its versioned crate archive at
<https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate> under MPL-2.0.
Other dependency source versions and checksums are pinned in the accompanying
Cargo.lock and generated license inventory. System frameworks are not redistributed
as source by these packages.
