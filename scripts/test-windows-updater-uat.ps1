[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Invoke-Harness {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [switch]$ExpectFailure
    )

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $script:PowerShellExecutable -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass `
            -File $script:HarnessPath @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($ExpectFailure) {
        if ($exitCode -eq 0) {
            throw "Harness unexpectedly accepted a rejected action: $($Arguments -join ' ')"
        }
    }
    elseif ($exitCode -ne 0) {
        throw "Harness failed with exit code ${exitCode}: $($output -join [Environment]::NewLine)"
    }
    return @($output)
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Get-AccessFingerprint {
    param([Parameter(Mandatory = $true)][Security.AccessControl.DirectorySecurity]$Acl)

    $rules = @($Acl.GetAccessRules(
        $true,
        $true,
        [Security.Principal.SecurityIdentifier]
    ) | ForEach-Object {
        "{0}|{1}|{2}|{3}|{4}|{5}" -f @(
            $_.IdentityReference.Value,
            [int]$_.AccessControlType,
            [int64]$_.FileSystemRights,
            [int]$_.InheritanceFlags,
            [int]$_.PropagationFlags,
            [int]$_.IsInherited
        )
    } | Sort-Object)
    $canonical = "protected=$([int]$Acl.AreAccessRulesProtected)`n" + ($rules -join "`n")
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString(
            $algorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes($canonical))
        )).Replace("-", "")
    }
    finally {
        $algorithm.Dispose()
    }
}

$HarnessPath = Join-Path $PSScriptRoot "windows-updater-uat.ps1"
if (-not (Test-Path -LiteralPath $HarnessPath -PathType Leaf)) {
    throw "Windows updater UAT harness is missing."
}
$HarnessPath = (Get-Item -LiteralPath $HarnessPath -Force).FullName
$PowerShellExecutable = (Get-Process -Id $PID).Path
if (-not (Test-Path -LiteralPath $PowerShellExecutable -PathType Leaf)) {
    throw "The current PowerShell executable could not be resolved."
}

$temporaryBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$fixtureName = "mcg-windows-updater-uat-test-" + [Guid]::NewGuid().ToString("N")
$fixtureRoot = Join-Path $temporaryBase $fixtureName
$backupAclRestored = $false

try {
    $baselineDirectory = Join-Path $fixtureRoot "artifacts\baseline"
    $feedDirectory = Join-Path $fixtureRoot "feed"
    $installRoot = Join-Path $fixtureRoot "install"
    $dataRoot = Join-Path $fixtureRoot "data"
    $backupsRoot = Join-Path $dataRoot "backups"
    foreach ($directory in @($baselineDirectory, $feedDirectory, $installRoot, $backupsRoot)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }

    Write-Utf8File (Join-Path $baselineDirectory "release-manifest.json") "{`"schema_version`":1}`n"
    Write-Utf8File (Join-Path $feedDirectory "control.json") "{`"scenario`":`"no-update`"}`n"
    Write-Utf8File (Join-Path $installRoot "payload.bin") "sealed fixture payload`n"
    Write-Utf8File (Join-Path $backupsRoot "existing.mcgb") "existing backup fixture`n"

    Invoke-Harness @(
        "-Action", "SetScenario",
        "-QualificationRoot", $fixtureRoot,
        "-Scenario", "changed-version",
        "-Lane", "fixture"
    ) | Out-Null
    $control = Get-Content -LiteralPath (Join-Path $feedDirectory "control.json") -Raw | ConvertFrom-Json
    Assert-Condition ($control.scenario -eq "changed-version") "Scenario switching did not converge."
    $scenarioEvidence = Join-Path $fixtureRoot "evidence\windows-updater-harness\scenario-fixture-changed-version.json"
    Assert-Condition (Test-Path -LiteralPath $scenarioEvidence -PathType Leaf) "Scenario evidence is missing."

    Invoke-Harness @(
        "-Action", "Snapshot",
        "-QualificationRoot", $fixtureRoot,
        "-InstallRoot", $installRoot,
        "-DataRoot", $dataRoot,
        "-Lane", "fixture"
    ) | Out-Null
    $snapshotPath = Join-Path $fixtureRoot "evidence\windows-updater-harness\snapshot-fixture.json"
    $snapshot = Get-Content -LiteralPath $snapshotPath -Raw | ConvertFrom-Json
    Assert-Condition ($snapshot.schema_version -eq 1) "Snapshot schema is incorrect."
    Assert-Condition ($snapshot.payloads.Count -eq 1) "Snapshot did not retain the exact fixture payload."
    Assert-Condition ($snapshot.payloads[0].relative_path -eq "payload.bin") "Snapshot exposed a non-relative payload path."
    Assert-Condition ($snapshot.backups.Count -eq 1) "Snapshot did not retain the exact fixture backup."
    Assert-Condition ($snapshot.temporary_files.Count -eq 0) "Snapshot reported unexpected temporary residue."
    $snapshotText = Get-Content -LiteralPath $snapshotPath -Raw
    foreach ($forbidden in @("auth_token", "access_token", "refresh_token", "password", "private_key", "command_line")) {
        Assert-Condition ($snapshotText.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -lt 0) `
            "Snapshot evidence contains forbidden field text: $forbidden"
    }

    $accessSection = [Security.AccessControl.AccessControlSections]::Access
    $originalAcl = Get-Acl -LiteralPath $backupsRoot
    $originalSddl = $originalAcl.GetSecurityDescriptorSddlForm($accessSection)
    $originalFingerprint = Get-AccessFingerprint $originalAcl
    Invoke-Harness @(
        "-Action", "DenyWrites",
        "-QualificationRoot", $fixtureRoot,
        "-InstallRoot", $installRoot,
        "-DataRoot", $dataRoot,
        "-FaultTarget", "backup"
    ) | Out-Null
    $writeWasRejected = $false
    try {
        Write-Utf8File (Join-Path $backupsRoot "must-not-exist.mcgb") "write should be denied"
    }
    catch {
        $writeWasRejected = $true
    }
    Assert-Condition $writeWasRejected "The disposable backup ACL fault did not reject writes."

    Invoke-Harness @(
        "-Action", "RestoreAcl",
        "-QualificationRoot", $fixtureRoot,
        "-InstallRoot", $installRoot,
        "-DataRoot", $dataRoot,
        "-FaultTarget", "backup"
    ) | Out-Null
    Assert-Condition ((Get-AccessFingerprint (Get-Acl -LiteralPath $backupsRoot)) -eq $originalFingerprint) `
        "The disposable backup access semantics were not restored exactly."
    $backupAclRestored = $true
    Write-Utf8File (Join-Path $backupsRoot "post-restore.mcgb") "writes restored`n"
    Assert-Condition (Test-Path -LiteralPath (Join-Path $backupsRoot "post-restore.mcgb") -PathType Leaf) `
        "Writes did not resume after ACL restoration."

    Invoke-Harness @(
        "-Action", "ImportCa",
        "-QualificationRoot", $fixtureRoot
    ) -ExpectFailure | Out-Null

    $evidenceFiles = @(Get-ChildItem -LiteralPath (Join-Path $fixtureRoot "evidence\windows-updater-harness") -File)
    $expectedEvidenceNames = @(
        "acl-backup-deny.json",
        "acl-backup-restore.json",
        "acl-backup-restored-state.json",
        "scenario-fixture-changed-version.json",
        "snapshot-fixture.json"
    )
    Assert-Condition ($evidenceFiles.Count -eq $expectedEvidenceNames.Count) `
        "The harness produced an unexpected evidence-file count."
    Assert-Condition (-not (Compare-Object $expectedEvidenceNames @($evidenceFiles.Name))) `
        "The harness produced unexpected evidence names."
    foreach ($evidenceFile in $evidenceFiles) {
        $null = Get-Content -LiteralPath $evidenceFile.FullName -Raw | ConvertFrom-Json
    }

    Write-Output "Windows updater UAT harness fixture passed with atomic scenario, snapshot, trust guard, and reversible ACL evidence."
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot -PathType Container) {
        $statePath = Join-Path $fixtureRoot "harness-state\acl-backup.json"
        if (-not $backupAclRestored -and (Test-Path -LiteralPath $statePath -PathType Leaf) -and
            (Test-Path -LiteralPath $backupsRoot -PathType Container)) {
            $state = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
            $restored = New-Object Security.AccessControl.DirectorySecurity
            $accessSection = [Security.AccessControl.AccessControlSections]::Access
            $restored.SetSecurityDescriptorSddlForm($state.original_sddl, $accessSection)
            (Get-Item -LiteralPath $backupsRoot -Force).SetAccessControl($restored)
        }

        $resolved = (Get-Item -LiteralPath $fixtureRoot -Force).FullName
        $expectedPrefix = $temporaryBase + [IO.Path]::DirectorySeparatorChar
        if (-not $resolved.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf $resolved) -notmatch "^mcg-windows-updater-uat-test-[a-f0-9]{32}$" -or
            ((Get-Item -LiteralPath $resolved -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Refusing to remove an unexpected fixture path: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
