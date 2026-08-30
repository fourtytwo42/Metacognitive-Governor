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
    $cargoArguments = @("build", "-p", "mcg-cli", "-p", "mcg-agent-mcp", "-p", "mcg-plugin-runner", "-p", "mcg-updater")
    $existingDebugRoot = Join-Path $repositoryRoot "target/debug/mcg-protected-rootd.exe"
    if ($Profile -eq "release" -or -not (Test-Path -LiteralPath $existingDebugRoot -PathType Leaf)) {
        $cargoArguments += @("-p", "mcg-protected-rootd")
    }
    if ($Profile -eq "release") {
        $cargoArguments += "--release"
    }
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Sidecar build failed with exit code $LASTEXITCODE."
    }

    # Windows Sandbox starts from a pristine image that does not include the
    # redistributable VCRUNTIME DLL. Keep this security-critical helper
    # self-contained instead of weakening the sandbox or installing host state.
    $runnerArguments = @("rustc", "-p", "mcg-shadow-runner")
    if ($Profile -eq "release") {
        $runnerArguments += "--release"
    }
    $runnerArguments += @("--", "-C", "target-feature=+crt-static")
    & cargo @runnerArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Static Windows Sandbox runner build failed with exit code $LASTEXITCODE."
    }
}

$sourceDirectory = Join-Path $repositoryRoot "target/$Profile"
$sandboxRunner = Join-Path $sourceDirectory "mcg-shadow-runner.exe"
if (-not (Test-Path -LiteralPath $sandboxRunner -PathType Leaf)) {
    throw "Required Windows Sandbox runner is missing: $sandboxRunner"
}
$runnerImage = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($sandboxRunner)
)
if ($runnerImage -match '(?i)(VCRUNTIME|MSVCP)[^\x00]*\.dll') {
    throw "Windows Sandbox runner depends on a Visual C++ redistributable DLL."
}

$sidecarDirectory = Join-Path $repositoryRoot "apps/desktop/src-tauri/binaries"
New-Item -ItemType Directory -Force -Path $sidecarDirectory | Out-Null

$sidecars = @{
    "mcg" = "mcg"
    "mcg-agent-mcp" = "mcg-agent-mcp"
    "mcg-protected-rootd" = "mcg-service"
    "mcg-plugin-runner" = "mcg-plugin-runner"
    "mcg-shadow-runner" = "mcg-shadow-runner"
    "mcg-updater" = "mcg-updater"
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
