param(
    [ValidateSet("debug", "release")]
    [string]$Configuration = "release",
    [string]$TargetTriple = ""
)

$ErrorActionPreference = "Stop"
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../../..")).Path

if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    $hostLine = rustc -vV | Where-Object { $_ -like "host: *" }
    if (-not $hostLine) {
        throw "Unable to determine the Rust host target triple."
    }
    $TargetTriple = $hostLine.Substring(6).Trim()
}

if ($TargetTriple -notmatch '^[A-Za-z0-9_.-]+$') {
    throw "Invalid Rust target triple."
}

$cargoArguments = @("build", "-p", "desktop-automation-host", "--target", $TargetTriple)
if ($Configuration -eq "release") {
    $cargoArguments += "--release"
}

Push-Location $repositoryRoot
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "The isolated automation Host build failed."
    }

    $extension = if ($TargetTriple -match 'windows') { ".exe" } else { "" }
    $source = Join-Path $repositoryRoot "target/$TargetTriple/$Configuration/desktop-automation-host$extension"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "The expected Host executable was not produced: $source"
    }

    $binaryDirectory = Join-Path $repositoryRoot "apps/desktop/src-tauri/binaries"
    New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null
    $destination = Join-Path $binaryDirectory "desktop-automation-host-$TargetTriple$extension"
    Copy-Item -LiteralPath $source -Destination $destination -Force
    Write-Output $destination
}
finally {
    Pop-Location
}
