param(
    [string]$ReportPath = 'phase4-results\update-health-fault.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; stop it before the update health fault test.'
}

$activePointer = 'D:\Tools\dsh-launcher\active.json'
if (-not (Test-Path -LiteralPath $activePointer -PathType Leaf)) {
    throw "Active Runtime pointer is missing: $activePointer"
}
$activeRuntime = Get-Content -Raw -Encoding UTF8 $activePointer | ConvertFrom-Json
$testRoot = Join-Path 'D:\Caches\dsh-launcher\tests' "update-health-fault-$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
$startedAt = [DateTime]::UtcNow
$previousRoot = $env:DSH_LAUNCHER_UPDATE_FAULT_ROOT
$previousPointer = $env:DSH_LAUNCHER_UPDATE_FAULT_ACTIVE_POINTER
try {
    $env:DSH_LAUNCHER_UPDATE_FAULT_ROOT = $testRoot
    $env:DSH_LAUNCHER_UPDATE_FAULT_ACTIVE_POINTER = $activePointer
    cargo test --manifest-path 'src-tauri\Cargo.toml' `
        production_health_failure_rolls_back_to_previous_runtime `
        -- --ignored --nocapture --test-threads=1
    $testExitCode = $LASTEXITCODE
} finally {
    $env:DSH_LAUNCHER_UPDATE_FAULT_ROOT = $previousRoot
    $env:DSH_LAUNCHER_UPDATE_FAULT_ACTIVE_POINTER = $previousPointer
}
if ($testExitCode -ne 0) {
    throw "Update health fault test failed with exit code $testExitCode."
}

$remaining = @(Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction Stop | Where-Object {
    $_.CommandLine -like '*dsh-bridge.mjs*'
})
if ($remaining.Count -ne 0) {
    throw "Update health fault test left $($remaining.Count) managed DSH process(es)."
}
if (Test-Path -LiteralPath $testRoot) {
    throw "Update health fault test root remains after success: $testRoot"
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
    injectedFailure = 'candidate DSH entry calls process.exit(42) before health becomes available'
    runtimeId = $activeRuntime.id
    dshVersion = $activeRuntime.dshVersion
    nodeVersion = $activeRuntime.nodeVersion
    previousRuntimeRestartedAndStopped = $true
    activePointerRestored = $true
    remainingManagedDshProcesses = $remaining.Count
    testRootRemoved = $true
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Update health fault report: $fullReportPath"
