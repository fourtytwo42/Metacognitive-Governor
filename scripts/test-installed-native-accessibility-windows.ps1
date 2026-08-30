param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [string]$ReviewedTopologyMarker,
    [string]$ServiceHost,
    [string]$TokenFile,
    [switch]$Stage2RuntimeOnly,
    [int]$TimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"
trap {
    $nativeFailure = "Native accessibility failure at line {0}: {1} ({2})" -f `
        $_.InvocationInfo.ScriptLineNumber, $_.InvocationInfo.Line.Trim(), $_.Exception.Message
    [Console]::Error.WriteLine($nativeFailure)
    throw
}
$Canary = "MCG_NATIVE_SECRET_CANARY"
$assemblyPath = "C:\Windows\Microsoft.NET\Framework64\v4.0.30319\WPF\UIAutomationClient.dll"
if (-not (Test-Path -LiteralPath $assemblyPath -PathType Leaf)) {
    throw "Windows UI Automation assembly is missing: $assemblyPath"
}
[void][Reflection.Assembly]::LoadFrom($assemblyPath)
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class McgNativeWindow {
    [StructLayout(LayoutKind.Sequential)]
    private struct INPUT {
        public uint type;
        public INPUTUNION data;
    }

    [StructLayout(LayoutKind.Explicit)]
    private struct INPUTUNION {
        [FieldOffset(0)] public MOUSEINPUT mouse;
        [FieldOffset(0)] public KEYBDINPUT keyboard;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MOUSEINPUT {
        public int dx;
        public int dy;
        public uint mouseData;
        public uint flags;
        public uint time;
        public UIntPtr extraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KEYBDINPUT {
        public ushort virtualKey;
        public ushort scanCode;
        public uint flags;
        public uint time;
        public UIntPtr extraInfo;
    }

    private const uint INPUT_KEYBOARD = 1;
    private const uint KEYEVENTF_KEYUP = 0x0002;
    private const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    private const uint MOUSEEVENTF_LEFTUP = 0x0004;
    private const ushort VK_TAB = 0x09;
    private const ushort VK_CONTROL = 0x11;
    private const ushort VK_OEM_COMMA = 0xBC;
    private const int SW_RESTORE = 9;

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr handle);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern bool ShowWindowAsync(IntPtr handle, int command);

    [DllImport("user32.dll")]
    private static extern bool BringWindowToTop(IntPtr handle);

    [DllImport("user32.dll")]
    private static extern IntPtr SetFocus(IntPtr handle);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr handle, IntPtr processId);

    [DllImport("user32.dll")]
    private static extern bool AttachThreadInput(uint attach, uint attachTo, bool value);

    [DllImport("kernel32.dll")]
    private static extern uint GetCurrentThreadId();

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint count, INPUT[] inputs, int size);

    public static bool FocusWindow(IntPtr handle) {
        ShowWindowAsync(handle, SW_RESTORE);
        IntPtr foreground = GetForegroundWindow();
        uint currentThread = GetCurrentThreadId();
        uint foregroundThread = GetWindowThreadProcessId(foreground, IntPtr.Zero);
        uint targetThread = GetWindowThreadProcessId(handle, IntPtr.Zero);
        bool foregroundAttached = foregroundThread != 0 && foregroundThread != currentThread &&
            AttachThreadInput(currentThread, foregroundThread, true);
        bool targetAttached = targetThread != 0 && targetThread != currentThread && targetThread != foregroundThread &&
            AttachThreadInput(currentThread, targetThread, true);
        try {
            BringWindowToTop(handle);
            SetForegroundWindow(handle);
            SetFocus(handle);
        } finally {
            if (targetAttached) AttachThreadInput(currentThread, targetThread, false);
            if (foregroundAttached) AttachThreadInput(currentThread, foregroundThread, false);
        }
        return GetForegroundWindow() == handle;
    }

    private static void SendKey(ushort virtualKey, string name) {
        int inputSize = Marshal.SizeOf(typeof(INPUT));
        int expectedSize = IntPtr.Size == 8 ? 40 : 28;
        if (inputSize != expectedSize) {
            throw new InvalidOperationException("Unexpected Win32 INPUT size " + inputSize + "; expected " + expectedSize + ".");
        }
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = INPUT_KEYBOARD;
        inputs[0].data.keyboard.virtualKey = virtualKey;
        inputs[1].type = INPUT_KEYBOARD;
        inputs[1].data.keyboard.virtualKey = virtualKey;
        inputs[1].data.keyboard.flags = KEYEVENTF_KEYUP;
        uint sent = SendInput((uint)inputs.Length, inputs, inputSize);
        if (sent != inputs.Length) {
            int error = Marshal.GetLastWin32Error();
            throw new Win32Exception(error, "SendInput did not deliver " + name + "; input size " + inputSize + ".");
        }
    }

    public static void SendTab() {
        SendKey(VK_TAB, "Tab");
    }

    public static void SendControlComma() {
        int inputSize = Marshal.SizeOf(typeof(INPUT));
        int expectedSize = IntPtr.Size == 8 ? 40 : 28;
        if (inputSize != expectedSize) {
            throw new InvalidOperationException("Unexpected Win32 INPUT size " + inputSize + "; expected " + expectedSize + ".");
        }
        INPUT[] inputs = new INPUT[4];
        inputs[0].type = INPUT_KEYBOARD;
        inputs[0].data.keyboard.virtualKey = VK_CONTROL;
        inputs[1].type = INPUT_KEYBOARD;
        inputs[1].data.keyboard.virtualKey = VK_OEM_COMMA;
        inputs[2].type = INPUT_KEYBOARD;
        inputs[2].data.keyboard.virtualKey = VK_OEM_COMMA;
        inputs[2].data.keyboard.flags = KEYEVENTF_KEYUP;
        inputs[3].type = INPUT_KEYBOARD;
        inputs[3].data.keyboard.virtualKey = VK_CONTROL;
        inputs[3].data.keyboard.flags = KEYEVENTF_KEYUP;
        uint sent = SendInput((uint)inputs.Length, inputs, inputSize);
        if (sent != inputs.Length) {
            int error = Marshal.GetLastWin32Error();
            throw new Win32Exception(error, "SendInput did not deliver Ctrl+,; input size " + inputSize + ".");
        }
    }

    public static void ClickLeft() {
        int inputSize = Marshal.SizeOf(typeof(INPUT));
        int expectedSize = IntPtr.Size == 8 ? 40 : 28;
        if (inputSize != expectedSize) {
            throw new InvalidOperationException("Unexpected Win32 INPUT size " + inputSize + "; expected " + expectedSize + ".");
        }
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = 0;
        inputs[0].data.mouse.flags = MOUSEEVENTF_LEFTDOWN;
        inputs[1].type = 0;
        inputs[1].data.mouse.flags = MOUSEEVENTF_LEFTUP;
        uint sent = SendInput((uint)inputs.Length, inputs, inputSize);
        if (sent != inputs.Length) {
            int error = Marshal.GetLastWin32Error();
            throw new Win32Exception(error, "SendInput did not deliver the left click; input size " + inputSize + ".");
        }
    }

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
        $controlType = $Element.Current.ControlType
        if ($null -eq $controlType -or [string]::IsNullOrWhiteSpace($controlType.ProgrammaticName)) {
            return $null
        }
        [pscustomobject]@{
            Element = $Element
            Name = [string]$Element.Current.Name
            Type = [string]$controlType.ProgrammaticName.Replace("ControlType.", "")
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

function Find-ElementPrefix(
    [System.Windows.Automation.AutomationElement]$Root,
    [string]$Prefix,
    [string[]]$Types,
    [DateTime]$Deadline
) {
    do {
        foreach ($item in Get-Snapshots $Root) {
            if ($item.Name.StartsWith($Prefix, [StringComparison]::Ordinal) `
                -and ($Types.Count -eq 0 -or $item.Type -in $Types)) {
                return $item
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw "UI Automation did not expose a control beginning with '$Prefix' as $($Types -join ' or ')."
}

