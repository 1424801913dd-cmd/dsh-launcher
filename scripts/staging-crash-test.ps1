param(
    [string]$ReportPath = 'phase4-results\staging-crash.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$startedAt = [DateTime]::UtcNow
cargo test --manifest-path 'src-tauri\Cargo.toml' `
    signed_runtime_staging_process_crash_preserves_active_and_recovers `
    -- --nocapture --test-threads=1
$testExitCode = $LASTEXITCODE
if ($testExitCode -ne 0) {
    throw "Signed Runtime staging crash test failed with exit code $testExitCode."
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
    helperExitMode = 'process::exit(89) after runtime.json sync and before staging directory rename'
    oldActivePointerPreserved = $true
    uncommittedVersionNotExposed = $true
    staleStagingRecovered = $true
    reparsePointsFollowed = $false
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Signed Runtime staging crash report: $fullReportPath"
