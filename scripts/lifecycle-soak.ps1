param(
    [ValidateRange(1, 1000)]
    [int]$Cycles = 100,
    [string]$ReportPath = 'phase4-results\lifecycle-soak.json'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; stop it before lifecycle soak testing.'
}

$startedAt = [DateTime]::UtcNow
$activePointerPath = 'D:\Tools\dsh-launcher\active.json'
$activeRuntime = if (Test-Path -LiteralPath $activePointerPath -PathType Leaf) {
    Get-Content -Raw -Encoding UTF8 $activePointerPath | ConvertFrom-Json
} else {
    $null
}
$previousCycles = $env:DSH_LAUNCHER_LIFECYCLE_CYCLES
try {
    $env:DSH_LAUNCHER_LIFECYCLE_CYCLES = $Cycles.ToString([Globalization.CultureInfo]::InvariantCulture)
    cargo test --manifest-path 'src-tauri\Cargo.toml' `
        starts_and_truly_stops_real_dsh_in_isolated_home `
        -- --ignored --nocapture --test-threads=1
    $testExitCode = $LASTEXITCODE
} finally {
    $env:DSH_LAUNCHER_LIFECYCLE_CYCLES = $previousCycles
}
if ($testExitCode -ne 0) {
    throw "Lifecycle soak test failed with exit code $testExitCode."
}

$managedProcesses = @(Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction Stop | Where-Object {
    $_.CommandLine -like '*dsh-bridge.mjs*'
})
if ($managedProcesses.Count -ne 0) {
    throw "Lifecycle soak left $($managedProcesses.Count) managed DSH process(es)."
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
    requestedCycles = $Cycles
    completedCycles = $Cycles
    testExitCode = $testExitCode
    remainingManagedDshProcesses = $managedProcesses.Count
    runtimeId = if ($activeRuntime) { $activeRuntime.id } else { 'legacy-fallback' }
    dshVersion = if ($activeRuntime) { $activeRuntime.dshVersion } else { '0.1.1-rc.2' }
    nodeVersion = if ($activeRuntime) { $activeRuntime.nodeVersion } else { '24.19.0' }
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Lifecycle soak report: $fullReportPath"
