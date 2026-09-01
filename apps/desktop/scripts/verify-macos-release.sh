#!/usr/bin/env bash
set -euo pipefail

dmg_path="${1:?usage: verify-macos-release.sh path/to/package.dmg}"
if [[ ! -f "$dmg_path" ]]; then
  echo "DMG does not exist: $dmg_path" >&2
  exit 2
fi

mount_directory="$(mktemp -d)"
cleanup() {
  hdiutil detach "$mount_directory" -quiet >/dev/null 2>&1 || true
  rmdir "$mount_directory" >/dev/null 2>&1 || true
}
trap cleanup EXIT

hdiutil attach "$dmg_path" -nobrowse -readonly -mountpoint "$mount_directory" -quiet
app_path="$(find "$mount_directory" -maxdepth 1 -name '*.app' -type d -print -quit)"
if [[ -z "$app_path" ]]; then
  echo "DMG does not contain an application bundle" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$app_path"
spctl --assess --type execute --verbose=2 "$app_path"
xcrun stapler validate "$app_path"
xcrun stapler validate "$dmg_path"

main_binary="$app_path/Contents/MacOS/trigix-desktop"
sidecar="$app_path/Contents/MacOS/desktop-automation-host"
for binary in "$main_binary" "$sidecar"; do
  if [[ ! -f "$binary" ]]; then
    echo "release binary is missing: $binary" >&2
    exit 1
  fi
  architectures="$(lipo -archs "$binary")"
  [[ " $architectures " == *" arm64 "* ]] || { echo "$binary lacks arm64" >&2; exit 1; }
  [[ " $architectures " == *" x86_64 "* ]] || { echo "$binary lacks x86_64" >&2; exit 1; }
done

printf 'verified=%s\n' "$dmg_path"
printf 'sha256=%s\n' "$(shasum -a 256 "$dmg_path" | awk '{print $1}')"
