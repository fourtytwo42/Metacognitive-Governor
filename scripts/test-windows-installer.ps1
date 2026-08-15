$ErrorActionPreference = "Stop"

$existing = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -eq "MCG" }
if ($existing) {
    throw "Installer UAT refuses to replace an existing registered MCG installation."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $repositoryRoot "target/release/bundle/nsis/MCG_0.1.0_x64-setup.exe"
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "Build the Windows installer before running installer UAT."
}

$uatRoot = Join-Path ([IO.Path]::GetTempPath()) ("mcg-installer-uat-" + [guid]::NewGuid().ToString("N"))
$installRoot = Join-Path $uatRoot "app"
$isolatedLocalAppData = Join-Path $uatRoot "localappdata"
New-Item -ItemType Directory -Path $uatRoot, $isolatedLocalAppData | Out-Null
$previousLocalAppData = $env:LOCALAPPDATA

try {
    $env:LOCALAPPDATA = $isolatedLocalAppData
    $install = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($install.ExitCode -ne 0) {
        throw "Installer exited with code $($install.ExitCode)."
    }

    $installed = Get-ChildItem -LiteralPath $installRoot -Recurse -File
    foreach ($name in @("mcg-desktop.exe", "mcg-service.exe", "mcg.exe", "mcg-plugin-runner.exe", "mcg-shadow-runner.exe", "THIRD_PARTY_NOTICES.md", "Apache-2.0.txt", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt")) {
        if ($installed.Name -notcontains $name) {
            throw "Installed payload is missing $name."
        }
    }

    $runner = Join-Path $installRoot "mcg-plugin-runner.exe"
    $module = @'
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (export "memory" (memory 0))
  (data (i32.const 8) "{\22package\22:true}")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 16))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 24)))))
'@
    $moduleBytes = [Text.Encoding]::UTF8.GetBytes($module)
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        $moduleDigest = -join ($sha256.ComputeHash($moduleBytes) | ForEach-Object { $_.ToString("x2") })
    } finally {
        $sha256.Dispose()
    }
    $runnerRequest = [ordered]@{
        schema_version = 1
        manifest = [ordered]@{
            schema_version = 1
            id = "uat.package"
            display_name = "Package UAT"
            version = "1.0.0"
            class = "sandboxed_wasi"
            requested_capabilities = @()
        }
        module_sha256 = $moduleDigest
        module_base64 = [Convert]::ToBase64String($moduleBytes)
        input = @{}
    }
    function Invoke-PackageRunner([object]$Request, [string]$Label) {
        $stdin = Join-Path $uatRoot "$Label-request.json"
        $stdout = Join-Path $uatRoot "$Label-response.json"
        $stderr = Join-Path $uatRoot "$Label-stderr.txt"
        [IO.File]::WriteAllText($stdin, ($Request | ConvertTo-Json -Depth 8 -Compress), [Text.UTF8Encoding]::new($false))
        $process = Start-Process -FilePath $runner -RedirectStandardInput $stdin -RedirectStandardOutput $stdout -RedirectStandardError $stderr -Wait -PassThru -WindowStyle Hidden
        [pscustomobject]@{ ExitCode = $process.ExitCode; Response = Get-Content -LiteralPath $stdout -Raw | ConvertFrom-Json; Stderr = Get-Content -LiteralPath $stderr -Raw }
    }
    $validRunner = Invoke-PackageRunner $runnerRequest "valid-plugin"
    if ($validRunner.ExitCode -ne 0 -or $validRunner.Stderr -or $validRunner.Response.state -ne "succeeded" -or -not $validRunner.Response.result.output.package) {
        throw "Installed plugin runner did not execute the digest-bound WASI package."
    }
    $runnerRequest.module_sha256 = "0" * 64
    $tamperedRunner = Invoke-PackageRunner $runnerRequest "tampered-plugin"
    if ($tamperedRunner.ExitCode -eq 0 -or $tamperedRunner.Stderr -or $tamperedRunner.Response.error_code -ne "module_digest_mismatch") {
        throw "Installed plugin runner did not reject a tampered module digest."
    }

    $dataRoot = Join-Path $isolatedLocalAppData "MCG"
    New-Item -ItemType Directory -Path $dataRoot | Out-Null
    $sentinel = Join-Path $dataRoot "installer-uat-retained.txt"
    Set-Content -LiteralPath $sentinel -Value "retain-on-uninstall"

    $repair = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($repair.ExitCode -ne 0) {
        throw "Repair installation exited with code $($repair.ExitCode)."
    }
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) {
        throw "Repair installation removed external MCG data."
    }
    $installed = Get-ChildItem -LiteralPath $installRoot -Recurse -File

    $env:MCG_DATA_DIR = $dataRoot
    $env:MCG_BIND_ADDRESS = "127.0.0.1:17777"
    $daemon = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden
    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 50; $attempt++) {
            try {
                $health = Invoke-RestMethod "http://127.0.0.1:17777/api/v1/system/health" -TimeoutSec 1
                $ready = $true
                break
            } catch {
                Start-Sleep -Milliseconds 100
            }
        }
        if (-not $ready) {
            throw "Installed daemon did not become ready."
        }
        $cliStatus = & (Join-Path $installRoot "mcg.exe") --host "http://127.0.0.1:17777" status 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Installed CLI failed: $cliStatus"
        }
    } finally {
        if (-not $daemon.HasExited) {
            Stop-Process -Id $daemon.Id
        }
        $daemon.WaitForExit()
    }

    $uninstaller = $installed | Where-Object Name -Match "^uninstall.*\.exe$" | Select-Object -First 1
    if (-not $uninstaller) {
        throw "Installed uninstaller is missing."
    }
    $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -Wait -PassThru -WindowStyle Hidden
    if ($uninstall.ExitCode -ne 0) {
        throw "Uninstaller exited with code $($uninstall.ExitCode)."
    }
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) {
        throw "Uninstall removed external MCG data."
    }

    [pscustomobject]@{
        UatRoot = $uatRoot
        InstalledFiles = $installed.Count
        HealthService = $health.service
        HealthVersion = $health.version
        CliStatus = $cliStatus -join " "
        DataRetained = $true
        RepairPreservedData = $true
        PluginRunnerQualified = $true
        AppExecutableRemoved = -not (Test-Path -LiteralPath (Join-Path $installRoot "mcg-desktop.exe"))
    }
} finally {
    $env:LOCALAPPDATA = $previousLocalAppData
    Remove-Item Env:MCG_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:MCG_BIND_ADDRESS -ErrorAction SilentlyContinue
}
