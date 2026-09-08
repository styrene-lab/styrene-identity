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
  ssh)
    args+=(--manifest-path Cargo.toml --features ssh-agent)
    if [[ "$(date -u +%F)" > "2026-12-07" ]]; then
      echo "The bounded RSA non-applicability decision requires renewal." >&2; exit 1
    fi
    cargo metadata --locked --format-version 1 --features ssh-agent | jq -e '
      [.packages[] | select(.name == "rsa" or .name == "ssh-key" or .name == "ssh-agent-lib") | [.name,.version]] | sort ==
      [["rsa","0.9.10"],["ssh-agent-lib","0.5.2"],["ssh-key","0.6.7"]]
    ' >/dev/null
    cargo test --locked --features ssh-agent rsa_requests_are_rejected_before_custody_access --quiet
    # Scope this exception to the guarded SSH invocation. Direct cargo-deny and
    # other release profiles retain the unmodified vulnerability policy.
    config="$(mktemp "${TMPDIR:-/tmp}/identity-deny.XXXXXX")"
    trap 'rm -f "$config"' EXIT
    awk '/^ignore = \[/ { print; print "  { id = \"RUSTSEC-2023-0071\", reason = \"Guarded SSH-only non-applicability decision; see docs/dependency-policy.md.\" },"; next } { print }' deny.toml > "$config"
    args+=(--config "$config")
    ;;
  *) echo "Unknown dependency profile: $profile" >&2; exit 2 ;;
esac
cargo deny "${args[@]}" check advisories licenses sources bans
