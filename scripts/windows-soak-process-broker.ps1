param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [Parameter(Mandatory = $true)]
    [string]$PidFile,

    [Parameter(Mandatory = $true)]
    [string]$StopFile,

    [Parameter(Mandatory = $true)]
    [string]$RssFile,

    [Parameter(Mandatory = $true)]
    [int]$ParentPid
)

$ErrorActionPreference = 'Stop'
$resolvedExecutable = (Resolve-Path -LiteralPath $FilePath).Path
$pidFilePath = [System.IO.Path]::GetFullPath($PidFile)
$stopFilePath = [System.IO.Path]::GetFullPath($StopFile)
$rssFilePath = [System.IO.Path]::GetFullPath($RssFile)
$pidDirectory = [System.IO.Path]::GetDirectoryName($pidFilePath)
$stopDirectory = [System.IO.Path]::GetDirectoryName($stopFilePath)
$rssDirectory = [System.IO.Path]::GetDirectoryName($rssFilePath)

if (-not [string]::Equals($pidDirectory, $stopDirectory, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not [string]::Equals($pidDirectory, $rssDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Broker control files must share one directory'
}
if (-not (Test-Path -LiteralPath $pidDirectory -PathType Container)) {
    throw 'Broker control directory does not exist'
}

$daemon = Start-Process -FilePath $resolvedExecutable -PassThru -WindowStyle Hidden
[System.IO.File]::WriteAllText($pidFilePath, $daemon.Id.ToString([System.Globalization.CultureInfo]::InvariantCulture))
$daemon.Refresh()
[System.IO.File]::WriteAllText($rssFilePath, $daemon.WorkingSet64.ToString([System.Globalization.CultureInfo]::InvariantCulture))

try {
    while (-not $daemon.HasExited) {
        $parentAlive = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
        if (-not $parentAlive -or (Test-Path -LiteralPath $stopFilePath -PathType Leaf)) {
            Stop-Process -Id $daemon.Id -Force -ErrorAction Stop
            $daemon.WaitForExit()
            break
        }
        Start-Sleep -Milliseconds 50
        $daemon.Refresh()
        [System.IO.File]::WriteAllText($rssFilePath, $daemon.WorkingSet64.ToString([System.Globalization.CultureInfo]::InvariantCulture))
    }
} finally {
    $daemon.Refresh()
    if (-not $daemon.HasExited) {
        Stop-Process -Id $daemon.Id -Force -ErrorAction SilentlyContinue
        $daemon.WaitForExit()
    }
}

exit $daemon.ExitCode
