param(
    [string]$InstallerPath,
    [switch]$SkipDesktop,
    [string]$PythonPath,
    [string]$NodeBinPath
)

$ErrorActionPreference = "Stop"

if ($NodeBinPath) {
    $resolvedNodeBin = [IO.Path]::GetFullPath($NodeBinPath)
    if (-not (Test-Path -LiteralPath $resolvedNodeBin -PathType Container) `
        -or -not (Test-Path -LiteralPath (Join-Path $resolvedNodeBin "node.exe") -PathType Leaf) `
        -or -not (Test-Path -LiteralPath (Join-Path $resolvedNodeBin "npm.cmd") -PathType Leaf)) {
        throw "NodeBinPath must contain node.exe and npm.cmd: $resolvedNodeBin"
    }
    $env:PATH = $resolvedNodeBin + [IO.Path]::PathSeparator + $env:PATH
} elseif (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue)) {
    throw "Windows package UAT requires npm on PATH or an explicit -NodeBinPath."
}

if ([string]::IsNullOrWhiteSpace($env:MCG_CODEX_APP_SERVER_EXECUTABLE)) {
    throw "Windows package UAT requires MCG_CODEX_APP_SERVER_EXECUTABLE to pin the native Codex 0.150.1 executable."
}
$resolvedCodexExecutable = [IO.Path]::GetFullPath($env:MCG_CODEX_APP_SERVER_EXECUTABLE)
if (-not (Test-Path -LiteralPath $resolvedCodexExecutable -PathType Leaf)) {
    throw "MCG_CODEX_APP_SERVER_EXECUTABLE does not exist: $resolvedCodexExecutable"
}
$codexVersion = & $resolvedCodexExecutable --version 2>&1
if ($LASTEXITCODE -ne 0 -or ($codexVersion -join "").Trim() -ne "codex-cli 0.150.1") {
    throw "Windows package UAT requires the native codex-cli 0.150.1 executable."
}
$env:MCG_CODEX_APP_SERVER_EXECUTABLE = $resolvedCodexExecutable

function Stop-UatProcess([Diagnostics.Process]$Process) {
    if ($null -ne $Process) {
        $Process.Refresh()
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
            $Process.WaitForExit()
        }
    }
}

function Test-ContainsByteSequence([byte[]]$Bytes, [byte[]]$Sequence) {
    if ($Sequence.Length -eq 0 -or $Bytes.Length -lt $Sequence.Length) { return $false }
    for ($offset = 0; $offset -le $Bytes.Length - $Sequence.Length; $offset++) {
        $matches = $true
        for ($index = 0; $index -lt $Sequence.Length; $index++) {
            if ($Bytes[$offset + $index] -ne $Sequence[$index]) {
                $matches = $false
                break
            }
        }
        if ($matches) { return $true }
    }
    return $false
}

function Stop-UatDaemonGracefully(
    [Diagnostics.Process]$Process,
    [string]$Executable,
    [string]$HostUrl,
    [string]$TokenFile,
    [string]$DataRoot,
    [string]$EvidencePath
) {
    $output = & $Executable --host $HostUrl --token-file $TokenFile stop --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI graceful stop failed: $output" }
    $response = ($output -join [Environment]::NewLine) | ConvertFrom-Json
    if ($response.accepted -ne $true -or $response.service -ne "mcg-daemon") {
        throw "Installed CLI graceful stop returned an invalid response."
    }
    $response | ConvertTo-Json | Set-Content -LiteralPath $EvidencePath -Encoding UTF8
    if (-not $Process.WaitForExit(10000)) {
        throw "Installed daemon did not exit after accepting graceful shutdown."
    }
    foreach ($sidecar in @("mcg.sqlite3-wal", "mcg.sqlite3-shm")) {
        if (Test-Path -LiteralPath (Join-Path $DataRoot $sidecar)) {
            throw "Installed daemon retained $sidecar after graceful shutdown."
        }
    }
    return $response
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

function Invoke-ConsumerCliProcess(
    [string]$Executable,
    [string[]]$Arguments,
    [string]$InputText,
    [string]$EvidenceRoot,
    [string]$Label
) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.Arguments = (@($Arguments | ForEach-Object { '"' + $_.Replace('"', '\"') + '"' }) -join ' ')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "Could not start installed consumer CLI for $Label." }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if ($null -ne $InputText) { $process.StandardInput.Write($InputText) }
    $process.StandardInput.Close()
    $process.WaitForExit()
    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $EvidenceRoot "$Label.stdout"), $stdout, $encoding)
    [IO.File]::WriteAllText((Join-Path $EvidenceRoot "$Label.stderr"), $stderr, $encoding)
    [pscustomobject]@{ ExitCode = $process.ExitCode; Stdout = $stdout; Stderr = $stderr }
}

