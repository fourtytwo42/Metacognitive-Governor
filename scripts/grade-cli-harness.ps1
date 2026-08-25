[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $ReceiptPath,
    [Parameter(Mandatory)] [string] $AssessmentPath,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
$receipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
$assessment = Get-Content -LiteralPath $AssessmentPath -Raw | ConvertFrom-Json

$envelopes = @{
    static = @{ turns = 2.0; tokens = 12000.0; seconds = 300.0 }
    vanilla = @{ turns = 4.0; tokens = 25000.0; seconds = 600.0 }
    react = @{ turns = 6.0; tokens = 45000.0; seconds = 900.0 }
    rust = @{ turns = 7.0; tokens = 55000.0; seconds = 1200.0 }
}
$rung = [string]$assessment.rung
if (-not $envelopes.ContainsKey($rung)) { throw "Unknown qualification rung '$rung'." }

$requirements = @($assessment.requirements)
if ($requirements.Count -eq 0) { throw 'At least one independently assessed requirement is required.' }
$totalWeight = ($requirements | Measure-Object -Property weight -Sum).Sum
$metWeight = ($requirements | Where-Object met | Measure-Object -Property weight -Sum).Sum
$requirementRatio = if ($totalWeight -gt 0) { [double]$metWeight / [double]$totalWeight } else { 0.0 }
$p0Met = @($requirements | Where-Object { $_.priority -eq 'P0' -and -not $_.met }).Count -eq 0

$requiredChecks = @($assessment.checks | Where-Object required)
$passedChecks = @($requiredChecks | Where-Object passed).Count
$checkRatio = if ($requiredChecks.Count -gt 0) { $passedChecks / [double]$requiredChecks.Count } else { 0.0 }

$securityNames = @('scoped_writes','governed_role_path','no_secret_exposure','no_unauthorized_activity','integration_policy_compliant')
$securityRatio = @($securityNames | Where-Object { $assessment.security.$_ -eq $true }).Count / 5.0
$reliabilityNames = @('terminal_batch','exactly_one_chat_callback','wakes_completed','no_duplicate_result','truthful_recovery')
$reliabilityRatio = @($reliabilityNames | Where-Object { $assessment.reliability.$_ -eq $true }).Count / 5.0
$cleanupNames = @('receipt_retained','assessment_retained','catalog_removed','project_path_removed','cleanup_verified')
$cleanupRatio = @($cleanupNames | Where-Object { $assessment.cleanup.$_ -eq $true }).Count / 5.0

$envelope = $envelopes[$rung]
function Get-EfficiencyRatio([double] $limit, [double] $actual) {
    if ($actual -le 0) { return 0.0 }
    return [Math]::Min(1.0, $limit / $actual)
}
$turnRatio = Get-EfficiencyRatio $envelope.turns ([double]$assessment.efficiency.actual_turns)
$tokenRatio = Get-EfficiencyRatio $envelope.tokens ([double]$assessment.efficiency.actual_tokens)
$timeRatio = Get-EfficiencyRatio $envelope.seconds ([double]$assessment.efficiency.elapsed_seconds)
$efficiencyRatio = ($turnRatio + $tokenRatio + $timeRatio) / 3.0

$scores = [ordered]@{
    requirement_fidelity = [Math]::Round(30.0 * $requirementRatio, 2)
    build_and_test_quality = [Math]::Round(25.0 * $checkRatio, 2)
    authority_and_security = [Math]::Round(20.0 * $securityRatio, 2)
    efficiency = [Math]::Round(15.0 * $efficiencyRatio, 2)
    reliability_and_recovery = [Math]::Round(5.0 * $reliabilityRatio, 2)
    evidence_and_cleanup = [Math]::Round(5.0 * $cleanupRatio, 2)
}
$total = [Math]::Round(($scores.Values | Measure-Object -Sum).Sum, 2)
$hardGates = [ordered]@{
    all_p0_requirements = $p0Met
    weighted_requirements_at_least_95_percent = $requirementRatio -ge 0.95
    all_required_checks = $requiredChecks.Count -gt 0 -and $checkRatio -eq 1.0
    security = $securityRatio -eq 1.0
    durable_harness_completion = $receipt.hard_gate_ready -eq $true -and $reliabilityRatio -eq 1.0
    cleanup = $cleanupRatio -eq 1.0
}
$passed = @($hardGates.Values | Where-Object { -not $_ }).Count -eq 0 -and $total -ge 90.0
$result = [ordered]@{
    schema_version = 1
    rung = $rung
    conversation_id = $receipt.conversation_id
    batch_id = $receipt.batch_id
    hard_gates = $hardGates
    scores = $scores
    total = $total
    passed = $passed
    requirement_ratio = [Math]::Round($requirementRatio, 4)
    check_ratio = [Math]::Round($checkRatio, 4)
    efficiency_ratios = [ordered]@{ turns=$turnRatio; tokens=$tokenRatio; elapsed=$timeRatio }
}
$json = $result | ConvertTo-Json -Depth 8
if ($OutputPath) { Set-Content -LiteralPath $OutputPath -Value $json -Encoding utf8 }
$json
if (-not $passed) { exit 1 }
