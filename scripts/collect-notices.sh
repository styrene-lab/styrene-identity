#!/usr/bin/env bash
set -euo pipefail
metadata="$1"
inventory="$2"
output="$3"
{
  printf 'Third-party dependency source notices\n\n'
  printf 'Exact versions and checksums are recorded in Cargo.lock. Source links below identify the unmodified dependency packages.\n'
  while IFS=$'\t' read -r name version manifest repository license; do
    printf '\nPACKAGE: %s %s\nSOURCE: %s\nSPDX: %s\n' "$name" "$version" "$repository" "$license"
    directory="$(dirname "$manifest")"
    found=0
    while IFS= read -r -d '' notice; do
      found=1
      printf '\nNOTICE FILE: %s\n' "${notice#"$directory/"}"
      cat "$notice"
      printf '\n'
    done < <(find "$directory" -maxdepth 2 -type f \( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' \) -print0)
    if [[ "$found" == 0 ]]; then
      printf 'No standalone notice file was included in the crate. Consult the versioned source archive and its manifest license declaration.\n'
    fi
  done < <(jq -r --slurpfile inventory "$inventory" '
    .packages[] | . as $p | select(.source != null) |
    select(any($inventory[0] | keys[]; startswith($p.name + " " + $p.version + " "))) |
    [.name,.version,.manifest_path,(.repository // ("https://crates.io/crates/"+.name+"/"+.version)),(.license // "SEE SOURCE")] | @tsv
  ' "$metadata")
} > "$output"
