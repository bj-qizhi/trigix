param(
    [ValidateSet("Sign", "Verify", "Cleanup")]
    [string]$Action = "Verify",
    [string]$FixturePath = "target/debug/desktop-automation-fixture.exe",
    [string]$StateDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$qualificationSubject = "CN=Trigix Automation Qualification Fixture"
if ([string]::IsNullOrWhiteSpace($StateDirectory)) {
    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        throw "StateDirectory or RUNNER_TEMP is required."
    }
    $StateDirectory = Join-Path $env:RUNNER_TEMP "trigix-fixture-signing"
}
$StateDirectory = [System.IO.Path]::GetFullPath($StateDirectory)
if ([System.IO.Path]::GetFileName($StateDirectory) -ne "trigix-fixture-signing" -or
    [System.IO.Path]::GetPathRoot($StateDirectory) -eq $StateDirectory) {
    throw "StateDirectory must be a dedicated trigix-fixture-signing directory."
}
$thumbprintPath = Join-Path $StateDirectory "certificate-thumbprint.txt"
$evidencePath = Join-Path $StateDirectory "qualification.json"

function Resolve-FixturePath {
    $resolved = Resolve-Path -LiteralPath $FixturePath -ErrorAction Stop
    if (-not (Test-Path -LiteralPath $resolved.Path -PathType Leaf)) {
        throw "The qualification fixture is not a file."
    }
    if ([System.IO.Path]::GetExtension($resolved.Path) -ne ".exe") {
        throw "The qualification fixture must be a Windows executable."
    }
    return $resolved.Path
}

function Read-QualificationThumbprint {
    if (-not (Test-Path -LiteralPath $thumbprintPath -PathType Leaf)) {
        throw "Qualification certificate state is missing."
    }
    $thumbprint = (Get-Content -LiteralPath $thumbprintPath -Raw).Trim()
    if ($thumbprint -notmatch '^[A-Fa-f0-9]{40}$') {
        throw "Qualification certificate state is invalid."
    }
    return $thumbprint.ToUpperInvariant()
}

