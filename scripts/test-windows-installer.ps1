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
        AppExecutableRemoved = -not (Test-Path -LiteralPath (Join-Path $installRoot "mcg-desktop.exe"))
    }
} finally {
    $env:LOCALAPPDATA = $previousLocalAppData
    Remove-Item Env:MCG_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:MCG_BIND_ADDRESS -ErrorAction SilentlyContinue
}
