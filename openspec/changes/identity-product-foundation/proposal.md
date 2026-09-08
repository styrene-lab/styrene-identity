# Identity product and release foundation

## Intent

Establish Identity as both an importable library and a self-contained lifecycle
product. CLI, standalone UI, and optional Mesh integration must share backend
operations. Establish explicit SemVer and release contracts before distributing
these surfaces independently.

Status: proposed. Product ownership and standalone operation are the requested
direction; package names, version policy details, UI framework, host mechanism,
and platform rollout require review. CLI ID01–ID08 and file-catalog mutation
recovery are implemented in the shared backend. Advanced custody workflows, UI,
and release automation remain pending. See `verification.md` for checks and limits.

## Scope

- Per-package SemVer, MSRV policy, profile stability, release gates, and evidence.
- Incremental workspace structure preserving the existing importable package.
- Shared lifecycle backend, robust CLI, standalone UI, and Identity-side plugin bridge.
- Shared behavioral acceptance, compatibility negotiation, and recovery contracts.

This change depends on the sibling `identity-provider-contract` change for
identity binding, custody capabilities, errors, and portable restore semantics.
Mesh runtime implementation, remote trust administration, and automatic migration
of external consumers remain with their owners.

## Success criteria

- Library consumers build without CLI, UI, or Mesh dependencies.
- CLI and standalone UI complete the initial file-custody lifecycle slice without Mesh.
- Shared backend scenarios produce equivalent effects across CLI and UI.
- Optional Mesh integration advertises supported capabilities and rejects incompatible contracts before mutation.
- A release candidate reproduces package and application artifacts with explicit compatibility and migration evidence.
- Publication, device acceptance, and unexecuted platforms have distinct recorded outcomes.
