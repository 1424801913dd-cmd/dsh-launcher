param(
    [string]$ReportPath = 'phase4-results\backup-crash.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$startedAt = [DateTime]::UtcNow
cargo test --manifest-path 'src-tauri\Cargo.toml' `
    backup_process_crash_preserves_active_runtime_and_staging_is_recoverable `
    -- --nocapture --test-threads=1
$testExitCode = $LASTEXITCODE
if ($testExitCode -ne 0) {
    throw "Backup crash test failed with exit code $testExitCode."
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
    helperExitMode = 'process::exit(87) after backup copy and before staging rename'
    activeRuntimePointerPreserved = $true
    interruptedStagingDetected = $true
    interruptedStagingRecovered = $true
    reparsePointsFollowed = $false
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Backup crash report: $fullReportPath"
