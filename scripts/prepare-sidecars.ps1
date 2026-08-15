param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$targetTriple = (rustc -vV | Select-String '^host: ' | ForEach-Object { $_.Line.Substring(6) })
if (-not $targetTriple) {
    throw "Rust did not report a host target triple."
}

if (-not $SkipBuild) {
    $cargoArguments = @("build", "-p", "mcg-cli", "-p", "mcg-shadow-runner")
    $existingDebugDaemon = Join-Path $repositoryRoot "target/debug/mcg-daemon.exe"
    if ($Profile -eq "release" -or -not (Test-Path -LiteralPath $existingDebugDaemon -PathType Leaf)) {
        $cargoArguments += @("-p", "mcg-daemon")
    }
    if ($Profile -eq "release") {
        $cargoArguments += "--release"
    }
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Sidecar build failed with exit code $LASTEXITCODE."
    }
}

$sourceDirectory = Join-Path $repositoryRoot "target/$Profile"
$sidecarDirectory = Join-Path $repositoryRoot "apps/desktop/src-tauri/binaries"
New-Item -ItemType Directory -Force -Path $sidecarDirectory | Out-Null

$sidecars = @{
    "mcg" = "mcg"
    "mcg-daemon" = "mcg-service"
    "mcg-shadow-runner" = "mcg-shadow-runner"
}
foreach ($binary in $sidecars.Keys) {
    $source = Join-Path $sourceDirectory "$binary.exe"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Required sidecar is missing: $source"
    }
    $destination = Join-Path $sidecarDirectory "$($sidecars[$binary])-$targetTriple.exe"
    Copy-Item -LiteralPath $source -Destination $destination -Force
}

Write-Output $targetTriple
