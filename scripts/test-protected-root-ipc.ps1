param(
    [string]$RootExecutable = "target/debug/mcg-protected-rootd.exe",
    [string]$CliExecutable = "target/debug/mcg.exe",
    [int]$Port = 17841
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$rootExecutablePath = (Resolve-Path (Join-Path $repositoryRoot $RootExecutable)).Path
$cliExecutablePath = (Resolve-Path (Join-Path $repositoryRoot $CliExecutable)).Path
$evidenceRoot = Join-Path $repositoryRoot "target/stage2-root-e2e"
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
$caseRoot = Join-Path $evidenceRoot ([guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $caseRoot | Out-Null
$stdout = Join-Path $caseRoot "root.stdout.log"
$stderr = Join-Path $caseRoot "root.stderr.log"
$priorDataDirectory = $env:MCG_DATA_DIR
$priorBindAddress = $env:MCG_BIND_ADDRESS
$env:MCG_DATA_DIR = $caseRoot
$env:MCG_BIND_ADDRESS = "127.0.0.1:$Port"
$rootProcess = Start-Process -FilePath $rootExecutablePath -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput $stdout -RedirectStandardError $stderr

try {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $sessionPath = Join-Path $caseRoot "root-ipc-session.json"
    while (-not (Test-Path -LiteralPath $sessionPath -PathType Leaf)) {
        if ([DateTime]::UtcNow -gt $deadline) {
            throw "Protected Root IPC session did not become ready."
        }
        Start-Sleep -Milliseconds 100
    }

    $statusJson = & $cliExecutablePath runtime status --json
    if ($LASTEXITCODE -ne 0) {
        throw "mcg runtime status failed: $statusJson"
    }
    $status = $statusJson | ConvertFrom-Json
    if ($status.service -ne "protected-root" -or
        $status.storage_schema_version -ne 51 -or
        $status.protocol_version -ne 1) {
        throw "Protected Root returned unexpected status: $statusJson"
    }

    $health = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/api/v1/system/health" -Method Get
    if ($health.service -ne "mcg-daemon" -or $health.status -ne "ok") {
        throw "The legacy compatibility health contract changed: $($health | ConvertTo-Json -Compress)"
    }

    $legacyStatusJson = & $cliExecutablePath --host "http://127.0.0.1:$Port" status --json
    if ($LASTEXITCODE -ne 0) {
        throw "Legacy mcg status failed against Protected Root: $legacyStatusJson"
    }
    $legacyStatus = $legacyStatusJson | ConvertFrom-Json
    if ($legacyStatus.service -ne "mcg-daemon") {
        throw "Legacy status no longer reports its compatibility identity: $legacyStatusJson"
    }

    $inspect = & $cliExecutablePath runtime inspect
    $inspectText = $inspect -join "`n"
    if ($LASTEXITCODE -ne 0 -or $inspectText -notmatch "writer queue capacity: 64") {
        throw "mcg runtime inspect failed: $inspect"
    }

    $stop = & $cliExecutablePath --host "http://127.0.0.1:$Port" stop --json
    if ($LASTEXITCODE -ne 0) {
        throw "Protected Root stop failed: $stop"
    }
    $rootProcess.WaitForExit(15000) | Out-Null
    if (-not $rootProcess.HasExited) {
        throw "Protected Root did not stop cleanly."
    }

    [ordered]@{
        schema_version = 1
        status = $status
        inspect = @($inspect)
        legacy_health = $health
        legacy_status = $legacyStatus
        stop = $stop | ConvertFrom-Json
        root_exit_code = $rootProcess.ExitCode
        evidence_directory = $caseRoot
    } | ConvertTo-Json -Depth 12
}
finally {
    if (-not $rootProcess.HasExited) {
        Stop-Process -Id $rootProcess.Id -Force -ErrorAction SilentlyContinue
        $rootProcess.WaitForExit()
    }
    $env:MCG_DATA_DIR = $priorDataDirectory
    $env:MCG_BIND_ADDRESS = $priorBindAddress
}
