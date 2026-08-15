$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repositoryRoot
try {
    node scripts/check-release-source.mjs
    if ($LASTEXITCODE -ne 0) {
        throw "Release source preflight failed with exit code $LASTEXITCODE."
    }
    & "$PSScriptRoot/prepare-sidecars.ps1" -Profile release
    if ($LASTEXITCODE -ne 0) {
        throw "Sidecar preparation failed with exit code $LASTEXITCODE."
    }
    npm run tauri --workspace @mcg/desktop -- build --config src-tauri/tauri.release.conf.json --bundles nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Windows installer build failed with exit code $LASTEXITCODE."
    }
    node scripts/write-release-manifest.mjs target/release/bundle/nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Windows release manifest failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
