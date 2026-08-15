param(
    [string]$InstallerPath,
    [switch]$SkipDesktop
)

$ErrorActionPreference = "Stop"

function Stop-UatProcess([Diagnostics.Process]$Process) {
    if ($null -ne $Process) {
        $Process.Refresh()
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
            $Process.WaitForExit()
        }
    }
}

function Wait-ForHealth([int]$Port, [string]$OutputPath) {
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        try {
            $health = Invoke-RestMethod "http://127.0.0.1:$Port/api/v1/system/health" -TimeoutSec 1
            $health | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
            return $health
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    throw "MCG did not become healthy on port $Port."
}

function Save-DesktopScreenshot([string]$Path) {
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    $bounds = [Windows.Forms.SystemInformation]::VirtualScreen
    if ($bounds.Width -lt 760 -or $bounds.Height -lt 540) {
        throw "Desktop screenshot bounds are too small: $($bounds.Width)x$($bounds.Height)."
    }
    $bitmap = [Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size, [Drawing.CopyPixelOperation]::SourceCopy)
        $bitmap.Save($Path, [Drawing.Imaging.ImageFormat]::Png)
        $colors = [Collections.Generic.HashSet[string]]::new()
        for ($x = 0; $x -lt $bitmap.Width; $x += [Math]::Max(1, [int]($bitmap.Width / 20))) {
            for ($y = 0; $y -lt $bitmap.Height; $y += [Math]::Max(1, [int]($bitmap.Height / 20))) {
                [void]$colors.Add($bitmap.GetPixel($x, $y).ToArgb().ToString("X8"))
            }
        }
        if ($colors.Count -lt 10) {
            throw "Packaged desktop screenshot appears blank or uniform."
        }
        [pscustomobject]@{ Width = $bitmap.Width; Height = $bitmap.Height; SampledColors = $colors.Count }
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$existing = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -eq "MCG" }
if ($existing) {
    throw "Installer UAT refuses to replace an existing registered MCG installation."
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $InstallerPath = Join-Path $repositoryRoot "target/release/bundle/nsis/MCG_0.1.0_x64-setup.exe"
}
if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw "Windows installer does not exist: $InstallerPath"
}
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path

$uatRoot = Join-Path ([IO.Path]::GetTempPath()) ("mcg-installer-uat-" + [guid]::NewGuid().ToString("N"))
$installRoot = Join-Path $uatRoot "app"
$isolatedLocalAppData = Join-Path $uatRoot "localappdata"
$serviceDataRoot = Join-Path $isolatedLocalAppData "MCG"
$guiDataRoot = Join-Path $uatRoot "gui-data"
New-Item -ItemType Directory -Path $uatRoot, $isolatedLocalAppData, $serviceDataRoot, $guiDataRoot | Out-Null
$previousLocalAppData = $env:LOCALAPPDATA
$previousDataDir = $env:MCG_DATA_DIR
$previousBindAddress = $env:MCG_BIND_ADDRESS
$daemon = $null
$restartedDaemon = $null
$desktop = $null
$desktopService = $null
$guiHealth = $null
$screenshot = $null
$desktopScreenshot = $null
$installationRemoved = $true

