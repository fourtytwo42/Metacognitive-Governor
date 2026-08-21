param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [int]$TimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"
$Canary = "MCG_NATIVE_SECRET_CANARY"
$assemblyPath = "C:\Windows\Microsoft.NET\Framework64\v4.0.30319\WPF\UIAutomationClient.dll"
if (-not (Test-Path -LiteralPath $assemblyPath -PathType Leaf)) {
    throw "Windows UI Automation assembly is missing: $assemblyPath"
}
[void][Reflection.Assembly]::LoadFrom($assemblyPath)
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class McgNativeWindow {
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr handle);
}
"@

function Get-Deadline() {
    [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
}

function Get-Elements([System.Windows.Automation.AutomationElement]$Root) {
    $values = [Collections.Generic.List[object]]::new()
    $values.Add($Root)
    $collection = $Root.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    if ($collection.Count -gt 2000) { throw "UI Automation tree exceeded the 2000-node safety bound." }
    foreach ($element in $collection) { $values.Add($element) }
    $values
}

function Get-ElementSnapshot([System.Windows.Automation.AutomationElement]$Element) {
    try {
        [pscustomobject]@{
            Element = $Element
            Name = [string]$Element.Current.Name
            Type = [string]$Element.Current.ControlType.ProgrammaticName.Replace("ControlType.", "")
            Focusable = [bool]$Element.Current.IsKeyboardFocusable
            Focused = [bool]$Element.Current.HasKeyboardFocus
            Enabled = [bool]$Element.Current.IsEnabled
            IsPassword = [bool]$Element.Current.IsPassword
        }
    } catch [System.Windows.Automation.ElementNotAvailableException] {
        $null
    }
}

function Get-Snapshots([System.Windows.Automation.AutomationElement]$Root) {
    @(Get-Elements $Root | ForEach-Object { Get-ElementSnapshot $_ } | Where-Object { $null -ne $_ })
}

function Find-Element(
    [System.Windows.Automation.AutomationElement]$Root,
    [string[]]$Names,
    [string[]]$Types,
    [DateTime]$Deadline
) {
    do {
        foreach ($item in Get-Snapshots $Root) {
            if ($item.Name -in $Names -and ($Types.Count -eq 0 -or $item.Type -in $Types)) {
                return $item
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw "UI Automation did not expose $($Names -join ' or ') as $($Types -join ' or ')."
}

function Wait-Focused(
    [System.Windows.Automation.AutomationElement]$Root,
    [string]$Name,
    [DateTime]$Deadline
) {
    do {
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -ne $focused -and $focused.Current.Name -eq $Name) { return }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw "Native keyboard focus did not reach '$Name'."
}

function Invoke-Accessible([System.Windows.Automation.AutomationElement]$Element) {
    $pattern = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
        return
    }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.TogglePattern]$pattern).Toggle()
        return
    }
    throw "UI Automation exposed no invokable pattern for '$($Element.Current.Name)'."
}

$process = Get-Process -Id $ProcessId -ErrorAction Stop
$deadline = Get-Deadline
do {
    $process.Refresh()
    if ($process.HasExited) { throw "MCG exited before native accessibility qualification." }
    if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)
if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw "MCG did not expose a native window." }

