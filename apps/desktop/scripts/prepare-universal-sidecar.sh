#!/usr/bin/env bash
set -euo pipefail

configuration="${1:-release}"
script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "$script_directory/../../.." && pwd)"

"$script_directory/prepare-sidecar.sh" "$configuration" aarch64-apple-darwin
"$script_directory/prepare-sidecar.sh" "$configuration" x86_64-apple-darwin

binary_directory="$repository_root/apps/desktop/src-tauri/binaries"
destination="$binary_directory/desktop-automation-host-universal-apple-darwin"
lipo -create \
  "$binary_directory/desktop-automation-host-aarch64-apple-darwin" \
  "$binary_directory/desktop-automation-host-x86_64-apple-darwin" \
  -output "$destination"
chmod 0755 "$destination"

architectures="$(lipo -archs "$destination")"
[[ " $architectures " == *" arm64 "* ]] || { echo "universal Host lacks arm64" >&2; exit 1; }
[[ " $architectures " == *" x86_64 "* ]] || { echo "universal Host lacks x86_64" >&2; exit 1; }
printf '%s\n' "$destination"
