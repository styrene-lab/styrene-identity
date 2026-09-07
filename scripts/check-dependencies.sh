#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
profile="${1:-core}"
target="${2:-$(rustc -vV | sed -n 's/^host: //p')}"
args=(--locked --exclude-dev --target "$target")
case "$profile" in
  core) args+=(--manifest-path Cargo.toml) ;;
  repository) args+=(--manifest-path Cargo.toml --no-default-features --features repository-signing) ;;
  pki) args+=(--manifest-path Cargo.toml --no-default-features --features repository-signing,pki,age-format) ;;
  cli) args+=(--manifest-path apps/cli/Cargo.toml) ;;
  desktop) args+=(--manifest-path apps/desktop/Cargo.toml --features desktop) ;;
  ssh) args+=(--manifest-path Cargo.toml --features ssh-agent) ;;
  *) echo "Unknown dependency profile: $profile" >&2; exit 2 ;;
esac
cargo deny "${args[@]}" check advisories licenses sources
