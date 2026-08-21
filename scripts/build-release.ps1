$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$updaterConfigDirectory = $null
$loadedSigningKeyFromPath = $false
$signingKeyPath = $null
Push-Location $repositoryRoot
try {
    node scripts/check-release-source.mjs
    if ($LASTEXITCODE -ne 0) {
        throw "Release source preflight failed with exit code $LASTEXITCODE."
    }
    $securityScanReport = Join-Path $repositoryRoot "target\release\release-security-scan.json"
    if (Test-Path -LiteralPath $securityScanReport) {
        throw "Refusing to replace existing release security-scan evidence: $securityScanReport"
    }
    & "$PSScriptRoot/prepare-sidecars.ps1" -Profile release
    if ($LASTEXITCODE -ne 0) {
        throw "Sidecar preparation failed with exit code $LASTEXITCODE."
    }
    $updaterRequested = -not [string]::IsNullOrWhiteSpace($env:MCG_UPDATER_ENDPOINT) -or
        -not [string]::IsNullOrWhiteSpace($env:MCG_UPDATER_PUBKEY) -or
        -not [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY) -or
        -not [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PATH)
    $tauriConfig = @("--config", "src-tauri/tauri.release.conf.json")
    if ($updaterRequested) {
        foreach ($name in @("MCG_UPDATER_ENDPOINT", "MCG_UPDATER_PUBKEY")) {
            if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
                throw "Signed updater builds require $name."
            }
        }
        $contentKey = -not [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY)
        $pathKey = -not [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PATH)
        if ($contentKey -eq $pathKey) {
            throw "Signed updater builds require exactly one of TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH."
        }
        node scripts/check-updater-config.mjs
        if ($LASTEXITCODE -ne 0) {
            throw "Signed updater configuration validation failed with exit code $LASTEXITCODE."
        }
        if ($pathKey) {
            $signingKeyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
            $env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText($signingKeyPath)
            Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PATH
            $loadedSigningKeyFromPath = $true
        }
        $updaterConfigDirectory = Join-Path ([IO.Path]::GetTempPath()) ("mcg-updater-build-" + [Guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $updaterConfigDirectory -ErrorAction Stop | Out-Null
        $updaterConfig = Join-Path $updaterConfigDirectory "updater.json"
        node scripts/write-updater-build-config.mjs $updaterConfig
        if ($LASTEXITCODE -ne 0) {
            throw "Signed updater bundler configuration failed with exit code $LASTEXITCODE."
        }
        $tauriConfig += @("--config", "src-tauri/tauri.updater.conf.json", "--config", $updaterConfig)
    }
    npm run tauri --workspace @mcg/desktop -- build --ci @tauriConfig --bundles nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Windows installer build failed with exit code $LASTEXITCODE."
    }
    node scripts/write-release-manifest.mjs target/release/bundle/nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Windows release manifest failed with exit code $LASTEXITCODE."
    }
    node scripts/scan-release-artifacts.mjs --root $repositoryRoot --manifest (Join-Path $repositoryRoot "target\release\release-manifest.json") --output $securityScanReport
    if ($LASTEXITCODE -ne 0) {
        throw "Windows release artifact malware scan failed with exit code $LASTEXITCODE."
    }
} finally {
    if ($loadedSigningKeyFromPath) {
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
        $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $signingKeyPath
    }
    if ($null -ne $updaterConfigDirectory -and (Test-Path -LiteralPath $updaterConfigDirectory)) {
        Remove-Item -LiteralPath $updaterConfigDirectory -Recurse -Force
    }
    Pop-Location
}
