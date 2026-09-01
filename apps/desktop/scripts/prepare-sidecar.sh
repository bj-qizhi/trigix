#!/usr/bin/env bash
set -euo pipefail

configuration="${1:-release}"
target_triple="${2:-}"

case "$configuration" in
  debug|release) ;;
  *) echo "configuration must be debug or release" >&2; exit 2 ;;
esac

if [[ -z "$target_triple" ]]; then
  target_triple="$(rustc -vV | awk '/^host: / { print $2 }')"
fi
if [[ ! "$target_triple" =~ ^[A-Za-z0-9_.-]+$ ]]; then
  echo "invalid Rust target triple" >&2
  exit 2
fi

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "$script_directory/../../.." && pwd)"
cargo_arguments=(build -p desktop-automation-host --target "$target_triple")
if [[ "$configuration" == release ]]; then
  cargo_arguments+=(--release)
fi

(cd "$repository_root" && cargo "${cargo_arguments[@]}")
source_path="$repository_root/target/$target_triple/$configuration/desktop-automation-host"
if [[ ! -f "$source_path" ]]; then
  echo "expected Host executable was not produced: $source_path" >&2
  exit 1
fi

binary_directory="$repository_root/apps/desktop/src-tauri/binaries"
mkdir -p "$binary_directory"
destination="$binary_directory/desktop-automation-host-$target_triple"
install -m 0755 "$source_path" "$destination"
printf '%s\n' "$destination"
