param(
    [switch]$SkipSidecarBuild
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows -and $env:OS -ne "Windows_NT") {
    throw "The Windows portable build must run on Windows."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$tauriConfigPath = Join-Path $repositoryRoot "apps\desktop\src-tauri\tauri.conf.json"
$tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json
$version = [string]$tauriConfig.version
if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "Tauri version is missing or invalid: $version"
}
$portableRoot = Join-Path $repositoryRoot "target\portable\MCG-Portable-Windows-x64-v$version"
$desktopFileName = "MCG-Desktop-v$version.exe"

Push-Location $repositoryRoot
try {
    & "$PSScriptRoot\prepare-sidecars.ps1" -Profile release -SkipBuild:$SkipSidecarBuild
    if ($LASTEXITCODE -ne 0) {
        throw "Portable sidecar preparation failed with exit code $LASTEXITCODE."
    }

    npm run tauri --workspace @mcg/desktop -- build --ci --no-bundle --features single-file-portable --config src-tauri/tauri.release.conf.json
    if ($LASTEXITCODE -ne 0) {
        throw "Portable desktop production build failed with exit code $LASTEXITCODE."
    }

    if (Test-Path -LiteralPath $portableRoot) {
        $resolvedPortableRoot = [System.IO.Path]::GetFullPath($portableRoot)
        $resolvedExpectedParent = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\portable"))
        if ([System.IO.Path]::GetDirectoryName($resolvedPortableRoot) -ne $resolvedExpectedParent) {
            throw "Refusing to refresh an unexpected portable directory: $resolvedPortableRoot"
        }
        Remove-Item -LiteralPath $resolvedPortableRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
    $desktopSource = Join-Path $repositoryRoot "target\release\mcg-desktop.exe"
    if (-not (Test-Path -LiteralPath $desktopSource -PathType Leaf)) {
        throw "Required portable desktop is missing: $desktopSource"
    }
    Copy-Item -LiteralPath $desktopSource -Destination (Join-Path $portableRoot $desktopFileName) -Force

    $desktop = Join-Path $portableRoot $desktopFileName
    $hash = (Get-FileHash -LiteralPath $desktop -Algorithm SHA256).Hash
    $size = (Get-Item -LiteralPath $desktop).Length
    $files = @(Get-ChildItem -LiteralPath $portableRoot -File)
    if ($files.Count -ne 1 -or $files[0].Name -ne $desktopFileName) {
        throw "The one-file portable output contains unexpected supporting files."
    }
    Write-Output "Portable directory: $portableRoot"
    Write-Output "Portable payload: one self-contained executable"
    Write-Output "$desktopFileName bytes: $size"
    Write-Output "$desktopFileName SHA-256: $hash"
} finally {
    Pop-Location
}
