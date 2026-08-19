[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Snapshot", "SetScenario", "ImportCa", "RemoveCa", "DenyWrites", "RestoreAcl")]
    [string]$Action,

    [Parameter(Mandatory = $true)]
    [string]$QualificationRoot,

    [ValidateSet("no-update", "valid", "changed-version", "bad-signature", "corrupt-json", "unavailable")]
    [string]$Scenario,

    [ValidatePattern("^[a-z0-9][a-z0-9-]{0,63}$")]
    [string]$Lane,

    [string]$InstallRoot,

    [string]$DataRoot,

    [string]$CaPath,

    [ValidateSet("backup", "install")]
    [string]$FaultTarget,

    [ValidatePattern("^[A-Fa-f0-9]{64}$")]
    [string]$ExpectedCaDerSha256,

    [ValidatePattern("^[A-Fa-f0-9]{40}$")]
    [string]$ExpectedCaThumbprint,

    [switch]$ConfirmTrustMutation
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-CanonicalDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not [IO.Path]::IsPathRooted($Path)) {
        throw "$Label must be an absolute path."
    }
    $full = [IO.Path]::GetFullPath($Path)
    $item = Get-Item -LiteralPath $full -Force -ErrorAction Stop
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "$Label must be a canonical non-reparse directory."
    }
    if ($item.FullName -ne $full) {
        throw "$Label must be canonical."
    }
    return $item.FullName
}

function Get-RegularFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label,
        [long]$MaximumBytes = 65536
    )

    if (-not [IO.Path]::IsPathRooted($Path)) {
        throw "$Label must be an absolute path."
    }
    $full = [IO.Path]::GetFullPath($Path)
    $item = Get-Item -LiteralPath $full -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        $item.Length -le 0 -or $item.Length -gt $MaximumBytes) {
        throw "$Label must be a bounded regular non-reparse file."
    }
    return $item.FullName
}

function Assert-ChildPath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $rootPrefix = $Root.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $Path.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Label escaped the qualification root."
    }
}

function Get-Sha256 {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($algorithm.ComputeHash($Bytes))).Replace("-", "")
    }
    finally {
        $algorithm.Dispose()
    }
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
    return Get-Sha256 ([Text.Encoding]::UTF8.GetBytes($canonical))
}

function Write-Evidence {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][object]$Payload
    )

    if ($Name -notmatch "^[a-z0-9][a-z0-9.-]{0,95}\.json$") {
        throw "Evidence name is invalid."
    }
    $directory = Join-Path $script:Root "evidence\windows-updater-harness"
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $directory = Get-CanonicalDirectory $directory "evidence directory"
    $path = Join-Path $directory $Name
    if (Test-Path -LiteralPath $path) {
        throw "Evidence output already exists: $path"
    }
    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($path, (($Payload | ConvertTo-Json -Depth 10) + "`n"), $encoding)
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    Write-Output ([ordered]@{ evidence = $path; sha256 = $hash } | ConvertTo-Json -Compress)
}

function Require-Value {
    param([string]$Value, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "$Label is required for action $Action."
    }
}

$Root = Get-CanonicalDirectory $QualificationRoot "qualification root"
$requiredArtifact = Join-Path $Root "artifacts\baseline\release-manifest.json"
$requiredControl = Join-Path $Root "feed\control.json"
$requiredInputs = if ($Action -eq "RemoveCa") {
    @()
}
elseif ($Action -in @("SetScenario", "Snapshot")) {
    @($requiredArtifact, $requiredControl)
}
else {
    @($requiredArtifact)
}
foreach ($required in $requiredInputs) {
    Assert-ChildPath $Root ([IO.Path]::GetFullPath($required)) "qualification input"
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Qualification root is missing required input: $required"
    }
}

