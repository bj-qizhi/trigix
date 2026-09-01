#!/usr/bin/env bash
set -euo pipefail

action="${1:-verify}"
state_directory="${2:-${RUNNER_TEMP:-}/trigix-development-signing}"
identity="Trigix Development Qualification"

if [[ -z "$state_directory" || "$(basename "$state_directory")" != "trigix-development-signing" || "$state_directory" == "/" ]]; then
  echo "state directory must be a dedicated trigix-development-signing directory" >&2
  exit 2
fi

keychain_path="$state_directory/trigix-development.keychain-db"
password_path="$state_directory/keychain-password.txt"
config_path="$state_directory/tauri.macos-development-signing.json"
evidence_path="$state_directory/macos-development-signing.txt"

prepare() {
  mkdir -p "$state_directory"
  chmod 700 "$state_directory"
  keychain_password="$(openssl rand -hex 24)"
  printf '%s' "$keychain_password" > "$password_path"
  chmod 600 "$password_path"

  openssl_config="$state_directory/openssl.cnf"
  printf '%s\n' \
    '[req]' \
    'distinguished_name = distinguished_name' \
    'x509_extensions = extensions' \
    'prompt = no' \
    '[distinguished_name]' \
    "CN = $identity" \
    '[extensions]' \
    'basicConstraints = critical,CA:TRUE' \
    'keyUsage = critical,digitalSignature,keyCertSign' \
    'extendedKeyUsage = codeSigning' \
    'subjectKeyIdentifier = hash' \
    'authorityKeyIdentifier = keyid:always,issuer' > "$openssl_config"

  openssl req -new -newkey rsa:3072 -nodes -x509 -days 1 \
    -config "$openssl_config" \
    -keyout "$state_directory/private-key.pem" \
    -out "$state_directory/certificate.pem"
  openssl pkcs12 -export \
    -inkey "$state_directory/private-key.pem" \
    -in "$state_directory/certificate.pem" \
    -out "$state_directory/identity.p12" \
    -passout "pass:$keychain_password"

  security create-keychain -p "$keychain_password" "$keychain_path"
  security set-keychain-settings -lut 21600 "$keychain_path"
  security unlock-keychain -p "$keychain_password" "$keychain_path"
  security import "$state_directory/identity.p12" \
    -P "$keychain_password" -A -x -t cert -f pkcs12 -k "$keychain_path"
  security list-keychains -d user -s "$keychain_path" login.keychain-db
  security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain_path"
  security find-certificate -c "$identity" "$keychain_path" >/dev/null

  printf '{"bundle":{"macOS":{"signingIdentity":"%s"}}}\n' "$identity" > "$config_path"
  rm -f "$state_directory/private-key.pem" "$state_directory/identity.p12" "$openssl_config"
  printf '%s\n' "$config_path"
}

verify() (
  dmg_path="${1:?usage: qualify-macos-development-signing.sh verify STATE_DIRECTORY DMG_PATH}"
  [[ -f "$dmg_path" ]] || { echo "DMG does not exist: $dmg_path" >&2; exit 1; }

  mount_directory="$(mktemp -d)"
  # Invoked by the EXIT trap on every success or failure path.
  # shellcheck disable=SC2317
  cleanup_mount() {
    hdiutil detach "$mount_directory" -quiet >/dev/null 2>&1 || true
    rmdir "$mount_directory" >/dev/null 2>&1 || true
  }
  trap cleanup_mount EXIT
  hdiutil attach "$dmg_path" -nobrowse -readonly -mountpoint "$mount_directory" -quiet
  app_path="$(find "$mount_directory" -maxdepth 1 -name '*.app' -type d -print -quit)"
  [[ -n "$app_path" ]] || { echo "DMG does not contain an application bundle" >&2; exit 1; }

  codesign --verify --deep --strict --verbose=2 "$app_path"
  codesign --verify --verbose=2 "$dmg_path"
  app_details="$(codesign --display --verbose=4 "$app_path" 2>&1)"
  dmg_details="$(codesign --display --verbose=4 "$dmg_path" 2>&1)"
  grep -Fq "Authority=$identity" <<< "$app_details"
  grep -Fq "Authority=$identity" <<< "$dmg_details"

  if spctl --assess --type execute "$app_path" >/dev/null 2>&1; then
    echo "self-signed development application unexpectedly passed public Gatekeeper assessment" >&2
    exit 1
  fi

  main_binary="$app_path/Contents/MacOS/trigix-desktop"
  sidecar="$app_path/Contents/MacOS/desktop-automation-host"
  for binary in "$main_binary" "$sidecar"; do
    [[ -f "$binary" ]] || { echo "development binary is missing: $binary" >&2; exit 1; }
    architectures="$(lipo -archs "$binary")"
    [[ " $architectures " == *" arm64 "* ]] || { echo "$binary lacks arm64" >&2; exit 1; }
    [[ " $architectures " == *" x86_64 "* ]] || { echo "$binary lacks x86_64" >&2; exit 1; }
  done

  {
    printf 'schema_version=1\n'
    printf 'purpose=macos_development_qualification\n'
    printf 'production_release_eligible=false\n'
    printf 'signer_identity=%s\n' "$identity"
    printf 'isolated_signature_verified=true\n'
    printf 'public_gatekeeper_accepted=false\n'
    printf 'architectures=arm64,x86_64\n'
    printf 'dmg_sha256=%s\n' "$(shasum -a 256 "$dmg_path" | awk '{print $1}')"
  } > "$evidence_path"
  echo "Verified self-signed macOS development DMG."
)

cleanup() {
  security delete-keychain "$keychain_path" >/dev/null 2>&1 || true
  rm -rf "$state_directory"
  echo "Removed ephemeral macOS development-signing state."
}

case "$action" in
  prepare) prepare ;;
  verify) verify "${3:-}" ;;
  cleanup) cleanup ;;
  *) echo "action must be prepare, verify, or cleanup" >&2; exit 2 ;;
esac
