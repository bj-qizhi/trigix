param(
    [ValidateSet("Prepare", "Verify", "Cleanup")]
    [string]$Action = "Verify",
    [string[]]$ArtifactPath = @(),
    [string]$StateDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$developmentSubject = "CN=Trigix Development Qualification"
if ([string]::IsNullOrWhiteSpace($StateDirectory)) {
    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw "StateDirectory or RUNNER_TEMP is required."
    }
    $StateDirectory = Join-Path $env:RUNNER_TEMP "trigix-development-signing"
}
$StateDirectory = [System.IO.Path]::GetFullPath($StateDirectory)
if ([System.IO.Path]::GetFileName($StateDirectory) -ne "trigix-development-signing" -or
    [System.IO.Path]::GetPathRoot($StateDirectory) -eq $StateDirectory) {
    throw "StateDirectory must be a dedicated trigix-development-signing directory."
}

$thumbprintPath = Join-Path $StateDirectory "certificate-thumbprint.txt"
$configPath = Join-Path $StateDirectory "tauri.windows-development-signing.json"
$evidencePath = Join-Path $StateDirectory "windows-development-signing.json"

function Open-PersonalCertificateStore {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new(
        "My",
        [System.Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser
    )
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    return $store
}

function Read-DevelopmentThumbprint {
    if (-not (Test-Path -LiteralPath $thumbprintPath -PathType Leaf)) {
        throw "Development certificate state is missing."
    }
    $thumbprint = (Get-Content -LiteralPath $thumbprintPath -Raw).Trim().ToUpperInvariant()
    if ($thumbprint -notmatch '^[A-F0-9]{40}$') {
        throw "Development certificate state is invalid."
    }
    return $thumbprint
}

function Remove-DevelopmentCertificate([string]$thumbprint) {
    $store = Open-PersonalCertificateStore
    try {
        $matches = $store.Certificates.Find(
            [System.Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint,
            $thumbprint,
            $false
        )
        foreach ($certificate in $matches) {
            if ($certificate.Subject -ne $developmentSubject) {
                throw "Refusing to remove a certificate with an unexpected subject."
            }
            $store.Remove($certificate)
        }
    }
    finally {
        $store.Close()
    }
}

function Assert-NonExportablePrivateKey(
    [System.Security.Cryptography.X509Certificates.X509Certificate2]$certificate
) {
    if (-not $certificate.HasPrivateKey) {
        throw "Development certificate has no private key."
    }
    $privateKey = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($certificate)
    if ($null -eq $privateKey) {
        throw "Development certificate does not use an RSA private key."
    }
    try {
        if ($privateKey -is [System.Security.Cryptography.RSACng]) {
            if ($privateKey.Key.ExportPolicy -ne [System.Security.Cryptography.CngExportPolicies]::None) {
                throw "Development private key is exportable."
            }
        }
        elseif ($privateKey -is [System.Security.Cryptography.RSACryptoServiceProvider]) {
            if ($privateKey.CspKeyContainerInfo.Exportable) {
                throw "Development private key is exportable."
            }
        }
        else {
            throw "Development private-key provider is unsupported."
        }
    }
    finally {
        $privateKey.Dispose()
    }
}

function Assert-DevelopmentSignature([string]$path, [string]$thumbprint) {
    $resolved = Resolve-Path -LiteralPath $path -ErrorAction Stop
    if (-not (Test-Path -LiteralPath $resolved.Path -PathType Leaf)) {
        throw "Signed development artifact is not a file: $path"
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $resolved.Path
    if ($signature.Status -notin @(
        [System.Management.Automation.SignatureStatus]::Valid,
        [System.Management.Automation.SignatureStatus]::NotTrusted,
        [System.Management.Automation.SignatureStatus]::UnknownError
    )) {
        throw "Development Authenticode integrity check failed for ${path}: $($signature.Status)."
    }
    if ($null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -ne $developmentSubject -or
        $signature.SignerCertificate.Thumbprint.ToUpperInvariant() -ne $thumbprint) {
        throw "Development artifact signer is invalid: $path"
    }

    $chain = [System.Security.Cryptography.X509Certificates.X509Chain]::new()
    try {
        $chain.ChainPolicy.TrustMode = [System.Security.Cryptography.X509Certificates.X509ChainTrustMode]::CustomRootTrust
        $chain.ChainPolicy.RevocationMode = [System.Security.Cryptography.X509Certificates.X509RevocationMode]::NoCheck
        $chain.ChainPolicy.CustomTrustStore.Add($signature.SignerCertificate) | Out-Null
        if (-not $chain.Build($signature.SignerCertificate)) {
            $statuses = ($chain.ChainStatus | ForEach-Object { $_.Status.ToString() }) -join ","
            throw "Development signer failed the isolated trust chain: $statuses."
        }
    }
    finally {
        $chain.Dispose()
    }

    return [ordered]@{
        file = [System.IO.Path]::GetFileName($resolved.Path)
        sha256 = (Get-FileHash -LiteralPath $resolved.Path -Algorithm SHA256).Hash.ToUpperInvariant()
        signature_status = $signature.Status.ToString()
    }
}

switch ($Action) {
    "Prepare" {
        New-Item -ItemType Directory -Force -Path $StateDirectory | Out-Null
        if (Test-Path -LiteralPath $thumbprintPath -PathType Leaf) {
            Remove-DevelopmentCertificate (Read-DevelopmentThumbprint)
        }

        $certificate = New-SelfSignedCertificate `
            -Type CodeSigningCert `
            -Subject $developmentSubject `
            -FriendlyName "Trigix ephemeral development package signing" `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -KeyAlgorithm RSA `
            -KeyLength 3072 `
            -HashAlgorithm SHA256 `
            -KeyExportPolicy NonExportable `
            -NotBefore (Get-Date).AddMinutes(-5) `
            -NotAfter (Get-Date).AddHours(8)
        Assert-NonExportablePrivateKey $certificate
        Set-Content -LiteralPath $thumbprintPath -Value $certificate.Thumbprint -Encoding ascii

        $config = [ordered]@{
            bundle = [ordered]@{
                windows = [ordered]@{
                    certificateThumbprint = $certificate.Thumbprint
                    digestAlgorithm = "sha256"
                }
            }
        }
        $config | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $configPath -Encoding utf8
        Write-Output $configPath
    }

    "Verify" {
        if ($ArtifactPath.Count -eq 0) {
            throw "At least one ArtifactPath is required for verification."
        }
        $thumbprint = Read-DevelopmentThumbprint
        $artifacts = @($ArtifactPath | ForEach-Object {
            Assert-DevelopmentSignature $_ $thumbprint
        })
        $evidence = [ordered]@{
            schema_version = 1
            purpose = "windows_development_qualification"
            production_release_eligible = $false
            signer_subject = $developmentSubject
            certificate_thumbprint = $thumbprint
            isolated_chain_trusted = $true
            signature_hash_algorithm = "sha256"
            artifacts = $artifacts
            operating_system = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        $evidence | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $evidencePath -Encoding utf8
        Write-Host "Verified self-signed Windows development artifacts."
    }

    "Cleanup" {
        if (Test-Path -LiteralPath $thumbprintPath -PathType Leaf) {
            Remove-DevelopmentCertificate (Read-DevelopmentThumbprint)
        }
        if (Test-Path -LiteralPath $StateDirectory -PathType Container) {
            Remove-Item -LiteralPath $StateDirectory -Recurse -Force
        }
        Write-Host "Removed ephemeral Windows development-signing state."
    }
}
