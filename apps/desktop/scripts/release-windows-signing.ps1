param(
    [ValidateSet("Prepare", "Verify", "RemoveIdentity", "Cleanup")]
    [string]$Action = "Verify",
    [string[]]$ArtifactPath = @(),
    [string]$StateDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($StateDirectory)) {
    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw "StateDirectory or RUNNER_TEMP is required."
    }
    $StateDirectory = Join-Path $env:RUNNER_TEMP "trigix-production-signing"
}
$StateDirectory = [System.IO.Path]::GetFullPath($StateDirectory)
if ([System.IO.Path]::GetFileName($StateDirectory) -ne "trigix-production-signing" -or
    [System.IO.Path]::GetPathRoot($StateDirectory) -eq $StateDirectory) {
    throw "StateDirectory must be a dedicated trigix-production-signing directory."
}

$thumbprintPath = Join-Path $StateDirectory "certificate-thumbprint.txt"
$subjectPath = Join-Path $StateDirectory "certificate-subject.txt"
$configPath = Join-Path $StateDirectory "tauri.windows-production-signing.json"
$evidencePath = Join-Path $StateDirectory "windows-production-signing.json"

function Require-EnvironmentValue([string]$name) {
    $value = [System.Environment]::GetEnvironmentVariable($name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "$name is required."
    }
    return $value
}

function Read-StateValue([string]$path, [string]$description) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "$description state is missing."
    }
    $value = (Get-Content -LiteralPath $path -Raw).Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "$description state is empty."
    }
    return $value
}

function Read-ProductionThumbprint {
    $thumbprint = (Read-StateValue $thumbprintPath "Certificate thumbprint").ToUpperInvariant()
    if ($thumbprint -notmatch '^[A-F0-9]{40}$') {
        throw "Certificate thumbprint state is invalid."
    }
    return $thumbprint
}