if ($Action -eq "SetScenario") {
    Require-Value $Scenario "Scenario"
    Require-Value $Lane "Lane"
    $control = Get-RegularFile $requiredControl "scenario control" 4096
    $swapId = [Guid]::NewGuid().ToString("N")
    $temporary = Join-Path (Split-Path -Parent $control) (".control." + $swapId + ".tmp")
    $backup = Join-Path (Split-Path -Parent $control) (".control." + $swapId + ".bak")
    $encoding = New-Object Text.UTF8Encoding($false)
    try {
        [IO.File]::WriteAllText($temporary, ('{"scenario":"' + $Scenario + '"}' + "`n"), $encoding)
        [IO.File]::Replace($temporary, $control, $backup, $true)
    }
    finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
        if (Test-Path -LiteralPath $backup) {
            Remove-Item -LiteralPath $backup -Force
        }
    }
    $selected = (Get-Content -LiteralPath $control -Raw | ConvertFrom-Json).scenario
    if ($selected -ne $Scenario) {
        throw "Scenario control did not retain the requested value."
    }
    Write-Evidence ("scenario-" + $Lane + "-" + $Scenario + ".json") ([ordered]@{
        schema_version = 1
        action = "set-scenario"
        scenario = $Scenario
        control_sha256 = (Get-FileHash -LiteralPath $control -Algorithm SHA256).Hash
        changed_at = (Get-Date).ToUniversalTime().ToString("o")
    })
    exit 0
}

