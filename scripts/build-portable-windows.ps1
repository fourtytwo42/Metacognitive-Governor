param(
    [switch]$SkipSidecarBuild
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows -and $env:OS -ne "Windows_NT") {
    throw "The Windows portable build must run on Windows."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$portableRoot = Join-Path $repositoryRoot "target\portable\MCG-Portable-Windows-x64"

Push-Location $repositoryRoot
try {
    & "$PSScriptRoot\prepare-sidecars.ps1" -Profile release -SkipBuild:$SkipSidecarBuild
    if ($LASTEXITCODE -ne 0) {
        throw "Portable sidecar preparation failed with exit code $LASTEXITCODE."
    }

    npm run tauri --workspace @mcg/desktop -- build --ci --no-bundle --config src-tauri/tauri.release.conf.json
    if ($LASTEXITCODE -ne 0) {
        throw "Portable desktop production build failed with exit code $LASTEXITCODE."
    }

    New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
    $payload = @{
        "target\release\mcg-desktop.exe" = "MCG-Desktop.exe"
        "target\release\mcg-daemon.exe" = "mcg-service.exe"
        "target\release\mcg.exe" = "MCG.exe"
        "target\release\mcg-plugin-runner.exe" = "mcg-plugin-runner.exe"
        "target\release\mcg-shadow-runner.exe" = "mcg-shadow-runner.exe"
        "target\release\mcg-updater.exe" = "mcg-updater.exe"
        "THIRD_PARTY_NOTICES.md" = "THIRD_PARTY_NOTICES.md"
        "licenses\Apache-2.0.txt" = "Apache-2.0.txt"
        "licenses\Wasmtime-Apache-2.0-WITH-LLVM-exception.txt" = "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt"
    }
    foreach ($entry in $payload.GetEnumerator()) {
        $source = Join-Path $repositoryRoot $entry.Key
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Required portable payload is missing: $source"
        }
        Copy-Item -LiteralPath $source -Destination (Join-Path $portableRoot $entry.Value) -Force
    }

    $desktop = Join-Path $portableRoot "MCG-Desktop.exe"
    $hash = (Get-FileHash -LiteralPath $desktop -Algorithm SHA256).Hash
    $size = (Get-Item -LiteralPath $desktop).Length
    Write-Output "Portable directory: $portableRoot"
    Write-Output "MCG-Desktop.exe bytes: $size"
    Write-Output "MCG-Desktop.exe SHA-256: $hash"
} finally {
    Pop-Location
}
