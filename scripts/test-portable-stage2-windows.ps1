param(
    [Parameter(Mandatory = $true)][string]$PortablePath,
    [int]$Port = 7777
)

$ErrorActionPreference = "Stop"

if ($Port -ne 7777) {
    throw "The packaged desktop currently owns the fixed GUI compatibility port 7777."
}
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "Stage 2 portable UAT requires unused loopback port $Port."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$resolvedPortable = [IO.Path]::GetFullPath($PortablePath)
if (-not (Test-Path -LiteralPath $resolvedPortable -PathType Leaf) -or [IO.Path]::GetExtension($resolvedPortable) -ne ".exe") {
    throw "PortablePath must identify the candidate executable: $resolvedPortable"
}
$evidenceParent = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target\stage2-portable-uat"))
$evidenceRoot = Join-Path $evidenceParent ([guid]::NewGuid().ToString("N"))
$dataRoot = Join-Path $evidenceRoot "data"
[void][IO.Directory]::CreateDirectory($dataRoot)

$previousDataDir = $env:MCG_DATA_DIR
$previousBindAddress = $env:MCG_BIND_ADDRESS
$previousWebView2Arguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$desktop = $null
$service = $null

try {
    $env:MCG_DATA_DIR = $dataRoot
    $env:MCG_BIND_ADDRESS = "127.0.0.1:$Port"
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--force-renderer-accessibility"
    $desktop = Start-Process -FilePath $resolvedPortable -PassThru `
        -RedirectStandardOutput (Join-Path $evidenceRoot "desktop.stdout.log") `
        -RedirectStandardError (Join-Path $evidenceRoot "desktop.stderr.log")

    $health = $null
    for ($attempt = 0; $attempt -lt 180; $attempt++) {
        $desktop.Refresh()
        if ($desktop.HasExited) {
            throw "Portable desktop exited before its service became healthy (exit $($desktop.ExitCode))."
        }
        try {
            $health = Invoke-RestMethod "http://127.0.0.1:$Port/api/v1/system/health" -TimeoutSec 1
            break
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($null -eq $health) { throw "Portable desktop service did not become healthy." }

    for ($attempt = 0; $attempt -lt 180; $attempt++) {
        $desktop.Refresh()
        if ($desktop.HasExited) { throw "Portable desktop exited before native UAT." }
        if ($desktop.MainWindowHandle -ne 0) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($desktop.MainWindowHandle -eq 0) { throw "Portable desktop did not expose a native window." }

    $listener = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction Stop | Select-Object -First 1
    $service = Get-Process -Id $listener.OwningProcess -ErrorAction Stop
    if ($service.ProcessName -ne "mcg-service") {
        throw "Portable desktop loopback listener is not owned by mcg-service."
    }
    $expectedRuntimeRoot = [IO.Path]::GetFullPath((Join-Path $dataRoot "runtime"))
    $servicePath = [IO.Path]::GetFullPath($service.Path)
    if (-not $servicePath.StartsWith($expectedRuntimeRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Portable service did not execute from the isolated runtime root."
    }

    $nativeReport = Join-Path $evidenceRoot "desktop-native-accessibility.json"
    $tokenFile = Join-Path $dataRoot "local-auth-token"
    & (Join-Path $PSScriptRoot "test-installed-native-accessibility-windows.ps1") `
        -ProcessId $desktop.Id -OutputPath $nativeReport -Stage2RuntimeOnly `
        -ServiceHost "http://127.0.0.1:$Port" -TokenFile $tokenFile -TimeoutSeconds 30

    $artifact = Get-Item -LiteralPath $resolvedPortable
    $report = [ordered]@{
        schema_version = 1
        source_commit = (git -C $repositoryRoot rev-parse HEAD).Trim()
        artifact_path = $resolvedPortable
        artifact_bytes = $artifact.Length
        artifact_sha256 = (Get-FileHash -LiteralPath $resolvedPortable -Algorithm SHA256).Hash
        evidence_root = $evidenceRoot
        isolated_data_root = $dataRoot
        service_process = $service.ProcessName
        service_executable = $servicePath
        health = $health
        native_accessibility = Get-Content -LiteralPath $nativeReport -Raw | ConvertFrom-Json
    }
    $summaryPath = Join-Path $evidenceRoot "stage2-portable-uat.json"
    [IO.File]::WriteAllText(
        $summaryPath,
        ($report | ConvertTo-Json -Depth 12 -Compress),
        [Text.UTF8Encoding]::new($false)
    )
    Write-Output "Windows Stage 2 portable UAT passed: $summaryPath"
    $report | ConvertTo-Json -Depth 12
} finally {
    if ($null -ne $desktop) {
        $desktop.Refresh()
        if (-not $desktop.HasExited) {
            Stop-Process -Id $desktop.Id -Force -ErrorAction SilentlyContinue
            $desktop.WaitForExit()
        }
    }
    if ($null -ne $service) {
        $service.Refresh()
        if (-not $service.HasExited) {
            Stop-Process -Id $service.Id -Force -ErrorAction SilentlyContinue
            $service.WaitForExit()
        }
    }
    if ($null -eq $previousDataDir) { Remove-Item Env:MCG_DATA_DIR -ErrorAction SilentlyContinue } else { $env:MCG_DATA_DIR = $previousDataDir }
    if ($null -eq $previousBindAddress) { Remove-Item Env:MCG_BIND_ADDRESS -ErrorAction SilentlyContinue } else { $env:MCG_BIND_ADDRESS = $previousBindAddress }
    if ($null -eq $previousWebView2Arguments) { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue } else { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousWebView2Arguments }
}