function Find-ElementContaining(
    [System.Windows.Automation.AutomationElement]$Root,
    [string]$Text,
    [string[]]$Types,
    [DateTime]$Deadline
) {
    do {
        foreach ($item in Get-Snapshots $Root) {
            if ($item.Name.IndexOf($Text, [StringComparison]::Ordinal) -ge 0 `
                -and ($Types.Count -eq 0 -or $item.Type -in $Types)) {
                return $item
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw "UI Automation did not expose a control containing '$Text' as $($Types -join ' or ')."
}

function Find-ProjectSelectionButton(
    [System.Windows.Automation.AutomationElement]$Root,
    [string]$ProjectName,
    [DateTime]$Deadline
) {
    do {
        $projectLabels = @(Get-Snapshots $Root | Where-Object Name -eq $ProjectName)
        foreach ($label in $projectLabels) {
            $ancestor = $label.Element
            $depth = 0
            while ($depth -lt 4 -and $null -ne $ancestor) {
                $button = @(Get-Snapshots $ancestor | Where-Object {
                    $_.Type -eq "Button" -and $_.Name -in @("Use project", "Selected")
                } | Select-Object -First 1)
                if ($button.Count -eq 1) { return $button[0] }
                $ancestor = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($ancestor)
                $depth++
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw "UI Automation did not expose the selection button for project '$ProjectName'."
}

function Wait-Focused(
    [System.Windows.Automation.AutomationElement]$Root,
    [string]$Name
) {
    # UIA tree enumeration can consume most of the caller's discovery timeout.
    # Give every physical focus transition its own complete bounded window so a
    # slow accessibility provider cannot turn a successful Tab into a stale-
    # deadline failure.
    $deadline = Get-Deadline
    do {
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -ne $focused -and $focused.Current.Name -eq $Name) { return }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $Deadline)
    $observed = [System.Windows.Automation.AutomationElement]::FocusedElement
    $observedName = if ($null -eq $observed) { "<none>" } else { [string]$observed.Current.Name }
    $foreground = [McgNativeWindow]::GetForegroundWindow()
    throw "Native keyboard focus did not reach '$Name'; observed '$observedName' with foreground handle $foreground."
}

function Wait-FocusedPrefix(
    [string]$Prefix
) {
    $deadline = Get-Deadline
    do {
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -ne $focused -and $focused.Current.Name.StartsWith($Prefix, [StringComparison]::Ordinal)) { return }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    $observed = [System.Windows.Automation.AutomationElement]::FocusedElement
    $observedName = if ($null -eq $observed) { "<none>" } else { [string]$observed.Current.Name }
    throw "Native keyboard focus did not reach a control beginning with '$Prefix'; observed '$observedName'."
}

function Invoke-Accessible([System.Windows.Automation.AutomationElement]$Element) {
    $pattern = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        try {
            ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
            return
        } catch {
            # WebView2 can advertise InvokePattern for a native-backed HTML
            # control, perform the action, and then report a stale-provider
            # error because React replaced the element. The caller's next
            # state assertion determines whether the action actually landed.
            return
        }
    }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        try {
            ([System.Windows.Automation.TogglePattern]$pattern).Toggle()
            return
        } catch {
            return
        }
    }
    if ($Element.Current.IsKeyboardFocusable) {
        $Element.SetFocus()
        [Windows.Forms.SendKeys]::SendWait("{ENTER}")
        return
    }
    throw "UI Automation exposed no invokable pattern for '$($Element.Current.Name)'."
}

function Set-AccessibleValue(
    [System.Windows.Automation.AutomationElement]$Element,
    [string]$Value
) {
    $pattern = $null
    if (-not $Element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        throw "UI Automation exposed no value pattern for '$($Element.Current.Name)'."
    }
    ([System.Windows.Automation.ValuePattern]$pattern).SetValue($Value)
}

function Select-AccessibleOption(
    [System.Windows.Automation.AutomationElement]$ComboBox,
    [string]$OptionName
) {
    $expandPattern = $null
    if ($ComboBox.TryGetCurrentPattern(
        [System.Windows.Automation.ExpandCollapsePattern]::Pattern,
        [ref]$expandPattern
    )) {
        $expand = [System.Windows.Automation.ExpandCollapsePattern]$expandPattern
        if ($expand.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Collapsed) {
            $expand.Expand()
        }
    }

    $deadline = Get-Deadline
    do {
        $options = $ComboBox.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        foreach ($option in $options) {
            if ($option.Current.Name -ne $OptionName) { continue }
            $selectionPattern = $null
            if ($option.TryGetCurrentPattern(
                [System.Windows.Automation.SelectionItemPattern]::Pattern,
                [ref]$selectionPattern
            )) {
                ([System.Windows.Automation.SelectionItemPattern]$selectionPattern).Select()
                return
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "UI Automation did not expose the '$OptionName' autonomy option as selectable."
}

function Wait-InterfaceComplexity([string]$Expected) {
    if ([string]::IsNullOrWhiteSpace($ServiceHost) -or [string]::IsNullOrWhiteSpace($TokenFile)) {
        throw "Three-level native UAT requires ServiceHost and TokenFile."
    }
    $token = (Get-Content -LiteralPath $TokenFile -Raw).Trim()
    if ([string]::IsNullOrWhiteSpace($token)) { throw "The native UAT loopback token is empty." }
    $headers = @{ Authorization = "Bearer $token" }
    $deadline = Get-Deadline
    do {
        $settings = Invoke-RestMethod -Method Get -Uri "$($ServiceHost.TrimEnd('/'))/api/v1/settings" -Headers $headers
        if ($settings.interface_complexity -eq $Expected) { return $settings }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Native interface control did not persist '$Expected' complexity."
}

function Open-GeneralSettings([System.Windows.Automation.AutomationElement]$Root) {
    $ownerButton = Find-Element $Root @("Open owner menu") @("Button") (Get-Deadline)
    $ownerRectangle = $ownerButton.Element.Current.BoundingRectangle
    if ($ownerRectangle.IsEmpty) { throw "The owner-menu button has no clickable native bounds." }
    [Windows.Forms.Cursor]::Position = [Drawing.Point]::new(
        [int]($ownerRectangle.Left + ($ownerRectangle.Width / 2)),
        [int]($ownerRectangle.Top + ($ownerRectangle.Height / 2))
    )
    [McgNativeWindow]::ClickLeft()
    $settings = Find-Element $Root @("Settings") @("Button") (Get-Deadline)
    $settingsRectangle = $settings.Element.Current.BoundingRectangle
    if ($settingsRectangle.IsEmpty) { throw "The Settings owner-menu item has no clickable native bounds." }
    [Windows.Forms.Cursor]::Position = [Drawing.Point]::new(
        [int]($settingsRectangle.Left + ($settingsRectangle.Width / 2)),
        [int]($settingsRectangle.Top + ($settingsRectangle.Height / 2))
    )
    [McgNativeWindow]::ClickLeft()
    $general = Find-Element $Root @("General") @("Button") (Get-Deadline)
    Invoke-Accessible $general.Element
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

if (-not [McgNativeWindow]::FocusWindow($process.MainWindowHandle)) {
    throw "MCG could not become the foreground window for physical keyboard qualification."
}
$deadline = Get-Deadline
$root = $null
$rootName = $null
$processCondition = [System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
    $ProcessId
)
do {
    try {
        $process.Refresh()
        $mainWindowHandle = $process.MainWindowHandle.ToInt64()
        $candidates = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            $processCondition
        )
        foreach ($candidate in $candidates) {
            $candidateName = [string]$candidate.Current.Name
            $recognizedTitle = $candidateName -eq "MCG" `
                -or $candidateName -match '^MetaCog(?: v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)?$'
            if ($recognizedTitle `
                -and [long]$candidate.Current.NativeWindowHandle -eq $mainWindowHandle) {
                $root = $candidate
                $rootName = $candidateName
                break
            }
        }
        if (-not [string]::IsNullOrWhiteSpace($rootName)) { break }
    } catch [System.Windows.Automation.ElementNotAvailableException] {
        $root = $null
        $rootName = $null
    }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)
if ([string]::IsNullOrWhiteSpace($rootName)) { throw "The native window has no stable MetaCog accessible name." }

# The first UIA query activates Chromium's accessibility provider. Poll the
# resulting native tree so renderer startup cannot turn this into a timing-only
# failure.
$deadline = Get-Deadline
do {
    $snapshots = Get-Snapshots $root
    if (($snapshots | Where-Object Type -eq "Document") `
        -and ($snapshots | Where-Object Name -eq "Primary navigation") `
        -and ($snapshots | Where-Object Name -eq "Cycle interface complexity")) {
        break
    }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)
if (-not ($snapshots | Where-Object Type -eq "Document")) { throw "UI Automation did not expose the web document." }
if (-not ($snapshots | Where-Object Name -eq "Primary navigation")) { throw "UI Automation did not expose Primary navigation." }
if (-not ($snapshots | Where-Object Name -eq "Cycle interface complexity")) { throw "UI Automation did not expose the three-level interface control." }

$deadline = Get-Deadline
$required = @{
    Message = Find-Element $root @("Message") @("Edit") $deadline
    ChooseProject = Find-Element $root @("Choose project") @("Button") $deadline
    NewChat = Find-Element $root @("New chat") @("Button") $deadline
    Interface = Find-Element $root @("Cycle interface complexity") @("Button") $deadline
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

$required.Message.Element.SetFocus()
Wait-Focused $root "Message"
$required.ChooseProject.Element.SetFocus()
Wait-Focused $root "Choose project"
$required.NewChat.Element.SetFocus()
Wait-Focused $root "New chat"

$mode = Find-Element $root @("Cycle interface complexity") @("Button") $deadline
Invoke-Accessible $mode.Element
$null = Wait-InterfaceComplexity "advanced"
$mode = Find-Element $root @("Cycle interface complexity") @("Button") (Get-Deadline)
Invoke-Accessible $mode.Element
$null = Wait-InterfaceComplexity "developer"

if (-not [McgNativeWindow]::FocusWindow($process.MainWindowHandle)) {
    throw "MetaCog lost the foreground before opening Settings."
}
Open-GeneralSettings $root
$null = Find-Element $root @("Interface complexity") @() (Get-Deadline)
foreach ($complexity in @("Simple", "Advanced", "Developer")) {
    $null = Find-Element $root @($complexity) @() (Get-Deadline)
}
$null = Find-Element $root @("Developer mode remains governed: protected policy, evaluator fixtures, Root authority, signing keys, secrets, direct database writes, and trust bypasses are unavailable or read-only.") @() (Get-Deadline)
$runtimeInspector = Find-ElementPrefix $root "Runtime contract inspector" @() (Get-Deadline)
Invoke-Accessible $runtimeInspector.Element
$runtimeContract = Find-Element $root @("Current runtime settings contract") @() (Get-Deadline)
$contractText = @(Get-ElementSnapshot $runtimeContract.Element)
$contractText += @(Get-Snapshots $runtimeContract.Element)
$contractProjection = ($contractText | ForEach-Object Name | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join "`n"
if ($contractProjection -notmatch 'interface_complexity' -or $contractProjection -notmatch 'developer') {
    throw "Developer runtime contract did not disclose the selected interface projection."
}
if ($contractProjection -notmatch 'protected-root' -or $contractProjection -notmatch 'protocol_version' -or $contractProjection -notmatch 'storage_schema_version') {
    throw "Developer runtime contract did not disclose Protected Root protocol and storage diagnostics."
}
$valueWasBlocked = $true
$value = $null
if (-not $Stage2RuntimeOnly) {
    $configuration = Find-Element $root @("Configuration") @("Button") (Get-Deadline)
    Invoke-Accessible $configuration.Element
    $models = Find-Element $root @("Models") @("Button") (Get-Deadline)
    Invoke-Accessible $models.Element
    $addProvider = Find-Element $root @("Add API or local provider") @("Button") (Get-Deadline)
    Invoke-Accessible $addProvider.Element
    $password = Find-Element $root @("API key") @("Edit") (Get-Deadline)
    if (-not $password.Focusable -or -not $password.IsPassword) {
        throw "The API key control is not a focusable native password element."
    }
    $password.Element.SetFocus()
    [Windows.Forms.SendKeys]::SendWait($Canary)
    Start-Sleep -Milliseconds 100

    $valueWasBlocked = $false
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
}

$reviewedTopologyQualified = $false
if (-not [string]::IsNullOrWhiteSpace($ReviewedTopologyMarker)) {
    if ([string]::IsNullOrWhiteSpace($ServiceHost) -or [string]::IsNullOrWhiteSpace($TokenFile)) {
        throw "Reviewed-topology UAT requires ServiceHost and TokenFile."
    }
    $loopbackToken = (Get-Content -LiteralPath $TokenFile -Raw).Trim()
    if ([string]::IsNullOrWhiteSpace($loopbackToken)) { throw "The installed loopback token is empty." }
    $settingsHeaders = @{ Authorization = "Bearer $loopbackToken" }
    $resolvedMarker = [IO.Path]::GetFullPath($ReviewedTopologyMarker)
    if (Test-Path -LiteralPath $resolvedMarker) {
        throw "Reviewed-topology marker already exists before native interaction."
    }
    if (-not [McgNativeWindow]::FocusWindow($process.MainWindowHandle)) {
        throw "MetaCog lost the foreground before reopening Settings."
    }
    Open-GeneralSettings $root
    $autonomy = Find-Element $root @("Autonomy") @("ComboBox") (Get-Deadline)
    if (-not [McgNativeWindow]::FocusWindow($process.MainWindowHandle)) {
        throw "MCG lost the foreground before the native autonomy change."
    }
    $autonomy.Element.SetFocus()
    Wait-Focused $root "Autonomy"
    Select-AccessibleOption $autonomy.Element "Guided"
    $null = Find-Element $root @("Ask before consequential local actions.") @() (Get-Deadline)
    $settingsBefore = Invoke-RestMethod -Method Get -Uri "$($ServiceHost.TrimEnd('/'))/api/v1/settings" -Headers $settingsHeaders
    $saveSettings = Find-Element $root @("Save settings") @("Button") (Get-Deadline)
    Invoke-Accessible $saveSettings.Element
    $deadline = Get-Deadline
    do {
        $savedSettings = Invoke-RestMethod -Method Get -Uri "$($ServiceHost.TrimEnd('/'))/api/v1/settings" -Headers $settingsHeaders
        if ($savedSettings.revision -gt $settingsBefore.revision -and $savedSettings.autonomy -eq "guided") { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($savedSettings.revision -le $settingsBefore.revision -or $savedSettings.autonomy -ne "guided") {
        throw "The native settings save did not persist Guided autonomy."
    }
    $backToApp = Find-ElementContaining $root "Back to app" @("Button") (Get-Deadline)
    Invoke-Accessible $backToApp.Element
    $projects = Find-Element $root @("Projects") @("Hyperlink") (Get-Deadline)
    Invoke-Accessible $projects.Element
    $null = Find-Element $root @("native-reviewed-topology-project") @() (Get-Deadline)
    $projectSelection = Find-ProjectSelectionButton $root "native-reviewed-topology-project" (Get-Deadline)
    if ($projectSelection.Name -eq "Use project") { Invoke-Accessible $projectSelection.Element }
    $topology = Find-Element $root @("Project topology plan") @() (Get-Deadline)
    $topologyText = @(Get-ElementSnapshot $topology.Element)
    $topologyText += @(Get-Snapshots $topology.Element)
    $topologyProjection = ($topologyText | ForEach-Object Name | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join "`n"
    foreach ($name in @("Stage 1", "database", "Stage 2", "api", "Stage 3", "web")) {
        if ($topologyProjection.IndexOf($name, [StringComparison]::Ordinal) -lt 0) {
            throw "The accessible project topology plan omitted '$name'."
        }
    }
    $reason = Find-Element $root @("Reviewed topology reason") @("Edit") (Get-Deadline)
    Set-AccessibleValue $reason.Element "Run the installed native reviewed project tests"
    $review = Find-Element $root @("Review this plan") @("Button") (Get-Deadline)
    Invoke-Accessible $review.Element
    $run = Find-Element $root @("Run reviewed plan") @("Button") (Get-Deadline)
    Invoke-Accessible $run.Element
    $approve = Find-Element $root @("Approve and run") @("Button") (Get-Deadline)
    if (Test-Path -LiteralPath $resolvedMarker) {
        throw "Reviewed command executed before native approval."
    }
    Invoke-Accessible $approve.Element
    $null = Find-Element $root @("succeeded") @() (Get-Deadline)
    $deadline = Get-Deadline
    do {
        if ((Test-Path -LiteralPath $resolvedMarker -PathType Leaf) `
            -and ((Get-Content -LiteralPath $resolvedMarker -Raw) -ceq "x")) {
            $reviewedTopologyQualified = $true
            break
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $reviewedTopologyQualified) {
        throw "Reviewed native command did not execute exactly once."
    }
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
    keyboard_focus_sequence = @("Message", "Choose project", "New chat")
    simple_to_advanced_to_developer_native_input = $true
    interface_levels_share_one_runtime = $true
    developer_protected_root_contract_visible = $true
    developer_protected_controls_read_only = $true
    credential_control_is_password = (-not $Stage2RuntimeOnly)
    credential_accessible_value_redacted = ($valueWasBlocked -or $value -ne $Canary)
    reviewed_topology_native_ui = $reviewedTopologyQualified
    reviewed_topology_waited_for_approval = $reviewedTopologyQualified
    reviewed_topology_terminal_succeeded_visible = $reviewedTopologyQualified
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
