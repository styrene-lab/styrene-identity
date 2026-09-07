# Identity 0.4.0-rc.1 handoff

## Source and review

- Source: `eaa5223ddafa28a5de88caa6e132bf6df1cc3eb1`.
- Annotated Git tag: `styrene-identity-v0.4.0-rc.1`.
- Review: <https://github.com/styrene-lab/styrene-identity/pull/1> (draft).
- Candidate release: <https://github.com/styrene-lab/styrene-identity/releases/tag/untagged-460fbef0a8924551f22c>
  (draft/prerelease, not crates.io publication).

The source includes core Identity `0.4.0-rc.1` and independent lifecycle, CLI,
shared UI, and desktop packages at `0.1.0-rc.1`. Use the full Git SHA in consumers;
neither a branch name nor registry 0.3.2 identifies these contracts.

## Candidate artifacts

Packaging ran from a clean tree with Rust 1.97.0 on `aarch64-apple-darwin`.
All generated SHA256SUMS entries verified. The distribution contains source,
optimized CLI, macOS app, toolchain/source manifest, licenses, and third-party notices.

| Artifact | SHA-256 |
|---|---|
| Combined candidate archive | `d2c4f7ae74b5bd0b0c43cd37026d36b38131596f4feaded1b72c28e8ce6ac068` |
| Root source crate | `a0c04c748c1f325511adb53fe0b152b65a847b4e70029c3642587c8bf4ae99f4` |
| Optimized idctl | `4247d5b1f6af351a1e740783da211d8d2b5257f62ba760ba1012a59a8f3ad2af` |
| macOS app zip | `0d597f1707a37468e2d0904d30689e2c8efee363b1d02455e19178736f1c0cab` |
| Extracted macOS executable | `ba15ea57c847f7195505995068fa71e0a614adf18885f6725f9ecb229774b791` |

GitHub's uploaded asset digest matches the combined archive. The app bundle is
ad-hoc sealed and passes `codesign --verify --deep --strict`; this is not Developer
ID signing or notarization. The first unsealed packaging attempt was retained as
failed evidence and replaced by this new immutable source/build, without moving a tag.

## Executed acceptance

- Complete local software matrix: 245 workspace tests and 3 doctests; 224 expanded
  root tests and 3 doctests; minimal, lint, formatting, and rustdoc gates passed.
- Target-filtered core, repository, PKI, CLI, and macOS desktop dependency profiles passed.
- The packaged optimized CLI executed create → export → restore → verify with
  separate protection roles and disposable identity
  `7adfe69f77240ec0ae3abc0c5467ccde`.
- The extracted sealed app launched as PID 95012 at 10:33:49 local on macOS 26.6.2.
  Its executable path matched the extracted archive and the hash above.
- Window 3486, 800×632 points, rendered the full source marker and disposable
  catalog ID `27b70e9d856e62bc4d09a46c0c38eb38`. Capture `candidate-overview.png`
  is 1736×1400 pixels, SHA-256
  `ae1878dafe59ee7f72d19f34955924493c88a1c3fb578525d79c66b016984542`.

Capture files and full machine-local evidence remain in the approved temporary and
Cargo target evidence directories. They are not product source.

## Consumer adoption

The shared `styrene-identity-ui::IdentityPage` accepts a typed `OverviewClient`
and `OverviewRequest`. `IdentityExtension` supplies the concrete one-page lifecycle
model. The standalone application owns lifecycle forms and its operation worker;
the Mesh host still owns navigation, session selection, enablement persistence,
and runtime identity reconciliation. No active UI-owner worktree was edited.

New artifact-capable stores reject older writers through a version-three guard.
Legacy recovery migration requires explicit expected identity and protection roles.
Completed operation replay is historical, never proof of current custody or runtime.

## Unclosed release gates

- Native-control automation/keyboard/focus testing needs Assistive Access; the
  attempted automation returned -1728. No permission was changed or bypassed.
- Actual Mesh host adoption and its native lifecycle scenarios remain with the UI owner.
- Linux desktop, mobile, physical token/device, and power-loss acceptance are not claimed.
- The optional SSH dependency graph retains RUSTSEC-2023-0071; RSA private operations
  are not exposed and RSA requests are rejected before custody, but registry release
  still needs an explicit advisory decision.
- Registry publication is not authorized by this candidate artifact or tag.
