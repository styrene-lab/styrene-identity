#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Candidate packaging requires a clean committed tree." >&2
  exit 2
fi
revision="$(git rev-parse HEAD)"
target="$(rustc -vV | sed -n 's/^host: //p')"
metadata="$(cargo metadata --locked --no-deps --format-version 1)"
target_dir="$(jq -r .target_directory <<<"$metadata")"
version="$(jq -r '.packages[] | select(.name=="styrene-identity") | .version' <<<"$metadata")"
out="$target_dir/release-evidence/$revision/$target"
mkdir -p "$out"
for profile in core repository pki cli; do
  scripts/check-dependencies.sh "$profile" "$target" 2>&1 | tee "$out/dependencies-$profile.log"
done
cargo package --locked -p styrene-identity
cp "$target_dir/package/styrene-identity-$version.crate" "$out/"
cargo build --release --locked -p styrene-identity-cli
cp "$target_dir/release/idctl" "$out/"
cargo metadata --locked --format-version 1 --filter-platform "$target" > "$out/dependency-metadata.json"
cargo deny --locked --exclude-dev --target "$target" --manifest-path apps/cli/Cargo.toml list --format json --layout crate > "$out/licenses-cli.json"
scripts/collect-notices.sh "$out/dependency-metadata.json" "$out/licenses-cli.json" "$out/THIRD-PARTY-NOTICES-CLI.txt"
if [[ "$(uname -s)" == Darwin ]]; then
  scripts/check-dependencies.sh desktop "$target" 2>&1 | tee "$out/dependencies-desktop.log"
  if [[ "$(dx --version)" != "dioxus 0.8.0-alpha.1 "* ]]; then echo "dx 0.8.0-alpha.1 is required" >&2; exit 2; fi
  (cd apps/desktop && IDENTITY_BUILD_REVISION="$revision" dx build --release --package styrene-identity-desktop --platform desktop --features desktop)
  bundle="$target_dir/dx/styrene-identity-desktop/release/macos/StyreneIdentityDesktop.app"
  # The linker signature does not seal the generated app's Info.plist/resources.
  # Ad-hoc sealing supports local candidate integrity; it is not notarization.
  codesign --force --sign - --identifier org.styrene.identity "$bundle"
  codesign --verify --deep --strict --verbose=2 "$bundle"
  ditto -c -k --keepParent "$bundle" "$out/StyreneIdentity-macos.zip"
  cargo metadata --locked --format-version 1 --filter-platform "$target" --features styrene-identity-desktop/desktop > "$out/dependency-metadata.json"
  cargo deny --locked --exclude-dev --target "$target" --manifest-path apps/desktop/Cargo.toml --features desktop list --format json --layout crate > "$out/licenses-desktop.json"
  scripts/collect-notices.sh "$out/dependency-metadata.json" "$out/licenses-desktop.json" "$out/THIRD-PARTY-NOTICES-DESKTOP.txt"
fi
cp LICENSE docs/dependency-policy.md "$out/"
printf '%s\n' "$metadata" > "$out/cargo-metadata.json"
rustc -vV > "$out/toolchain.txt"
jq -n --arg revision "$revision" --arg target "$target" --arg version "$version" \
  '{source_revision:$revision,target:$target,identity_version:$version,registry_published:false,device_acceptance:false}' > "$out/manifest.json"
(cd "$out" && for file in *; do
  [[ "$file" == SHA256SUMS ]] && continue
  if command -v sha256sum >/dev/null; then sha256sum "$file"; else shasum -a 256 "$file"; fi
done) > "$out/SHA256SUMS"
git diff --exit-code
git diff --cached --exit-code
test -z "$(git ls-files --others --exclude-standard)"
echo "Candidate evidence: $out"
