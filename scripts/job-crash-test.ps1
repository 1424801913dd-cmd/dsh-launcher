param(
    [string]$ReportPath = 'phase4-results\job-crash.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; stop it before the Job crash test.'
}

$testRoot = Join-Path 'D:\Caches\dsh-launcher\tests' "job-crash-$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
$activePointer = 'D:\Tools\dsh-launcher\active.json'
$activeRuntime = Get-Content -Raw -Encoding UTF8 $activePointer | ConvertFrom-Json
$startedAt = [DateTime]::UtcNow
$previousRoot = $env:DSH_LAUNCHER_JOB_CRASH_ROOT
$previousPointer = $env:DSH_LAUNCHER_JOB_CRASH_ACTIVE_POINTER
try {
    $env:DSH_LAUNCHER_JOB_CRASH_ROOT = $testRoot
    $env:DSH_LAUNCHER_JOB_CRASH_ACTIVE_POINTER = $activePointer
    cargo test --manifest-path 'src-tauri\Cargo.toml' `
        --test job_crash `
        job_object_kills_dsh_tree_when_supervisor_process_crashes `
        -- --ignored --nocapture --test-threads=1
    $testExitCode = $LASTEXITCODE
} finally {
    $env:DSH_LAUNCHER_JOB_CRASH_ROOT = $previousRoot
    $env:DSH_LAUNCHER_JOB_CRASH_ACTIVE_POINTER = $previousPointer
}
if ($testExitCode -ne 0) {
    throw "Job crash test failed with exit code $testExitCode."
}

$remaining = @(Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction Stop | Where-Object {
    $_.CommandLine -like '*dsh-bridge.mjs*'
})
if ($remaining.Count -ne 0) {
    throw "Job crash test left $($remaining.Count) managed DSH process(es)."
}
if (Test-Path -LiteralPath $testRoot) {
    throw "Job crash test root remains after success: $testRoot"
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
    supervisorExitMode = 'process::exit(86) without graceful DSH shutdown'
    runtimeId = $activeRuntime.id
    dshVersion = $activeRuntime.dshVersion
    nodeVersion = $activeRuntime.nodeVersion
    remainingManagedDshProcesses = $remaining.Count
    testRootRemoved = $true
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Job crash report: $fullReportPath"
