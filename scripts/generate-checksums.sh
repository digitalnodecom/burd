#!/usr/bin/env bash
#
# Generate SHA-256 checksums for service binaries and print them keyed by
# version, ready to paste into the `checksums` map of a platform entry in
# src-tauri/services.json.
#
# The download/verify path (src-tauri/src/binary.rs) enforces a checksum
# whenever services.json has one for the version being installed, and skips
# verification when absent — so checksums can be filled in incrementally,
# highest-value services (the project-controlled Direct downloads) first.
#
# Usage:
#   scripts/generate-checksums.sh <url> [<url> ...]
#   scripts/generate-checksums.sh --file urls.txt
#
# Each URL should be the exact artifact URL for one version/platform (the same
# URL binary.rs would download). The script prints one "<version>": "<sha256>"
# line per URL; set VERSION_FROM to control how the version is derived.
#
# Example (Direct download, versioned URL):
#   scripts/generate-checksums.sh \
#     "https://burdbin.s3.fr-par.scw.cloud/mariadb/11.4.2/mariadb-arm64.tar.gz"
#
set -euo pipefail

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1
  else shasum -a 256 | cut -d' ' -f1; fi
}

hash_url() {
  local url="$1"
  # Best-effort version extraction: first path segment that looks like a
  # version (e.g. 11.4.2, v2.10.2, 8.0). Override by exporting VERSION=... .
  local version="${VERSION:-}"
  if [ -z "$version" ]; then
    version="$(printf '%s\n' "$url" | grep -oE '[vV]?[0-9]+\.[0-9]+(\.[0-9]+)?' | head -1)"
    version="${version#[vV]}"
  fi
  local sum
  sum="$(curl -fsSL "$url" | sha256)"
  printf '      "%s": "%s"\n' "${version:-UNKNOWN}" "$sum"
}

main() {
  local urls=()
  if [ "${1:-}" = "--file" ]; then
    [ -n "${2:-}" ] || { echo "usage: $0 --file urls.txt" >&2; exit 2; }
    while IFS= read -r line; do [ -n "$line" ] && urls+=("$line"); done < "$2"
  else
    urls=("$@")
  fi
  [ "${#urls[@]}" -gt 0 ] || { echo "usage: $0 <url> [<url> ...] | --file urls.txt" >&2; exit 2; }

  echo '    "checksums": {'
  local i
  for i in "${!urls[@]}"; do
    hash_url "${urls[$i]}"
  done
  echo '    }'
  echo
  echo "# Merge the entries above into the matching platform block in services.json." >&2
}

main "$@"