if ($Action -eq "Snapshot") {
    Require-Value $Lane "Lane"
    Require-Value $InstallRoot "InstallRoot"
    Require-Value $DataRoot "DataRoot"
    $install = Get-CanonicalDirectory $InstallRoot "install root"
    $data = Get-CanonicalDirectory $DataRoot "data root"
    Assert-ChildPath $Root $install "install root"
    Assert-ChildPath $Root $data "data root"

    $payloads = @(Get-ChildItem -LiteralPath $install -File -Recurse | Sort-Object FullName | ForEach-Object {
        [ordered]@{
            relative_path = $_.FullName.Substring($install.Length + 1)
            size_bytes = $_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    })
    $backupsPath = Join-Path $data "backups"
    $backups = @(if (Test-Path -LiteralPath $backupsPath -PathType Container) {
        Get-ChildItem -LiteralPath $backupsPath -File | Sort-Object Name | ForEach-Object {
            [ordered]@{ name = $_.Name; size_bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
        }
    }
    else { })
    $temporaryFiles = @(Get-ChildItem -LiteralPath $data -File -Recurse | Where-Object {
        $_.Name -match "(\.tmp$|\.part$|\.mcgb-[A-Za-z0-9.-]+$)"
    } | Sort-Object FullName | ForEach-Object {
        [ordered]@{ relative_path = $_.FullName.Substring($data.Length + 1); size_bytes = $_.Length }
    })
    $processes = @(Get-CimInstance Win32_Process | Where-Object {
        $_.ExecutablePath -and $_.ExecutablePath.StartsWith($install + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
    } | Sort-Object ProcessId | ForEach-Object {
        [ordered]@{ process_id = $_.ProcessId; parent_process_id = $_.ParentProcessId; name = $_.Name }
    })
    $registration = @(Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
        Where-Object DisplayName -eq "MCG" | ForEach-Object {
            [ordered]@{ display_name = $_.DisplayName; display_version = $_.DisplayVersion; install_location = $_.InstallLocation }
        })
    $listeners = @(Get-NetTCPConnection -LocalPort 7777 -State Listen -ErrorAction SilentlyContinue | ForEach-Object {
        [ordered]@{ address = $_.LocalAddress; port = $_.LocalPort; owning_process = $_.OwningProcess }
    })
    $trustMatches = 0
    if (-not [string]::IsNullOrWhiteSpace($ExpectedCaThumbprint)) {
        $thumbprint = $ExpectedCaThumbprint.ToUpperInvariant()
        $trustMatches = @(Get-ChildItem Cert:\CurrentUser\Root | Where-Object Thumbprint -eq $thumbprint).Count
    }
    $disk = Get-PSDrive -Name ([IO.Path]::GetPathRoot($Root).Substring(0, 1))
    Write-Evidence ("snapshot-" + $Lane + ".json") ([ordered]@{
        schema_version = 1
        lane = $Lane
        captured_at = (Get-Date).ToUniversalTime().ToString("o")
        registration = $registration
        payloads = $payloads
        backups = $backups
        temporary_files = $temporaryFiles
        rooted_processes = $processes
        port_7777_listeners = $listeners
        current_user_ca_matches = $trustMatches
        free_bytes = [long]$disk.Free
    })
    exit 0
}

if ($Action -in @("ImportCa", "RemoveCa")) {
    if (-not $ConfirmTrustMutation) {
        throw "$Action requires -ConfirmTrustMutation after action-time owner confirmation."
    }
    Require-Value $ExpectedCaDerSha256 "ExpectedCaDerSha256"
    Require-Value $ExpectedCaThumbprint "ExpectedCaThumbprint"
    $expectedDer = $ExpectedCaDerSha256.ToUpperInvariant()
    $expectedThumbprint = $ExpectedCaThumbprint.ToUpperInvariant()
    $expectedSubject = "CN=MCG disposable updater UAT CA"

    if ($Action -eq "ImportCa") {
        Require-Value $CaPath "CaPath"
        $caPath = Get-RegularFile $CaPath "disposable CA" 65536
        Assert-ChildPath $Root $caPath "disposable CA"
        $certificate = New-Object Security.Cryptography.X509Certificates.X509Certificate2($caPath)
        $actualDer = Get-Sha256 $certificate.RawData
        if ($certificate.Subject -ne $expectedSubject -or
            $actualDer -ne $expectedDer -or $certificate.Thumbprint -ne $expectedThumbprint) {
            throw "Disposable CA identity does not match the confirmed certificate."
        }
        $subjectMatches = @(Get-ChildItem Cert:\CurrentUser\Root | Where-Object Subject -eq $expectedSubject)
        $thumbprintMatches = @($subjectMatches | Where-Object Thumbprint -eq $expectedThumbprint)
        if ($subjectMatches.Count -ne 0 -or $thumbprintMatches.Count -ne 0) {
            throw "Import refused because a matching disposable CA already exists."
        }
        $imported = Import-Certificate -FilePath $caPath -CertStoreLocation Cert:\CurrentUser\Root
        if ($imported.Thumbprint -ne $expectedThumbprint -or
            @(Get-ChildItem Cert:\CurrentUser\Root | Where-Object Thumbprint -eq $expectedThumbprint).Count -ne 1) {
            throw "Disposable CA import did not converge on exactly one confirmed certificate."
        }
        Write-Evidence "trust-import.json" ([ordered]@{
            schema_version = 1
            action = "import"
            subject = $expectedSubject
            thumbprint = $expectedThumbprint
            der_sha256 = $actualDer
            changed_at = (Get-Date).ToUniversalTime().ToString("o")
        })
        exit 0
    }

    $subjectMatches = @(Get-ChildItem Cert:\CurrentUser\Root | Where-Object Subject -eq $expectedSubject)
    $thumbprintMatches = @($subjectMatches | Where-Object Thumbprint -eq $expectedThumbprint)
    if ($thumbprintMatches.Count -ne 1 -or $subjectMatches.Count -ne 1) {
        throw "Removal refused because the confirmed disposable CA is not the only matching certificate."
    }
    $actualDer = Get-Sha256 $thumbprintMatches[0].RawData
    if ($actualDer -ne $expectedDer) {
        throw "Removal refused because the trusted certificate bytes changed."
    }
    Remove-Item -LiteralPath ("Cert:\CurrentUser\Root\" + $expectedThumbprint) -Force
    if (@(Get-ChildItem Cert:\CurrentUser\Root | Where-Object Thumbprint -eq $expectedThumbprint).Count -ne 0) {
        throw "Disposable CA removal did not complete."
    }
    Write-Evidence "trust-remove.json" ([ordered]@{
        schema_version = 1
        action = "remove"
        subject = $expectedSubject
        thumbprint = $expectedThumbprint
        der_sha256 = $actualDer
        changed_at = (Get-Date).ToUniversalTime().ToString("o")
    })
    exit 0
}

if ($Action -in @("DenyWrites", "RestoreAcl")) {
    Require-Value $FaultTarget "FaultTarget"
    Require-Value $InstallRoot "InstallRoot"
    Require-Value $DataRoot "DataRoot"
    $install = Get-CanonicalDirectory $InstallRoot "install root"
    $data = Get-CanonicalDirectory $DataRoot "data root"
    Assert-ChildPath $Root $install "install root"
    Assert-ChildPath $Root $data "data root"
    $target = if ($FaultTarget -eq "install") { $install } else { Join-Path $data "backups" }
    if ($FaultTarget -eq "backup" -and -not (Test-Path -LiteralPath $target)) {
        New-Item -ItemType Directory -Path $target | Out-Null
    }
    $target = Get-CanonicalDirectory $target "$FaultTarget fault target"
    Assert-ChildPath $Root $target "$FaultTarget fault target"
    $stateDirectory = Join-Path $Root "harness-state"
    New-Item -ItemType Directory -Path $stateDirectory -Force | Out-Null
    $stateDirectory = Get-CanonicalDirectory $stateDirectory "harness state directory"
    $statePath = Join-Path $stateDirectory ("acl-" + $FaultTarget + ".json")

    if ($Action -eq "DenyWrites") {
        if (Test-Path -LiteralPath $statePath) {
            throw "An unrestored $FaultTarget ACL fault already exists."
        }
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
        $acl = Get-Acl -LiteralPath $target
        $accessSection = [Security.AccessControl.AccessControlSections]::Access
        $encoding = New-Object Text.UTF8Encoding($false)
        [IO.File]::WriteAllText($statePath, ([ordered]@{
            schema_version = 1
            target = $target
            owner_sid = $identity.Value
            original_sddl = $acl.GetSecurityDescriptorSddlForm($accessSection)
            original_access_fingerprint = Get-AccessFingerprint $acl
        } | ConvertTo-Json -Compress), $encoding)
        $rights = [Security.AccessControl.FileSystemRights]::CreateFiles -bor
            [Security.AccessControl.FileSystemRights]::CreateDirectories -bor
            [Security.AccessControl.FileSystemRights]::WriteData -bor
            [Security.AccessControl.FileSystemRights]::AppendData -bor
            [Security.AccessControl.FileSystemRights]::WriteAttributes -bor
            [Security.AccessControl.FileSystemRights]::WriteExtendedAttributes -bor
            [Security.AccessControl.FileSystemRights]::Delete -bor
            [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles
        $inheritance = [Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [Security.AccessControl.InheritanceFlags]::ObjectInherit
        $rule = New-Object Security.AccessControl.FileSystemAccessRule(
            $identity,
            $rights,
            $inheritance,
            [Security.AccessControl.PropagationFlags]::None,
            [Security.AccessControl.AccessControlType]::Deny
        )
        $acl.AddAccessRule($rule) | Out-Null
        (Get-Item -LiteralPath $target -Force).SetAccessControl($acl)
        Write-Evidence ("acl-" + $FaultTarget + "-deny.json") ([ordered]@{
            schema_version = 1
            action = "deny-writes"
            target = $FaultTarget
            owner_sid = $identity.Value
            changed_at = (Get-Date).ToUniversalTime().ToString("o")
        })
        exit 0
    }

    $stateFile = Get-RegularFile $statePath "$FaultTarget ACL state" 65536
    $state = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
    if ($state.target -ne $target -or $state.owner_sid -ne [Security.Principal.WindowsIdentity]::GetCurrent().User.Value) {
        throw "ACL restoration state does not match the current target and owner."
    }
    $restored = New-Object Security.AccessControl.DirectorySecurity
    $accessSection = [Security.AccessControl.AccessControlSections]::Access
    $restored.SetSecurityDescriptorSddlForm($state.original_sddl, $accessSection)
    (Get-Item -LiteralPath $target -Force).SetAccessControl($restored)
    $restoredAcl = Get-Acl -LiteralPath $target
    if ((Get-AccessFingerprint $restoredAcl) -ne $state.original_access_fingerprint) {
        throw "ACL restoration did not reproduce the original access semantics."
    }
    $archivedState = Join-Path (Join-Path $Root "evidence\windows-updater-harness") ("acl-" + $FaultTarget + "-restored-state.json")
    if (Test-Path -LiteralPath $archivedState) {
        throw "Restored ACL state evidence already exists."
    }
    Move-Item -LiteralPath $stateFile -Destination $archivedState
    Write-Evidence ("acl-" + $FaultTarget + "-restore.json") ([ordered]@{
        schema_version = 1
        action = "restore-acl"
        target = $FaultTarget
        restored_at = (Get-Date).ToUniversalTime().ToString("o")
    })
    exit 0
}

throw "Unsupported action."
