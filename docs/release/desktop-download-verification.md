# Desktop download and verification

Public Trigix Desktop releases use tags in the form `desktop-vMAJOR.MINOR.PATCH`. Download the installer and its matching `.sha256` file from the same [GitHub Release](https://github.com/bj-qizhi/trigix/releases). Source archives attached automatically by GitHub are not Desktop installers.

If no stable `desktop-v*` release exists, there is no Official Trigix Desktop GA installer. Do not substitute a CI artifact or a development-signed build and describe it as official.

Community and self-managed distributors may publish their own installers without waiting for Official GA. Verify the named distributor, its signing identity, source revision, modifications, support boundary, security channel, and release evidence. Responsibility for that artifact remains with its distributor. See [Distribution and GA responsibility](distribution-responsibility.md).

## Verify SHA-256

On Windows PowerShell:

```powershell
$installer = "Trigix_Desktop_VERSION_x64-setup.exe"
$expected = (Get-Content "$installer.sha256").Split()[0].ToLowerInvariant()
$actual = (Get-FileHash -Algorithm SHA256 $installer).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "SHA-256 mismatch" }
```

On macOS:

```sh
shasum -a 256 -c Trigix_Desktop_VERSION_universal.dmg.sha256
```

The result must match exactly. A checksum proves byte identity with the release record, not publisher identity by itself.

## Verify Windows publisher trust

```powershell
$signature = Get-AuthenticodeSignature .\Trigix_Desktop_VERSION_x64-setup.exe
$signature | Format-List Status,StatusMessage,SignerCertificate,TimeStamperCertificate
if ($signature.Status -ne "Valid") { throw "Invalid Authenticode signature" }
```

Compare the certificate subject and SHA-256 thumbprint with the release note. Confirm a valid public timestamp. For an Official Trigix release, the release note names the Trigix publisher identity. For an independent distribution, it names that distributor. Stop if Windows reports an unexpected publisher, hash mismatch, untrusted chain, or different subject.

## Verify macOS trust and architecture

```sh
hdiutil verify Trigix_Desktop_VERSION_universal.dmg
spctl --assess --type open --context context:primary-signature --verbose=4 Trigix_Desktop_VERSION_universal.dmg
xcrun stapler validate Trigix_Desktop_VERSION_universal.dmg
```

Mount the DMG, then verify the application:

```sh
codesign --verify --deep --strict --verbose=2 "/Volumes/Trigix Desktop/Trigix Desktop.app"
codesign -dv --verbose=4 "/Volumes/Trigix Desktop/Trigix Desktop.app" 2>&1
lipo -archs "/Volumes/Trigix Desktop/Trigix Desktop.app/Contents/MacOS/Trigix Desktop"
lipo -archs "/Volumes/Trigix Desktop/Trigix Desktop.app/Contents/MacOS/desktop-automation-host"
```

Both binaries must report `x86_64 arm64` or `arm64 x86_64`. Compare the Developer ID identity and Team ID with the release note. Official and independent distributors use their own distinct identities. Stop when Gatekeeper, stapling, signature, or architecture verification fails.

## Mirrors and offline transfer

An approved mirror must copy the installer, checksum, signed manifest, SBOM, provenance, and verification evidence byte for byte. Do not regenerate or edit release metadata. Offline transfer requires controlled media, an entry malware scan, custody records, and the same signature and digest checks.

## Report a mismatch

Do not open or redistribute a suspect installer. Preserve the release URL, asset name, observed digest, UTC time, and verification output without credentials or private mirror URLs. Report it through the private channel described in [SECURITY.md](../../SECURITY.md).