function Open-PersonalCertificateStore {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new(
        "My",
        [System.Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser
    )
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    return $store
}

function Remove-ProductionCertificate([string]$thumbprint, [string]$expectedSubject) {
    $store = Open-PersonalCertificateStore
    try {
        $matches = $store.Certificates.Find(
            [System.Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint,
            $thumbprint,
            $false
        )
        foreach ($certificate in $matches) {
            if ($certificate.Subject -ne $expectedSubject) {
                throw "Refusing to remove a certificate with an unexpected subject."
            }
            $store.Remove($certificate)
        }
    }
    finally {
        $store.Close()
    }
}

function Assert-ProductionSignature([string]$path, [string]$thumbprint, [string]$expectedSubject) {
    $resolved = Resolve-Path -LiteralPath $path -ErrorAction Stop
    if (-not (Test-Path -LiteralPath $resolved.Path -PathType Leaf)) {
        throw "Signed production artifact is not a file: $path"
    }

    $signature = Get-AuthenticodeSignature -LiteralPath $resolved.Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "Production Authenticode validation failed for ${path}: $($signature.Status)."
    }
    if ($null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -ne $expectedSubject -or
        $signature.SignerCertificate.Thumbprint.ToUpperInvariant() -ne $thumbprint) {
        throw "Production artifact signer does not match the protected identity: $path"
    }
    if ($null -eq $signature.TimeStamperCertificate) {
        throw "Production artifact has no verifiable timestamp: $path"
    }

    $signerSha256 = [System.Convert]::ToHexString(
        [System.Security.Cryptography.SHA256]::HashData($signature.SignerCertificate.RawData)
    )

    return [ordered]@{
        file = [System.IO.Path]::GetFileName($resolved.Path)
        sha256 = (Get-FileHash -LiteralPath $resolved.Path -Algorithm SHA256).Hash.ToUpperInvariant()
        signature_status = $signature.Status.ToString()
        signer_subject = $signature.SignerCertificate.Subject
        signer_sha1_thumbprint = $signature.SignerCertificate.Thumbprint.ToUpperInvariant()
        signer_sha256_fingerprint = $signerSha256
        signer_not_after_utc = $signature.SignerCertificate.NotAfter.ToUniversalTime().ToString("o")
        timestamp_subject = $signature.TimeStamperCertificate.Subject
        timestamp_thumbprint = $signature.TimeStamperCertificate.Thumbprint.ToUpperInvariant()
    }
}

switch ($Action) {
    "Prepare" {
        $certificateBase64 = Require-EnvironmentValue "WINDOWS_CERTIFICATE"
        $certificatePassword = Require-EnvironmentValue "WINDOWS_CERTIFICATE_PASSWORD"
        $expectedSubject = Require-EnvironmentValue "WINDOWS_SIGNING_SUBJECT"
        $timestampUrl = Require-EnvironmentValue "WINDOWS_TIMESTAMP_URL"
        $timestampUri = $null
        if (-not [System.Uri]::TryCreate($timestampUrl, [System.UriKind]::Absolute, [ref]$timestampUri) -or
            $timestampUri.Scheme -notin @("http", "https") -or
            -not [string]::IsNullOrEmpty($timestampUri.UserInfo)) {
            throw "WINDOWS_TIMESTAMP_URL must be an absolute HTTP(S) URL without user information."
        }

        New-Item -ItemType Directory -Force -Path $StateDirectory | Out-Null
        $pfxPath = Join-Path $StateDirectory "identity.pfx"
        try {
            [System.IO.File]::WriteAllBytes($pfxPath, [System.Convert]::FromBase64String($certificateBase64))
            $securePassword = ConvertTo-SecureString $certificatePassword -AsPlainText -Force
            $imported = @(Import-PfxCertificate `
                -FilePath $pfxPath `
                -CertStoreLocation "Cert:\CurrentUser\My" `
                -Password $securePassword `
                -Exportable:$false | Where-Object { $_.HasPrivateKey })
            if ($imported.Count -ne 1) {
                throw "The signing archive must import exactly one certificate with a private key."
            }
            $certificate = $imported[0]
            Set-Content -LiteralPath $thumbprintPath -Value $certificate.Thumbprint.ToUpperInvariant() -Encoding ascii
            Set-Content -LiteralPath $subjectPath -Value $certificate.Subject -Encoding utf8
            if ($certificate.Subject -ne $expectedSubject) {
                throw "The imported signing certificate subject does not match WINDOWS_SIGNING_SUBJECT."
            }
            $codeSigningOid = "1.3.6.1.5.5.7.3.3"
            $hasCodeSigningEku = $false
            foreach ($extension in $certificate.Extensions) {
                if ($extension -is [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]) {
                    $hasCodeSigningEku = $extension.EnhancedKeyUsages | Where-Object { $_.Value -eq $codeSigningOid }
                }
            }
            if (-not $hasCodeSigningEku) {
                throw "The imported certificate is not valid for code signing."
            }

            $config = [ordered]@{
                bundle = [ordered]@{
                    windows = [ordered]@{
                        certificateThumbprint = $certificate.Thumbprint
                        digestAlgorithm = "sha256"
                        timestampUrl = $timestampUrl
                    }
                }
            }
            $config | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $configPath -Encoding utf8
            Write-Output $configPath
        }
        finally {
            if (Test-Path -LiteralPath $pfxPath -PathType Leaf) {
                Remove-Item -LiteralPath $pfxPath -Force
            }
        }
    }

    "Verify" {
        if ($ArtifactPath.Count -eq 0) {
            throw "At least one ArtifactPath is required for verification."
        }
        $thumbprint = Read-ProductionThumbprint
        $expectedSubject = Read-StateValue $subjectPath "Certificate subject"
        $artifacts = @($ArtifactPath | ForEach-Object {
            Assert-ProductionSignature $_ $thumbprint $expectedSubject
        })
        $evidence = [ordered]@{
            schema_version = 1
            purpose = "windows_production_release"
            production_release_eligible = $true
            release_tag = Require-EnvironmentValue "RELEASE_TAG"
            product_version = Require-EnvironmentValue "PRODUCT_VERSION"
            source_revision = Require-EnvironmentValue "SOURCE_REVISION"
            signer_subject = $expectedSubject
            certificate_sha1_thumbprint = $thumbprint
            signature_hash_algorithm = "sha256"
            timestamp_required = $true
            artifacts = $artifacts
            operating_system = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        $evidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $evidencePath -Encoding utf8
        Write-Host "Verified production-signed and timestamped Windows artifacts."
    }

    "RemoveIdentity" {
        if ((Test-Path -LiteralPath $thumbprintPath -PathType Leaf) -and
            (Test-Path -LiteralPath $subjectPath -PathType Leaf)) {
            Remove-ProductionCertificate (Read-ProductionThumbprint) (Read-StateValue $subjectPath "Certificate subject")
            Remove-Item -LiteralPath $thumbprintPath, $subjectPath -Force
        }
        Write-Host "Removed the temporary Windows production-signing identity."
    }

    "Cleanup" {
        if ((Test-Path -LiteralPath $thumbprintPath -PathType Leaf) -and
            (Test-Path -LiteralPath $subjectPath -PathType Leaf)) {
            Remove-ProductionCertificate (Read-ProductionThumbprint) (Read-StateValue $subjectPath "Certificate subject")
        }
        if (Test-Path -LiteralPath $StateDirectory -PathType Container) {
            Remove-Item -LiteralPath $StateDirectory -Recurse -Force
        }
        Write-Host "Removed temporary Windows production-signing state."
    }
}
