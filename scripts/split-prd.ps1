param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Stop'

$sourcePath = Join-Path $RepositoryRoot 'MCG_PRD.md'
$archiveDirectory = Join-Path $RepositoryRoot 'docs\archive'
$specDirectory = Join-Path $RepositoryRoot 'docs\spec'
$archivePath = Join-Path $archiveDirectory 'MCG_PRD.original.md'

New-Item -ItemType Directory -Force -Path $archiveDirectory, $specDirectory | Out-Null

if (Test-Path -LiteralPath $sourcePath) {
    $source = [IO.File]::ReadAllText($sourcePath)
    if (-not (Test-Path -LiteralPath $archivePath)) {
        [IO.File]::WriteAllText($archivePath, $source, [Text.UTF8Encoding]::new($false))
    }
} elseif (Test-Path -LiteralPath $archivePath) {
    $source = [IO.File]::ReadAllText($archivePath)
} else {
    throw 'Could not find MCG_PRD.md or its archived source.'
}

# The source was previously saved after UTF-8 bytes were decoded as Windows-1252.
# Reverse that transformation so punctuation and diagrams render correctly.
if ($source.Contains([char]0x00E2)) {
    $windows1252 = [Text.Encoding]::GetEncoding(1252)
    $source = [Text.Encoding]::UTF8.GetString($windows1252.GetBytes($source))
}

$matches = [regex]::Matches($source, '(?ms)^## (?<number>\d+)\. .*?(?=^## \d+\.|\z)')
$sections = @{}
foreach ($match in $matches) {
    $sections[[int]$match.Groups['number'].Value] = $match.Value.Trim()
}

if ($sections.Count -ne 60) {
    throw "Expected 60 numbered PRD sections but found $($sections.Count)."
}

$documents = [ordered]@{
    '01-product-vision.md' = @{
        Title = 'Product Vision and Success'; Sections = @(1, 2, 3, 58, 59, 60)
    }
    '02-system-architecture.md' = @{
        Title = 'System Architecture and Organization'; Sections = @(4, 5, 8, 9, 10, 11, 35, 36, 38, 39, 40)
    }
    '03-governor-evaluation.md' = @{
        Title = 'Governor, Evaluation, and Learning'; Sections = @(6, 12, 13, 14, 18, 19, 20, 21, 22, 23, 24, 25, 37)
    }
    '04-memory-project-intelligence.md' = @{
        Title = 'Memory and Project Intelligence'; Sections = @(15, 16)
    }
    '05-models-provider-mcp.md' = @{
        Title = 'Models, Provider Proxy, and MCP'; Sections = @(17, 31, 32, 33)
    }
    '06-tools-security-connectivity.md' = @{
        Title = 'Tools, Security, Plugins, and Connectivity'; Sections = @(7, 26, 27, 28, 29, 30, 34, 49, 50)
    }
    '07-product-surfaces.md' = @{
        Title = 'GUI, Projects, Threads, Scheduler, Sites, and CLI'; Sections = @(42, 43, 44, 45, 46, 47, 48, 53, 54)
    }
    '08-delivery-platform-roadmap.md' = @{
        Title = 'Delivery, Platform Strategy, and Roadmap'; Sections = @(41, 51, 52, 55, 56, 57)
    }
}

$generatedNotice = @'
> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.
'@

foreach ($entry in $documents.GetEnumerator()) {
    $body = ($entry.Value.Sections | ForEach-Object { $sections[$_] }) -join "`n`n---`n`n"
    $content = "# $($entry.Value.Title)`n`n$generatedNotice`n`n$body`n"
    $destination = Join-Path $specDirectory $entry.Key
    [IO.File]::WriteAllText($destination, $content, [Text.UTF8Encoding]::new($false))
}

$index = @'
# MCG Product Requirements

The MCG requirements are split by concern so product, architecture, security,
and delivery work can evolve independently while remaining navigable.

## Canonical specification

- [Product vision and success](docs/spec/01-product-vision.md)
- [System architecture and organization](docs/spec/02-system-architecture.md)
- [Governor, evaluation, and learning](docs/spec/03-governor-evaluation.md)
- [Memory and project intelligence](docs/spec/04-memory-project-intelligence.md)
- [Models, provider proxy, and MCP](docs/spec/05-models-provider-mcp.md)
- [Tools, security, plugins, and connectivity](docs/spec/06-tools-security-connectivity.md)
- [Product surfaces](docs/spec/07-product-surfaces.md)
- [Delivery, platform strategy, and roadmap](docs/spec/08-delivery-platform-roadmap.md)

## Supporting documents

- [Product and engineering baseline](docs/decisions/0001-product-engineering-baseline.md)
- [Original PRD archive](docs/archive/MCG_PRD.original.md)

The archived file preserves the initial source, including its original encoding
damage. The canonical split documents contain repaired UTF-8 punctuation and
diagrams.
'@

[IO.File]::WriteAllText($sourcePath, $index, [Text.UTF8Encoding]::new($false))

Write-Output "Generated $($documents.Count) specification documents from $($sections.Count) PRD sections."