try {
    $env:LOCALAPPDATA = $isolatedLocalAppData
    $install = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($install.ExitCode -ne 0) {
        throw "Installer exited with code $($install.ExitCode)."
    }
    $installationRemoved = $false

    $requiredPayload = @("mcg-desktop.exe", "mcg-service.exe", "mcg.exe", "mcg-plugin-runner.exe", "mcg-shadow-runner.exe", "THIRD_PARTY_NOTICES.md", "Apache-2.0.txt", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt")
    $installed = Get-ChildItem -LiteralPath $installRoot -Recurse -File
    foreach ($name in $requiredPayload) {
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

    $sentinel = Join-Path $serviceDataRoot "installer-uat-retained.txt"
    Set-Content -LiteralPath $sentinel -Value "retain-on-uninstall"
    $repair = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($repair.ExitCode -ne 0) {
        throw "Repair installation exited with code $($repair.ExitCode)."
    }
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) {
        throw "Repair installation removed external MCG data."
    }
    $installed = Get-ChildItem -LiteralPath $installRoot -Recurse -File

    $servicePort = 17777
    $env:MCG_DATA_DIR = $serviceDataRoot
    $env:MCG_BIND_ADDRESS = "127.0.0.1:$servicePort"
    $daemon = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden
    $health = Wait-ForHealth $servicePort (Join-Path $uatRoot "health.json")
    $cli = Join-Path $installRoot "mcg.exe"
    $tokenFile = Join-Path $serviceDataRoot "local-auth-token"
    $cliStatus = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile status 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI status failed: $cliStatus" }
    $doctorOutput = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile doctor --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI doctor failed: $doctorOutput" }
    $doctor = ($doctorOutput -join [Environment]::NewLine) | ConvertFrom-Json
    if ($doctor.schema_version -ne 1 -or $doctor.service -ne "mcg-daemon") { throw "Installed CLI doctor returned an invalid report." }
    $doctor | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $uatRoot "doctor.json") -Encoding UTF8
    $backupOutput = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile backup create --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI backup failed: $backupOutput" }
    $backup = ($backupOutput -join [Environment]::NewLine) | ConvertFrom-Json
    if ($backup.manifest.schema_version -ne 1 -or $backup.manifest.secrets_included -ne $false -or $backup.size_bytes -le 0) { throw "Installed CLI backup returned invalid or unsafe metadata." }
    $backup | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $uatRoot "backup.json") -Encoding UTF8

    $token = (Get-Content -LiteralPath $tokenFile -Raw).Trim()
    $mcpMeta = @{
        "io.modelcontextprotocol/protocolVersion" = "2026-07-28"
        "io.modelcontextprotocol/clientInfo" = @{ name = "mcg-windows-package-uat"; version = "1.0.0" }
        "io.modelcontextprotocol/clientCapabilities" = @{}
    }
    function Invoke-Mcp([int]$Id, [string]$Method) {
        $payload = @{ jsonrpc = "2.0"; id = $Id; method = $Method; params = @{ _meta = $mcpMeta } } | ConvertTo-Json -Depth 10 -Compress
        Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$servicePort/mcp" -Headers @{
            Authorization = "Bearer $token"
            Accept = "application/json, text/event-stream"
            "MCP-Protocol-Version" = "2026-07-28"
            "Mcp-Method" = $Method
        } -ContentType "application/json" -Body $payload
    }
    $discovered = Invoke-Mcp 1 "server/discover"
    if ($discovered.result.supportedVersions[0] -ne "2026-07-28") { throw "Installed MCP discovery version mismatch." }
    $mcpTools = Invoke-Mcp 2 "tools/list"
    $toolNames = @($mcpTools.result.tools | ForEach-Object { $_.name })
    if ($toolNames -notcontains "mcg_run_task" -or $toolNames -notcontains "mcg_system_status") { throw "Installed MCP tool catalog is incomplete." }
    @{ supportedVersion = "2026-07-28"; toolCount = $toolNames.Count } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $uatRoot "mcp.json") -Encoding UTF8
    Stop-UatProcess $daemon
    $daemon = $null

    $restartedDaemon = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden
    $restartHealth = Wait-ForHealth $servicePort (Join-Path $uatRoot "restarted-health.json")
    $restartStatus = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile status 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI failed after daemon restart: $restartStatus" }
    Stop-UatProcess $restartedDaemon
    $restartedDaemon = $null

    if (-not $SkipDesktop) {
        $guiPort = 7777
        if (Get-NetTCPConnection -LocalPort $guiPort -State Listen -ErrorAction SilentlyContinue) {
            throw "Packaged desktop UAT requires unused loopback port 7777."
        }
        $env:MCG_DATA_DIR = $guiDataRoot
        $env:MCG_BIND_ADDRESS = "127.0.0.1:$guiPort"
        $desktop = Start-Process -FilePath (Join-Path $installRoot "mcg-desktop.exe") -PassThru
        $guiHealth = Wait-ForHealth $guiPort (Join-Path $uatRoot "gui-health.json")
        for ($attempt = 0; $attempt -lt 100; $attempt++) {
            $desktop.Refresh()
            if ($desktop.HasExited) { throw "Packaged desktop exited before screenshot capture." }
            if ($desktop.MainWindowHandle -ne 0) { break }
            Start-Sleep -Milliseconds 100
        }
        if ($desktop.MainWindowHandle -eq 0) { throw "Packaged desktop did not create a visible window." }
        $listener = Get-NetTCPConnection -LocalPort $guiPort -State Listen -ErrorAction Stop | Select-Object -First 1
        $desktopService = Get-Process -Id $listener.OwningProcess
        if ($desktopService.ProcessName -ne "mcg-service") { throw "Packaged desktop loopback listener is not mcg-service." }
        Start-Sleep -Seconds 2
        $desktopScreenshot = Join-Path $uatRoot "desktop.png"
        $screenshot = Save-DesktopScreenshot $desktopScreenshot
        Stop-UatProcess $desktop
        $desktop = $null
        Stop-UatProcess $desktopService
        $desktopService = $null
    }

    $uninstaller = $installed | Where-Object Name -Match "^uninstall.*\.exe$" | Select-Object -First 1
    if (-not $uninstaller) { throw "Installed uninstaller is missing." }
    $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -Wait -PassThru -WindowStyle Hidden
    if ($uninstall.ExitCode -ne 0) { throw "Uninstaller exited with code $($uninstall.ExitCode)." }
    $installationRemoved = $true
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) { throw "Uninstall removed external MCG data." }
    if (Test-Path -LiteralPath (Join-Path $installRoot "mcg-desktop.exe")) { throw "Uninstall retained the application executable." }

    $reinstall = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($reinstall.ExitCode -ne 0) { throw "Reinstall exited with code $($reinstall.ExitCode)." }
    $installationRemoved = $false
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) { throw "Reinstall removed external MCG data." }
    foreach ($name in $requiredPayload) {
        if (-not (Get-ChildItem -LiteralPath $installRoot -Recurse -File | Where-Object Name -EQ $name)) { throw "Reinstall is missing $name." }
    }
    $finalUninstaller = Get-ChildItem -LiteralPath $installRoot -Recurse -File | Where-Object Name -Match "^uninstall.*\.exe$" | Select-Object -First 1
    if (-not $finalUninstaller) { throw "Reinstalled uninstaller is missing." }
    $finalUninstall = Start-Process -FilePath $finalUninstaller.FullName -ArgumentList "/S" -Wait -PassThru -WindowStyle Hidden
    if ($finalUninstall.ExitCode -ne 0) { throw "Final uninstaller exited with code $($finalUninstall.ExitCode)." }
    $installationRemoved = $true
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) { throw "Final uninstall removed external MCG data." }
    if (Test-Path -LiteralPath (Join-Path $installRoot "mcg-desktop.exe")) { throw "Final uninstall retained the application executable." }

    [pscustomobject]@{
        UatRoot = $uatRoot
        Installer = $installer
        InstalledFiles = $installed.Count
        HealthService = $health.service
        HealthVersion = $health.version
        RestartHealthVersion = $restartHealth.version
        GuiHealthVersion = if ($null -eq $guiHealth) { "skipped" } else { $guiHealth.version }
        CliStatus = $cliStatus -join " "
        DoctorSchema = $doctor.schema_version
        BackupSchema = $backup.manifest.schema_version
        McpToolCount = $toolNames.Count
        DesktopScreenshot = $desktopScreenshot
        ScreenshotWidth = if ($null -eq $screenshot) { 0 } else { $screenshot.Width }
        ScreenshotHeight = if ($null -eq $screenshot) { 0 } else { $screenshot.Height }
        ScreenshotSampledColors = if ($null -eq $screenshot) { 0 } else { $screenshot.SampledColors }
        DesktopSkipped = [bool]$SkipDesktop
        DataRetained = $true
        RepairPreservedData = $true
        ReinstallPreservedData = $true
        PluginRunnerQualified = $true
        AppExecutableRemoved = $true
    }
} finally {
    Stop-UatProcess $desktop
    Stop-UatProcess $desktopService
    Stop-UatProcess $daemon
    Stop-UatProcess $restartedDaemon
    if (-not $installationRemoved -and (Test-Path -LiteralPath $installRoot -PathType Container)) {
        try {
            $resolvedInstallRoot = [IO.Path]::GetFullPath($installRoot)
            $resolvedUatRoot = [IO.Path]::GetFullPath($uatRoot)
            if (-not $resolvedInstallRoot.StartsWith($resolvedUatRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
                throw "Refusing installer cleanup outside the exact UAT root."
            }
            $cleanupUninstaller = Get-ChildItem -LiteralPath $resolvedInstallRoot -Recurse -File -Filter "uninstall*.exe" | Select-Object -First 1
            if ($cleanupUninstaller) {
                $cleanup = Start-Process -FilePath $cleanupUninstaller.FullName -ArgumentList "/S" -Wait -PassThru -WindowStyle Hidden
                if ($cleanup.ExitCode -ne 0) { Write-Warning "UAT cleanup uninstaller exited with code $($cleanup.ExitCode)." }
            }
        } catch {
            Write-Warning "UAT installation cleanup failed: $_"
        }
    }
    $env:LOCALAPPDATA = $previousLocalAppData
    if ($null -eq $previousDataDir) { Remove-Item Env:MCG_DATA_DIR -ErrorAction SilentlyContinue } else { $env:MCG_DATA_DIR = $previousDataDir }
    if ($null -eq $previousBindAddress) { Remove-Item Env:MCG_BIND_ADDRESS -ErrorAction SilentlyContinue } else { $env:MCG_BIND_ADDRESS = $previousBindAddress }
}