function Open-CertificateStore([string]$name) {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new(
        $name,
        [System.Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser
    )
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    return $store
}

function Remove-QualificationCertificate([string]$thumbprint) {
    $store = Open-CertificateStore "My"
    try {
        $matches = $store.Certificates.Find(
            [System.Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint,
            $thumbprint,
            $false
        )
        foreach ($certificate in $matches) {
            if ($certificate.Subject -ne $qualificationSubject) {
                throw "Refusing to remove a certificate with an unexpected subject."
            }
            $store.Remove($certificate)
        }
    }
    finally {
        $store.Close()
    }
}

function Assert-PrivateKeyIsNonExportable(
    [System.Security.Cryptography.X509Certificates.X509Certificate2]$certificate
) {
    if (-not $certificate.HasPrivateKey) {
        throw "Qualification certificate has no private key."
    }
    $privateKey = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($certificate)
    if ($null -eq $privateKey) {
        throw "Qualification certificate does not use an RSA private key."
    }
    try {
        if ($privateKey -is [System.Security.Cryptography.RSACng]) {
            if ($privateKey.Key.ExportPolicy -ne [System.Security.Cryptography.CngExportPolicies]::None) {
                throw "Qualification private key is exportable."
            }
        }
        elseif ($privateKey -is [System.Security.Cryptography.RSACryptoServiceProvider]) {
            if ($privateKey.CspKeyContainerInfo.Exportable) {
                throw "Qualification private key is exportable."
            }
        }
        else {
            throw "Qualification private-key provider is unsupported."
        }
    }
    finally {
        $privateKey.Dispose()
    }
}

function Assert-ValidFixtureSignature([string]$path) {
    $signature = Get-AuthenticodeSignature -LiteralPath $path
    if ($signature.Status -notin @(
        [System.Management.Automation.SignatureStatus]::Valid,
        [System.Management.Automation.SignatureStatus]::NotTrusted,
        [System.Management.Automation.SignatureStatus]::UnknownError
    )) {
        throw "Fixture Authenticode integrity check failed: $($signature.Status)."
    }
    if ($null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Subject -ne $qualificationSubject) {
        throw "Fixture signer identity is invalid."
    }
    $chain = [System.Security.Cryptography.X509Certificates.X509Chain]::new()
    try {
        $chain.ChainPolicy.TrustMode = [System.Security.Cryptography.X509Certificates.X509ChainTrustMode]::CustomRootTrust
        $chain.ChainPolicy.RevocationMode = [System.Security.Cryptography.X509Certificates.X509RevocationMode]::NoCheck
        $chain.ChainPolicy.VerificationFlags = [System.Security.Cryptography.X509Certificates.X509VerificationFlags]::NoFlag
        $chain.ChainPolicy.CustomTrustStore.Add($signature.SignerCertificate) | Out-Null
        if (-not $chain.Build($signature.SignerCertificate)) {
            $statuses = ($chain.ChainStatus | ForEach-Object { $_.Status.ToString() }) -join ","
            throw "Fixture signer failed the isolated qualification trust chain: $statuses."
        }
    }
    finally {
        $chain.Dispose()
    }
    return $signature
}

switch ($Action) {
    "Sign" {
        New-Item -ItemType Directory -Force -Path $StateDirectory | Out-Null
        if (Test-Path -LiteralPath $thumbprintPath -PathType Leaf) {
            $staleThumbprint = Read-QualificationThumbprint
            Remove-QualificationCertificate $staleThumbprint
            Remove-Item -LiteralPath $thumbprintPath -Force
        }

        $fixture = Resolve-FixturePath
        $certificate = New-SelfSignedCertificate `
            -Type CodeSigningCert `
            -Subject $qualificationSubject `
            -FriendlyName "Trigix ephemeral fixture qualification" `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -KeyAlgorithm RSA `
            -KeyLength 2048 `
            -HashAlgorithm SHA256 `
            -KeyExportPolicy NonExportable `
            -NotBefore (Get-Date).AddMinutes(-5) `
            -NotAfter (Get-Date).AddHours(8)
        Set-Content -LiteralPath $thumbprintPath -Value $certificate.Thumbprint -Encoding ascii
        Assert-PrivateKeyIsNonExportable $certificate

        $signature = Set-AuthenticodeSignature `
            -LiteralPath $fixture `
            -Certificate $certificate `
            -HashAlgorithm SHA256
        if ($null -eq $signature.SignerCertificate -or
            $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint) {
            throw "Fixture signing did not produce the expected signer."
        }
        $verified = Assert-ValidFixtureSignature $fixture

        $tamperedFixture = Join-Path $StateDirectory "tampered-fixture.exe"
        Copy-Item -LiteralPath $fixture -Destination $tamperedFixture
        $tamperedStream = [System.IO.File]::OpenWrite($tamperedFixture)
        try {
            $tamperedStream.Seek(0, [System.IO.SeekOrigin]::End) | Out-Null
            $tamperedStream.WriteByte(0)
        }
        finally {
            $tamperedStream.Dispose()
        }
        $tamperRejected = $false
        try {
            Assert-ValidFixtureSignature $tamperedFixture | Out-Null
        }
        catch {
            $tamperRejected = $true
        }
        finally {
            Remove-Item -LiteralPath $tamperedFixture -Force
        }
        if (-not $tamperRejected) {
            throw "Fixture signature verification accepted a tampered executable."
        }

        $digest = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToUpperInvariant()
        $evidence = [ordered]@{
            schema_version = 1
            purpose = "windows_fixture_qualification"
            signer_subject = $verified.SignerCertificate.Subject
            certificate_thumbprint = $verified.SignerCertificate.Thumbprint.ToUpperInvariant()
            signature_status = $verified.Status.ToString()
            isolated_chain_trusted = $true
            tamper_check_rejected = $true
            signature_hash_algorithm = "sha256"
            fixture_sha256 = $digest
            operating_system = [System.Environment]::OSVersion.VersionString
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        $evidence | ConvertTo-Json | Set-Content -LiteralPath $evidencePath -Encoding utf8
        Write-Host "Signed and verified the deterministic Windows qualification fixture."
    }

    "Verify" {
        $fixture = Resolve-FixturePath
        $thumbprint = Read-QualificationThumbprint
        if (-not (Test-Path -LiteralPath $evidencePath -PathType Leaf)) {
            throw "Qualification evidence is missing."
        }
        $signature = Assert-ValidFixtureSignature $fixture
        if ($signature.SignerCertificate.Thumbprint.ToUpperInvariant() -ne $thumbprint) {
            throw "Fixture signature does not match qualification certificate state."
        }
        $evidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
        if ($evidence.schema_version -ne 1 -or
            $evidence.purpose -ne "windows_fixture_qualification" -or
            $evidence.signer_subject -ne $qualificationSubject -or
            $evidence.certificate_thumbprint -ne $thumbprint -or
            $evidence.signature_status -ne $signature.Status.ToString() -or
            $evidence.isolated_chain_trusted -ne $true -or
            $evidence.tamper_check_rejected -ne $true -or
            $evidence.signature_hash_algorithm -ne "sha256") {
            throw "Qualification evidence is invalid."
        }
        $digest = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToUpperInvariant()
        if ($evidence.fixture_sha256 -ne $digest) {
            throw "Signed fixture digest does not match qualification evidence."
        }
        Write-Host "Verified the signed Windows fixture and qualification evidence."
    }

    "Cleanup" {
        if (Test-Path -LiteralPath $thumbprintPath -PathType Leaf) {
            $thumbprint = Read-QualificationThumbprint
            Remove-QualificationCertificate $thumbprint
        }
        if (Test-Path -LiteralPath $StateDirectory -PathType Container) {
            Remove-Item -LiteralPath $StateDirectory -Recurse -Force
        }
        Write-Host "Removed ephemeral fixture-signing state."
    }
}
