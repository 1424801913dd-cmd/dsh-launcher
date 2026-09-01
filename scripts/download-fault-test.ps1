param(
    [string]$ReportPath = 'phase4-results\download-faults.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$startedAt = [DateTime]::UtcNow
foreach ($testName in @(
    'socket_disconnect_preserves_verified_cache_and_removes_partial',
    'download_process_kill_preserves_verified_cache_and_partial_is_recoverable'
)) {
    cargo test --manifest-path 'src-tauri\Cargo.toml' `
        $testName `
        -- --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) {
        throw "Download fault test $testName failed with exit code $LASTEXITCODE."
    }
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
    socketDisconnect = [ordered]@{
        declaredBytes = 1024
        transmittedBytes = 128
        previousVerifiedCachePreserved = $true
        partialRemovedImmediately = $true
    }
    processKill = [ordered]@{
        killedWhileStreaming = $true
        previousVerifiedCachePreserved = $true
        stalePartialRecoveredOnNextOperation = $true
    }
} | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Download fault report: $fullReportPath"
