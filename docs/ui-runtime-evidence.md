# Standalone UI runtime evidence — 2026-09-07

## Environment and boundary

macOS 26.6.2, `aarch64-apple-darwin`, Rust 1.97.0, Dioxus crates and `dx`
0.8.0-alpha.1. The app was built with the desktop renderer from this repository.
It did not launch Mesh or use a Mesh backend dependency. Existing unrelated host
applications and checkouts were not stopped or modified.

Captures are retained in the approved temporary evidence directory under
`opencode/identity-ui-20260907/`, not committed as source. Window bounds were
1435×949 points; the retained window captures are 3094×2122 pixels including
window capture framing. Dark application styling was used. No mobile or native
accessibility-frame acceptance is inferred from those dimensions.

## Iteration ledger

| Iteration | Changed input / hypothesis | Artifact and process | Capture and observation |
|---|---|---|---|
| Initial overview | New standalone app should load only the explicit disposable catalog | Source d3a7f3b plus dirty UI work; binary SHA-256 `c1c8265c0aab1030bc5a54243ab1d9e7c58b9b41c13f027e1fb99f39d4faefe9`; PID 81366, started 09:09:28 local | Window 3221; `overview.png`, SHA-256 `3d4b319ddde76c183e62f9aff95c3a8b3c572e192b3250a725ac8ada4e998124`; correct canonical identity rendered, but loading text remained stale |
| Status correction | Snapshot completion should replace stale reading text | Source 738cc26 plus dirty UI work; binary SHA-256 `3ba073ec730f4f35e832f1ed1a7e7aac4ebbbbc48d93023a56ad853ea2b10943`; PID 80810, started 09:43:04 local | Window 3389; `overview-loaded.png`, SHA-256 `177d781d299e9b13dfde0fde4b8cfdf2c8e116d07290e030a6e83f9df39ebd58`; visible build marker and “Public catalog loaded. Custody has not been unlocked.” confirmed |
| Mock host | `--extension-demo` should compose the same page under explicit fixture controls | Same status-correction artifact; PID 81226, started 09:48:18 local, verified executable path and arguments | Window 3395; `extension-demo.png`, SHA-256 `ce83c39a20575eac4a319313b7150f770a4a2f58bcbc3e681d341abd76b465bc`; fixture notices, host controls, mounted page, and generation 1 visible |

The generated app path was
`target/dx/styrene-identity-desktop/debug/macos/StyreneIdentityDesktop.app`
under the active Cargo target directory. Process command lines matched its
`Contents/MacOS/styrene-identity-desktop` executable and the recorded runtime args.
The source-visible marker was `dev 738cc26b884f-dirty` for the latter captures.
These are development-iteration captures; the final candidate build receives its
own immutable source marker and artifact hashes.

The disposable catalog was created through `idctl` and contained canonical ID
`27b70e9d856e62bc4d09a46c0c38eb38`, entry `identity-ui-fixture`. Its displayed ID
matched the CLI receipt. No operator identity was used as a fixture.

## Interaction limits

`System Events` rejected native automation with
`osascript is not allowed assistive access` (-1728). This is an actual blocked
native-control lane. No permission setting was changed or bypassed. Typed service
parity, SSR, and host-model tests cover software behavior; they do not prove native
keyboard, focus, VoiceOver, or full packaged mutation interaction.

The current UI-owner checkout is still independently active. Actual Mesh host
integration, persistence of its enable preference, Linux desktop, and mobile/device
acceptance remain separate handoff lanes.