function Test-ConsumerCli(
    [string]$Executable,
    [string]$HostUrl,
    [string]$TokenFile,
    [string]$Project,
    [string]$EvidenceRoot
) {
    New-Item -ItemType Directory -Path $Project -Force | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $Project "package.json"),
        '{"scripts":{"test":"node --test"}}' + [Environment]::NewLine,
        [Text.UTF8Encoding]::new($false)
    )
    $base = @("--host", $HostUrl, "--token-file", $TokenFile, "-C", $Project)
    $results = [Collections.Generic.List[object]]::new()
    $baselineTasks = Invoke-ConsumerCliProcess $Executable ($base + @("task", "list", "--json")) $null $EvidenceRoot "consumer-baseline-tasks"
    $baselineProjects = Invoke-ConsumerCliProcess $Executable ($base + @("project", "list", "--json")) $null $EvidenceRoot "consumer-baseline-projects"
    if ($baselineTasks.ExitCode -ne 0 -or $baselineProjects.ExitCode -ne 0) {
        throw "Installed consumer CLI could not establish its durable baseline."
    }
    $baselineTaskCount = @((($baselineTasks.Stdout | ConvertFrom-Json).tasks)).Count
    $baselineProjectCount = @((($baselineProjects.Stdout | ConvertFrom-Json).projects)).Count

    $bare = Invoke-ConsumerCliProcess $Executable ($base + @("review", "this", "diff")) ("diff --git a/file.txt b/file.txt`n+private fixture content`n") $EvidenceRoot "consumer-bare"
    $results.Add($bare)
    if ($bare.ExitCode -eq 0 -or $bare.Stderr -notmatch "not inside a Git repository" `
        -or $bare.Stdout -match "private fixture content" -or $bare.Stderr -match "private fixture content") {
        throw "Installed consumer CLI non-Git safety contract failed."
    }

    $registered = Invoke-ConsumerCliProcess $Executable ($base + @("project", "add", $Project, "--initialize-git", "--json")) $null $EvidenceRoot "consumer-project-add"
    $results.Add($registered)
    if ($registered.ExitCode -ne 0) {
        throw "Installed consumer CLI could not explicitly initialize and register its project."
    }
    $registeredValue = $registered.Stdout | ConvertFrom-Json
    $projectId = [string]$registeredValue.project.project_id
    if ([string]::IsNullOrWhiteSpace($projectId) -or -not (Test-Path -LiteralPath (Join-Path $Project ".git") -PathType Container)) {
        throw "Installed consumer CLI project initialization returned invalid state."
    }

    $blockedGoals = @(
        "review this diff",
        "fix the tests",
        "inspect the project",
        "summarize this project"
    )
    $taskIds = [Collections.Generic.List[string]]::new()
    for ($index = 0; $index -lt $blockedGoals.Count; $index++) {
        $number = $index + 1
        $created = Invoke-ConsumerCliProcess $Executable ($base + @("task", "create", $blockedGoals[$index], "--project-id", $projectId, "--json")) $null $EvidenceRoot "consumer-task-$number-create"
        $results.Add($created)
        if ($created.ExitCode -ne 0) {
            throw "Installed consumer CLI could not create durable task $number."
        }
        $createdValue = $created.Stdout | ConvertFrom-Json
        if ($createdValue.task.goal -ne $blockedGoals[$index] -or $createdValue.task.project_id -ne $projectId) {
            throw "Installed consumer CLI durable task $number did not preserve its goal and project."
        }
        $taskId = [string]$createdValue.task.id
        [void]$taskIds.Add($taskId)

        $launched = Invoke-ConsumerCliProcess $Executable ($base + @("task", "launch", $taskId, "--json")) $null $EvidenceRoot "consumer-task-$number-launch"
        $results.Add($launched)
        if ($launched.ExitCode -ne 0) {
            throw "Installed consumer CLI could not launch durable task $number."
        }
        $launchedValue = $launched.Stdout | ConvertFrom-Json
        if ($launchedValue.task.id -ne $taskId -or $launchedValue.task.state -ne "blocked" `
            -or $launchedValue.execution.state -ne "blocked" `
            -or $launchedValue.execution.detail -notmatch "no discovered provider models") {
            throw "Installed consumer CLI no-provider task $number did not block deterministically."
        }
    }

    $events = Invoke-ConsumerCliProcess $Executable ($base + @("task", "events", $taskIds[0])) $null $EvidenceRoot "consumer-task-events"
    $results.Add($events)
    $eventValues = @(($events.Stdout | ConvertFrom-Json).events)
    if ($events.ExitCode -ne 0 -or $eventValues.Count -lt 2) {
        throw "Installed consumer CLI durable task event-history contract failed."
    }

    $oversized = Invoke-ConsumerCliProcess $Executable $base ("x" * 1048577) $EvidenceRoot "consumer-oversized"
    $results.Add($oversized)
    if ($oversized.ExitCode -eq 0 -or $oversized.Stderr -notmatch "1048576-byte limit") {
        throw "Installed consumer CLI input limit failed."
    }

    $tasks = Invoke-ConsumerCliProcess $Executable ($base + @("task", "list", "--json")) $null $EvidenceRoot "consumer-tasks"
    $projects = Invoke-ConsumerCliProcess $Executable ($base + @("project", "list", "--json")) $null $EvidenceRoot "consumer-projects"
    $results.Add($tasks)
    $results.Add($projects)
    $taskValues = @((($tasks.Stdout | ConvertFrom-Json).tasks))
    $projectValues = @((($projects.Stdout | ConvertFrom-Json).projects))
    if ($tasks.ExitCode -ne 0 -or $projects.ExitCode -ne 0 `
        -or ($taskValues.Count - $baselineTaskCount) -ne 4 `
        -or ($projectValues.Count - $baselineProjectCount) -ne 1) {
        throw "Installed consumer CLI did not reuse one durable project across four tasks."
    }

    $token = (Get-Content -LiteralPath $TokenFile -Raw).Trim()
    foreach ($result in $results) {
        if ($result.Stdout.Contains($token) -or $result.Stderr.Contains($token)) {
            throw "Installed consumer CLI exposed the local bearer token."
        }
    }

    $report = [ordered]@{
        schema_version = 1
        non_git_refused = $true
        explicit_git_initialization = $true
        durable_tasks_blocked_without_provider = $true
        durable_task_events = $true
        current_human_json_stream_contracts = "provider-success.json"
        oversized_rejected = $true
        task_count = $taskValues.Count
        project_count = $projectValues.Count
        created_task_count = $taskValues.Count - $baselineTaskCount
        created_project_count = $projectValues.Count - $baselineProjectCount
    }
    [IO.File]::WriteAllText(
        (Join-Path $EvidenceRoot "consumer-cli.json"),
        ($report | ConvertTo-Json -Compress),
        [Text.UTF8Encoding]::new($false)
    )
    [pscustomobject]$report
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
$nativeAccessibilityVerifier = Join-Path $PSScriptRoot "test-installed-native-accessibility-windows.ps1"
$nativeReviewedTopologyVerifier = Join-Path $PSScriptRoot "test-installed-native-reviewed-topology.py"
if (-not (Test-Path -LiteralPath $nativeAccessibilityVerifier -PathType Leaf)) {
    throw "Windows native accessibility verifier is missing: $nativeAccessibilityVerifier"
}
if (-not (Test-Path -LiteralPath $nativeReviewedTopologyVerifier -PathType Leaf)) {
    throw "Native reviewed-topology verifier is missing: $nativeReviewedTopologyVerifier"
}
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $tauriConfig = Get-Content -LiteralPath (Join-Path $repositoryRoot "apps/desktop/src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json
    $productName = [string]$tauriConfig.productName
    $version = [string]$tauriConfig.version
    if ([string]::IsNullOrWhiteSpace($productName) -or $version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
        throw "Tauri package identity is missing or invalid."
    }
    $InstallerPath = Join-Path $repositoryRoot "target/release/bundle/nsis/${productName}_${version}_x64-setup.exe"
    $compatibilityInstaller = Join-Path $repositoryRoot "target/release/bundle/nsis/MCG_${version}_x64-setup.exe"
    if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf) -and (Test-Path -LiteralPath $compatibilityInstaller -PathType Leaf)) {
        $InstallerPath = $compatibilityInstaller
    }
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
$previousWebView2Arguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$daemon = $null
$restartedDaemon = $null
$desktop = $null
$desktopService = $null
$nativePrepareService = $null
$guiHealth = $null
$screenshot = $null
$desktopScreenshot = $null
$nativeAccessibilityReport = $null
$installationRemoved = $true

try {
    $env:LOCALAPPDATA = $isolatedLocalAppData
    $install = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installRoot") -Wait -PassThru -WindowStyle Hidden
    if ($install.ExitCode -ne 0) {
        throw "Installer exited with code $($install.ExitCode)."
    }
    $installationRemoved = $false

    $requiredPayload = @("mcg-desktop.exe", "mcg-service.exe", "mcg.exe", "mcg-agent-mcp.exe", "mcg-action-runner.exe", "mcg-provider-runner.exe", "mcg-runtime-worker.exe", "mcg-plugin-runner.exe", "mcg-shadow-runner.exe", "mcg-updater.exe", "THIRD_PARTY_NOTICES.md", "Apache-2.0.txt", "Wasmtime-Apache-2.0-WITH-LLVM-exception.txt")
    $installed = Get-ChildItem -LiteralPath $installRoot -Recurse -File
    foreach ($name in $requiredPayload) {
        if ($installed.Name -notcontains $name) {
            throw "Installed payload is missing $name."
        }
    }

    $actionRunner = Join-Path $installRoot "mcg-action-runner.exe"
    $actionQualificationRoot = Join-Path $uatRoot "action-runner-qualification"
    New-Item -ItemType Directory -Path $actionQualificationRoot | Out-Null
    $actionQualificationStderr = Join-Path $uatRoot "action-runner-stderr.txt"
    $actionQualificationJson = (& $actionRunner --qualify-filesystem-root $actionQualificationRoot 2> $actionQualificationStderr) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "Packaged action runner qualification failed with exit code $LASTEXITCODE."
    }
    if ((Get-Item -LiteralPath $actionQualificationStderr).Length -ne 0) {
        throw "Packaged action runner wrote unexpected stderr."
    }
    $actionQualification = $actionQualificationJson | ConvertFrom-Json
    if ($actionQualification.created -ne "effect_confirmed" -or
        $actionQualification.reconciled -ne "observational" -or
        $actionQualification.duplicate -ne "no_effect_confirmed" -or
        $actionQualification.compensated -ne "effect_confirmed" -or
        -not $actionQualification.final_absent) {
        throw "Packaged action runner qualification report is incomplete."
    }
    if ((Get-ChildItem -LiteralPath $actionQualificationRoot -Force).Count -ne 0) {
        throw "Packaged action runner qualification left filesystem residue."
    }
    [IO.File]::WriteAllText((Join-Path $uatRoot "action-runner.json"), $actionQualificationJson, [Text.UTF8Encoding]::new($false))

    $runtimeWorker = Join-Path $installRoot "mcg-runtime-worker.exe"
    $runtimeQualificationJson = (& $runtimeWorker --qualify-runtime-worker) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "Packaged Runtime worker qualification failed with exit code $LASTEXITCODE."
    }
    $runtimeQualification = $runtimeQualificationJson | ConvertFrom-Json
    if ($runtimeQualification.protocol_version -ne 1 -or
        $runtimeQualification.role -ne "cognition" -or
        $runtimeQualification.storage_linked -ne $false -or
        $runtimeQualification.executable_hash.domain -ne "MetaCog.RuntimeWorkerExecutable") {
        throw "Packaged Runtime worker qualification report is incomplete."
    }
    [IO.File]::WriteAllText((Join-Path $uatRoot "runtime-worker.json"), $runtimeQualificationJson, [Text.UTF8Encoding]::new($false))

    $providerRunner = Join-Path $installRoot "mcg-provider-runner.exe"
    $providerQualificationJson = (& $providerRunner --qualify-provider-runner) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw "Packaged provider runner qualification failed with exit code $LASTEXITCODE."
    }
    $providerQualification = $providerQualificationJson | ConvertFrom-Json
    if ($providerQualification.protocol_version -ne 1 -or
        $providerQualification.role -ne "provider_adapter" -or
        $providerQualification.storage_linked -ne $false -or
        $providerQualification.credential_store -ne "operating_system" -or
        $providerQualification.physical_retry_attempts -ne 1) {
        throw "Packaged provider runner qualification report is incomplete."
    }
    [IO.File]::WriteAllText((Join-Path $uatRoot "provider-runner.json"), $providerQualificationJson, [Text.UTF8Encoding]::new($false))

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
    $daemon = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $uatRoot "service.stdout.log") `
        -RedirectStandardError (Join-Path $uatRoot "service.stderr.log")
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
    if ($PythonPath) {
        $python = [IO.Path]::GetFullPath($PythonPath)
        if (-not (Test-Path -LiteralPath $python -PathType Leaf)) {
            throw "PythonPath is not a regular file: $python"
        }
    } else {
        $python = (Get-Command python.exe -ErrorAction Stop).Source
    }
    $node = (Get-Command node.exe -ErrorAction Stop).Source
    $experimentVerifier = Join-Path $PSScriptRoot "test-installed-experiments.py"
    $experimentState = Join-Path $uatRoot "experiment-state.json"
    & $python $experimentVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --state $experimentState `
        --output (Join-Path $uatRoot "experiments.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed experiment lifecycle verification failed." }
    $providerSuccessVerifier = Join-Path $PSScriptRoot "test-installed-provider-success.py"
    $providerSuccessProject = Join-Path $uatRoot "provider-success-project"
    $providerSuccessState = Join-Path $uatRoot "provider-success-state.json"
    $providerSuccessReport = Join-Path $uatRoot "provider-success.json"
    & $python $providerSuccessVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $providerSuccessProject `
        --state $providerSuccessState --output $providerSuccessReport
    if ($LASTEXITCODE -ne 0) { throw "Installed provider-backed consumer success verification failed." }
    $memoryTransferVerifier = Join-Path $PSScriptRoot "test-installed-memory-transfer.py"
    $memoryTransferState = Join-Path $uatRoot "memory-transfer-state.json"
    $memoryTransferReport = Join-Path $uatRoot "memory-transfer.json"
    & $python $memoryTransferVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --workspace $uatRoot `
        --state $memoryTransferState --output $memoryTransferReport
    if ($LASTEXITCODE -ne 0) { throw "Installed cross-project memory transfer verification failed." }
    $organizationSharingVerifier = Join-Path $PSScriptRoot "test-installed-organization-sharing.mjs"
    $organizationSharingState = Join-Path $uatRoot "organization-sharing-state.json"
    $organizationSharingReport = Join-Path $uatRoot "organization-sharing.json"
    & $node $organizationSharingVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --state $organizationSharingState `
        --output $organizationSharingReport
    if ($LASTEXITCODE -ne 0) { throw "Installed signed-organization sharing verification failed." }
    $interfaceSuccessVerifier = Join-Path $PSScriptRoot "test-installed-interface-success.py"
    $interfaceSuccessState = Join-Path $uatRoot "interface-success-state.json"
    $interfaceSuccessReport = Join-Path $uatRoot "interface-success.json"
    & $python $interfaceSuccessVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --workspace $uatRoot `
        --state $interfaceSuccessState --output $interfaceSuccessReport
    if ($LASTEXITCODE -ne 0) { throw "Installed REST/OpenAI/MCP success verification failed." }
    $reviewedTopologyVerifier = Join-Path $PSScriptRoot "test-installed-reviewed-topology.py"
    $reviewedTopologyState = Join-Path $uatRoot "reviewed-topology-state.json"
    $reviewedTopologyReport = Join-Path $uatRoot "reviewed-topology.json"
    & $python $reviewedTopologyVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --workspace $uatRoot `
        --state $reviewedTopologyState --output $reviewedTopologyReport
    if ($LASTEXITCODE -ne 0) { throw "Installed reviewed-topology workflow verification failed." }
    $realProjectVerifier = Join-Path $PSScriptRoot "test-installed-real-project.py"
    $realProject = Join-Path $uatRoot "real-project"
    $realProjectState = Join-Path $uatRoot "real-project-state.json"
    & $python $realProjectVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realProject `
        --state $realProjectState --output (Join-Path $uatRoot "real-project.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative website project verification failed." }
    $realApiVerifier = Join-Path $PSScriptRoot "test-installed-real-api-project.py"
    $realApiProject = Join-Path $uatRoot "real-api-project"
    $realApiState = Join-Path $uatRoot "real-api-state.json"
    & $python $realApiVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realApiProject `
        --state $realApiState --output (Join-Path $uatRoot "real-api-project.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative JSON API project verification failed." }
    $realFullstackVerifier = Join-Path $PSScriptRoot "test-installed-real-fullstack-project.py"
    $realFullstackProject = Join-Path $uatRoot "homebase"
    $realFullstackState = Join-Path $uatRoot "real-fullstack-state.json"
    & $python $realFullstackVerifier --mode exercise --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realFullstackProject `
        --state $realFullstackState --output (Join-Path $uatRoot "real-fullstack-project.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative full-stack project verification failed." }
    $backupOutput = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile backup create --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI backup failed: $backupOutput" }
    $backup = ($backupOutput -join [Environment]::NewLine) | ConvertFrom-Json
    if ($backup.manifest.schema_version -ne 2 -or $backup.manifest.secrets_included -ne $false -or $null -ne $backup.manifest.secret_encryption -or $backup.size_bytes -le 0) { throw "Installed CLI backup returned invalid or unsafe metadata." }
    $backup | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $uatRoot "backup.json") -Encoding UTF8
    $encryptedBackupId = "00000000-0000-4000-8000-00000000b003"
    $encryptedPassphrase = "package fixture passphrase 2026 windows encrypted backup"
    $encryptedBackup = Invoke-RestMethod -Method Post `
        -Uri "http://127.0.0.1:$servicePort/api/v1/backups/$encryptedBackupId/encrypted" `
        -Headers @{ Authorization = "Bearer $((Get-Content -LiteralPath $tokenFile -Raw).Trim())" } `
        -ContentType "application/json" `
        -Body (@{ passphrase = $encryptedPassphrase } | ConvertTo-Json -Compress)
    $encryption = $encryptedBackup.manifest.secret_encryption
    if ($encryptedBackup.manifest.schema_version -ne 2 -or $encryptedBackup.manifest.secrets_included -ne $true `
        -or $encryption.schema_version -ne 1 -or $encryption.entry_path -ne "secrets/credentials.v1.enc" `
        -or $encryption.algorithm -ne "xchacha20-poly1305" -or $encryption.kdf -ne "argon2id" `
        -or $encryption.argon2_version -ne 19 -or $encryption.memory_kib -ne 65536 `
        -or $encryption.iterations -ne 3 -or $encryption.parallelism -ne 1 `
        -or $encryptedBackup.manifest.entries[-1].path -ne "secrets/credentials.v1.enc") {
        throw "Installed encrypted backup returned invalid metadata."
    }
    $encryptedBundle = Join-Path $serviceDataRoot "backups\$encryptedBackupId.mcgb"
    $bundleBytes = [IO.File]::ReadAllBytes($encryptedBundle)
    if ($bundleBytes.Length -ne $encryptedBackup.size_bytes `
        -or (Test-ContainsByteSequence $bundleBytes ([Text.Encoding]::UTF8.GetBytes($encryptedPassphrase))) `
        -or (Test-ContainsByteSequence $bundleBytes ([Text.Encoding]::UTF8.GetBytes((Get-Content -LiteralPath $tokenFile -Raw).Trim())))) {
        throw "Installed encrypted backup exposed protected plaintext or had the wrong size."
    }
    $encryptedBackup | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $uatRoot "encrypted-backup.json") -Encoding UTF8
    $encryptedVerifyOutput = & $cli backup verify $encryptedBundle --json 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI encrypted backup verification failed: $encryptedVerifyOutput" }
    $encryptedVerify = ($encryptedVerifyOutput -join [Environment]::NewLine) | ConvertFrom-Json
    if ($encryptedVerify.manifest.secrets_included -ne $true -or $encryptedVerify.sha256 -ne $encryptedBackup.sha256) {
        throw "Installed CLI encrypted backup verification returned inconsistent metadata."
    }
    $encryptedVerify | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $uatRoot "encrypted-backup-verified.json") -Encoding UTF8
    $consumerCli = Test-ConsumerCli $cli "http://127.0.0.1:$servicePort" $tokenFile (Join-Path $uatRoot "consumer-project") $uatRoot

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
    if ($toolNames -notcontains "mcg_run_task" -or $toolNames -notcontains "mcg_system_status" `
        -or $toolNames -notcontains "mcg_experiments_inspect" -or $toolNames -notcontains "mcg_experiments_mutate") {
        throw "Installed MCP tool catalog is incomplete."
    }
    @{ supportedVersion = "2026-07-28"; toolCount = $toolNames.Count } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $uatRoot "mcp.json") -Encoding UTF8
    $tokenBeforeRestore = [IO.File]::ReadAllBytes($tokenFile)
    $null = Stop-UatDaemonGracefully $daemon $cli "http://127.0.0.1:$servicePort" $tokenFile $serviceDataRoot (Join-Path $uatRoot "graceful-stop.json")
    $daemon = $null

    $restoreArguments = @("backup", "restore", $encryptedBundle, "--confirm", "--restore-secrets", "--passphrase-stdin")
    $wrongRestore = Invoke-ConsumerCliProcess $cli $restoreArguments "wrong package fixture passphrase`n" $uatRoot "encrypted-restore-wrong"
    if ($wrongRestore.ExitCode -eq 0 -or $wrongRestore.Stdout -match "wrong package fixture" -or $wrongRestore.Stderr -match "wrong package fixture") {
        throw "Installed encrypted restore did not reject or redact the wrong passphrase."
    }
    $correctRestore = Invoke-ConsumerCliProcess $cli $restoreArguments "$encryptedPassphrase`n" $uatRoot "encrypted-restore-correct"
    if ($correctRestore.ExitCode -ne 0 -or $correctRestore.Stdout -notmatch "with encrypted credentials" `
        -or $correctRestore.Stdout -match [regex]::Escape($encryptedPassphrase) `
        -or $correctRestore.Stderr -match [regex]::Escape($encryptedPassphrase)) {
        throw "Installed encrypted restore failed or exposed its passphrase."
    }
    if ([Convert]::ToBase64String($tokenBeforeRestore) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($tokenFile))) {
        throw "Installed encrypted restore changed the local daemon token."
    }
    $rollbackDatabases = @(Get-ChildItem -LiteralPath (Join-Path $serviceDataRoot "backups") -Filter "pre-restore-*.sqlite3" -File)
    if ($rollbackDatabases.Count -lt 1) { throw "Installed encrypted restore retained no rollback database." }
    @{ WrongPassphraseExit = $wrongRestore.ExitCode; RestoreExit = $correctRestore.ExitCode; RollbackDatabaseCount = $rollbackDatabases.Count; TokenUnchanged = $true } `
        | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $uatRoot "encrypted-restore.json") -Encoding UTF8

    $restartedDaemon = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $uatRoot "restarted-service.stdout.log") `
        -RedirectStandardError (Join-Path $uatRoot "restarted-service.stderr.log")
    $restartHealth = Wait-ForHealth $servicePort (Join-Path $uatRoot "restarted-health.json")
    $restartStatus = & $cli --host "http://127.0.0.1:$servicePort" --token-file $tokenFile status 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI failed after daemon restart: $restartStatus" }
    & $python $experimentVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --state $experimentState `
        --output (Join-Path $uatRoot "experiments-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed experiment history did not survive daemon restart." }
    & $python $providerSuccessVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $providerSuccessProject `
        --state $providerSuccessState --output (Join-Path $uatRoot "provider-success-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed provider-backed consumer evidence did not survive daemon restart." }
    & $python $memoryTransferVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --workspace $uatRoot `
        --state $memoryTransferState --output (Join-Path $uatRoot "memory-transfer-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed cross-project memory transfer did not survive daemon restart." }
    & $node $organizationSharingVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --state $organizationSharingState `
        --output (Join-Path $uatRoot "organization-sharing-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed signed-organization sharing did not survive daemon restart." }
    & $python $interfaceSuccessVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --workspace $uatRoot `
        --state $interfaceSuccessState --output (Join-Path $uatRoot "interface-success-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed REST/OpenAI/MCP evidence did not survive daemon restart." }
    & $python $reviewedTopologyVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --workspace $uatRoot `
        --state $reviewedTopologyState --output (Join-Path $uatRoot "reviewed-topology-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed reviewed-topology evidence did not survive daemon restart." }
    & $python $realProjectVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realProject `
        --state $realProjectState --output (Join-Path $uatRoot "real-project-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative website evidence did not survive daemon restart." }
    & $python $realApiVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realApiProject `
        --state $realApiState --output (Join-Path $uatRoot "real-api-project-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative JSON API evidence did not survive daemon restart." }
    & $python $realFullstackVerifier --mode verify --host "http://127.0.0.1:$servicePort" `
        --token-file $tokenFile --cli $cli --project $realFullstackProject `
        --state $realFullstackState --output (Join-Path $uatRoot "real-fullstack-project-restart.json")
    if ($LASTEXITCODE -ne 0) { throw "Installed representative full-stack evidence did not survive daemon restart." }
    $null = Stop-UatDaemonGracefully $restartedDaemon $cli "http://127.0.0.1:$servicePort" $tokenFile $serviceDataRoot (Join-Path $uatRoot "restarted-graceful-stop.json")
    $restartedDaemon = $null

    if (-not $SkipDesktop) {
        $guiPort = 7777
        if (Get-NetTCPConnection -LocalPort $guiPort -State Listen -ErrorAction SilentlyContinue) {
            throw "Packaged desktop UAT requires unused loopback port 7777."
        }
        $env:MCG_DATA_DIR = $guiDataRoot
        $env:MCG_BIND_ADDRESS = "127.0.0.1:$guiPort"
        # Chromium enables its complete native accessibility tree when assistive
        # technology is active. Force that same mode for deterministic headless
        # UI Automation qualification without changing the installed payload.
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--force-renderer-accessibility"
        $guiTokenFile = Join-Path $guiDataRoot "local-auth-token"
        $nativeReviewedTopologyState = Join-Path $uatRoot "native-reviewed-topology-state.json"
        $nativeReviewedTopologyReport = Join-Path $uatRoot "native-reviewed-topology-prepare.json"
        $nativePrepareService = Start-Process -FilePath (Join-Path $installRoot "mcg-service.exe") -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $uatRoot "native-prepare-service.stdout.log") `
            -RedirectStandardError (Join-Path $uatRoot "native-prepare-service.stderr.log")
        $null = Wait-ForHealth $guiPort (Join-Path $uatRoot "native-prepare-health.json")
        & $python $nativeReviewedTopologyVerifier --mode prepare --host "http://127.0.0.1:$guiPort" `
            --token-file $guiTokenFile --workspace $uatRoot --state $nativeReviewedTopologyState `
            --output $nativeReviewedTopologyReport
        if ($LASTEXITCODE -ne 0) { throw "Native reviewed-topology preparation failed." }
        $null = Stop-UatDaemonGracefully $nativePrepareService $cli "http://127.0.0.1:$guiPort" $guiTokenFile $guiDataRoot (Join-Path $uatRoot "native-prepare-service-stop.json")
        $nativePrepareService = $null
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
        $nativeAccessibilityReport = Join-Path $uatRoot "desktop-native-accessibility.json"
        $nativeReviewedTopologyMarker = Join-Path $uatRoot "native-reviewed-topology-project\execution-count.txt"
        & $nativeAccessibilityVerifier -ProcessId $desktop.Id -OutputPath $nativeAccessibilityReport `
            -ReviewedTopologyMarker $nativeReviewedTopologyMarker `
            -ServiceHost "http://127.0.0.1:$guiPort" -TokenFile $guiTokenFile
        $nativeReviewedTopologyVerifyReport = Join-Path $uatRoot "native-reviewed-topology-verify.json"
        & $python $nativeReviewedTopologyVerifier --mode verify --host "http://127.0.0.1:$guiPort" `
            --token-file $guiTokenFile --workspace $uatRoot --state $nativeReviewedTopologyState `
            --output $nativeReviewedTopologyVerifyReport
        if ($LASTEXITCODE -ne 0) { throw "Native reviewed-topology verification failed." }
        $nativeReviewedTopologyScreenshot = Join-Path $uatRoot "desktop-reviewed-topology.png"
        $null = Save-DesktopScreenshot $nativeReviewedTopologyScreenshot
        Stop-UatProcess $desktop
        $desktop = $null
        $null = Stop-UatDaemonGracefully $desktopService $cli "http://127.0.0.1:$guiPort" $guiTokenFile $guiDataRoot (Join-Path $uatRoot "desktop-service-graceful-stop.json")
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
        EncryptedBackupSchema = $encryptedBackup.manifest.schema_version
        EncryptedRestoreReport = Join-Path $uatRoot "encrypted-restore.json"
        GracefulShutdownQualified = $true
        ConsumerCliReport = Join-Path $uatRoot "consumer-cli.json"
        ConsumerCliTaskCount = $consumerCli.task_count
        ConsumerCliProjectCount = $consumerCli.project_count
        ProviderSuccessReport = $providerSuccessReport
        ProviderSuccessRestartReport = Join-Path $uatRoot "provider-success-restart.json"
        MemoryTransferReport = $memoryTransferReport
        MemoryTransferRestartReport = Join-Path $uatRoot "memory-transfer-restart.json"
        OrganizationSharingReport = $organizationSharingReport
        OrganizationSharingRestartReport = Join-Path $uatRoot "organization-sharing-restart.json"
        InterfaceSuccessReport = $interfaceSuccessReport
        InterfaceSuccessRestartReport = Join-Path $uatRoot "interface-success-restart.json"
        ReviewedTopologyReport = $reviewedTopologyReport
        ReviewedTopologyRestartReport = Join-Path $uatRoot "reviewed-topology-restart.json"
        RealProjectReport = Join-Path $uatRoot "real-project.json"
        RealProjectRestartReport = Join-Path $uatRoot "real-project-restart.json"
        RealApiProjectReport = Join-Path $uatRoot "real-api-project.json"
        RealApiProjectRestartReport = Join-Path $uatRoot "real-api-project-restart.json"
        RealFullstackProjectReport = Join-Path $uatRoot "real-fullstack-project.json"
        RealFullstackProjectRestartReport = Join-Path $uatRoot "real-fullstack-project-restart.json"
        McpToolCount = $toolNames.Count
        DesktopScreenshot = $desktopScreenshot
        NativeAccessibilityReport = $nativeAccessibilityReport
        NativeReviewedTopologyReport = $nativeReviewedTopologyVerifyReport
        NativeReviewedTopologyScreenshot = $nativeReviewedTopologyScreenshot
        ScreenshotWidth = if ($null -eq $screenshot) { 0 } else { $screenshot.Width }
        ScreenshotHeight = if ($null -eq $screenshot) { 0 } else { $screenshot.Height }
        ScreenshotSampledColors = if ($null -eq $screenshot) { 0 } else { $screenshot.SampledColors }
        DesktopSkipped = [bool]$SkipDesktop
        DataRetained = $true
        RepairPreservedData = $true
        ReinstallPreservedData = $true
        PluginRunnerQualified = $true
        ActionRunnerQualified = $true
        AppExecutableRemoved = $true
    }
} finally {
    Stop-UatProcess $desktop
    Stop-UatProcess $desktopService
    Stop-UatProcess $nativePrepareService
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
    if ($null -eq $previousWebView2Arguments) { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue } else { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousWebView2Arguments }
}
