param(
    [string]$ReportPath = 'phase4-results\switch-crash.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$startedAt = [DateTime]::UtcNow
cargo test --manifest-path 'src-tauri\Cargo.toml' `
    switch_process_crash_between_pointers_preserves_old_active_and_recovers `
    -- --nocapture --test-threads=1
$testExitCode = $LASTEXITCODE
if ($testExitCode -ne 0) {
    throw "Switch crash test failed with exit code $testExitCode."
}

$finishedAt = [DateTime]::UtcNow
$fullReportPath = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot $ReportPath))
$reportParent = Split-Path -Parent $fullReportPath
New-Item -ItemType Directory -Force -Path $reportParent | Out-Null
$report = [ordered]@{
    schemaVersion = 1
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = $finishedAt.ToString('o')
    durationSeconds = [math]::Round(($finishedAt - $startedAt).TotalSeconds, 3)
    testExitCode = $testExitCode
    helperExitMode = 'process::exit(88) after previous.json commit and before active.json commit'
    oldActivePointerPreserved = $true
    subsequentSwitchSucceeded = $true
    subsequentRollbackSucceeded = $true
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Switch crash report: $fullReportPath"