[void][McgNativeWindow]::SetForegroundWindow($process.MainWindowHandle)
$root = [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
if ($root.Current.Name -ne "MCG") { throw "The native window has no stable MCG accessible name." }

# The first UIA query activates Chromium's accessibility provider. Poll the
# resulting native tree so renderer startup cannot turn this into a timing-only
# failure.
$deadline = Get-Deadline
do {
    $snapshots = Get-Snapshots $root
    if (($snapshots | Where-Object Type -eq "Document") `
        -and ($snapshots | Where-Object Name -eq "Primary navigation") `
        -and ($snapshots | Where-Object Name -eq "Settings")) {
        break
    }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)
if (-not ($snapshots | Where-Object Type -eq "Document")) { throw "UI Automation did not expose the web document." }
if (-not ($snapshots | Where-Object Name -eq "Primary navigation")) { throw "UI Automation did not expose Primary navigation." }
if (-not ($snapshots | Where-Object Name -eq "Settings")) { throw "UI Automation did not expose Settings." }

$deadline = Get-Deadline
$required = @{
    Goal = Find-Element $root @("Goal") @("Edit") $deadline
    OpenFolder = Find-Element $root @("Open folder") @("Button") $deadline
    CreateTask = Find-Element $root @("Create task") @("Button") $deadline
    OpenAi = Find-Element $root @("Continue with OpenAI", "Connect with OpenAI") @("Button") $deadline
    AddProvider = Find-Element $root @("Add API or local provider") @("Button") $deadline
}
foreach ($name in $required.Keys) {
    if (-not $required[$name].Focusable) { throw "Required control '$name' is not keyboard-focusable." }
}

$interactiveTypes = @("Button", "CheckBox", "ComboBox", "Edit", "Hyperlink", "RadioButton", "Slider", "Spinner")
$unnamedFocusable = @($snapshots | Where-Object {
    $_.Focusable -and $_.Type -in $interactiveTypes -and [string]::IsNullOrWhiteSpace($_.Name)
} | Select-Object -ExpandProperty Type -Unique)
if ($unnamedFocusable.Count -ne 0) {
    throw "UI Automation exposed unnamed focusable controls: $($unnamedFocusable -join ', ')."
}

$required.Goal.Element.SetFocus()
Wait-Focused $root "Goal" $deadline
[Windows.Forms.SendKeys]::SendWait("{TAB}")
Wait-Focused $root "Open folder" $deadline
[Windows.Forms.SendKeys]::SendWait("{TAB}")
Wait-Focused $root "Create task" $deadline

$mode = Find-Element $root @("Advanced mode") @("Button") $deadline
Invoke-Accessible $mode.Element
$null = Find-Element $root @("Simple mode") @("Button") $deadline
$addProvider = Find-Element $root @("Add API or local provider") @("Button") $deadline
Invoke-Accessible $addProvider.Element
$password = Find-Element $root @("API key") @("Edit") $deadline
if (-not $password.Focusable -or -not $password.IsPassword) {
    throw "The API key control is not a focusable native password element."
}
$password.Element.SetFocus()
[Windows.Forms.SendKeys]::SendWait($Canary)
Start-Sleep -Milliseconds 100

$valueWasBlocked = $false
$value = $null
try {
    $raw = $password.Element.GetCurrentPropertyValue(
        [System.Windows.Automation.ValuePattern]::ValueProperty,
        $true
    )
    if ($raw -ne [System.Windows.Automation.AutomationElement]::NotSupported) {
        $value = [string]$raw
    }
} catch [System.UnauthorizedAccessException] {
    $valueWasBlocked = $true
}
if ($value -eq $Canary) { throw "UI Automation exposed the API-key cleartext." }
if (Get-Snapshots $root | Where-Object { $_.Name -like "*$Canary*" }) {
    throw "The API-key cleartext leaked into an accessible name."
}

$report = [ordered]@{
    schema_version = 1
    platform = "windows"
    backend = "uia"
    application_exposed = $true
    document_exposed = $true
    primary_navigation_exposed = $true
    required_controls_named = $true
    required_controls_focusable = $true
    unnamed_focusable_control_types = @()
    keyboard_focus_sequence = @("Goal", "Open folder", "Create task")
    simple_to_advanced_native_input = $true
    credential_control_is_password = $true
    credential_accessible_value_redacted = ($valueWasBlocked -or $value -ne $Canary)
    node_count = (Get-Snapshots $root).Count
}
$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$parent = [IO.Path]::GetDirectoryName($resolvedOutput)
if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    [void][IO.Directory]::CreateDirectory($parent)
}
[IO.File]::WriteAllText(
    $resolvedOutput,
    ($report | ConvertTo-Json -Compress),
    [Text.UTF8Encoding]::new($false)
)
Write-Output "Windows native accessibility qualification passed: $resolvedOutput"
