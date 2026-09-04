param(
    [string]$TestRoot,
    [string]$ReportPath = 'phase4-results\clean-runtime-install.json',
    [switch]$IsolatedEnvironment,
    [switch]$KeepArtifacts
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $PSScriptRoot
if ((Test-Path -LiteralPath 'D:\Tools\node-v24.19.0-win-x64\node.exe' -PathType Leaf) -and
    (Test-Path -LiteralPath 'D:\Tools\rust\cargo\bin\cargo.exe' -PathType Leaf)) {
    . (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')
} else {
    Set-Location -LiteralPath $ProjectRoot
}

if (-not $IsolatedEnvironment) {
    throw 'From-scratch Runtime testing downloads and deletes an isolated test tree; pass -IsolatedEnvironment explicitly.'
}
if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; stop it before the isolated Runtime installation test.'
}

$timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
if ([string]::IsNullOrWhiteSpace($TestRoot)) {
    $TestRoot = "D:\Caches\dsh-launcher\tests\clean-runtime-$timestamp"
}
$testRootPath = [System.IO.Path]::GetFullPath($TestRoot)
$testRootDrive = [System.IO.Path]::GetPathRoot($testRootPath)
$testRootLeaf = Split-Path -Leaf $testRootPath
if ($testRootPath -eq $testRootDrive -or $testRootPath.Length -le ($testRootDrive.Length + 16) -or
    $testRootLeaf -notmatch '^clean-runtime-[A-Za-z0-9._-]+$') {
    throw "Unsafe clean Runtime test root: $testRootPath"
}
if (Test-Path -LiteralPath $testRootPath) {
    throw "Clean Runtime test root already exists: $testRootPath"
}

$reportPathValue = [System.IO.Path]::GetFullPath($ReportPath)
$reportParent = Split-Path -Parent $reportPathValue
New-Item -ItemType Directory -Force -Path $reportParent | Out-Null
New-Item -ItemType Directory -Force -Path $testRootPath | Out-Null
$resolvedTestRoot = (Resolve-Path -LiteralPath $testRootPath).Path
if ($resolvedTestRoot -ne $testRootPath) {
    throw "Resolved clean Runtime root changed unexpectedly: $resolvedTestRoot"
}

$productionPointer = 'D:\Tools\dsh-launcher\active.json'
$productionHashBefore = if (Test-Path -LiteralPath $productionPointer -PathType Leaf) {
    (Get-FileHash -LiteralPath $productionPointer -Algorithm SHA256).Hash
} else {
    $null
}
$startedAt = [DateTime]::UtcNow
$testExitCode = -1
$oldRuntimeRoot = $env:DSH_LAUNCHER_RUNTIME_ROOT
$oldCacheRoot = $env:DSH_LAUNCHER_CACHE_ROOT
$oldInstallRoot = $env:DSH_LAUNCHER_TEST_INSTALL_ROOT
$oldWorkspace = $env:DSH_LAUNCHER_TEST_WORKSPACE
$oldVersionReport = $env:DSH_LAUNCHER_TEST_VERSION_REPORT
$versionReportPath = Join-Path $testRootPath 'version-consistency.json'

try {
    $env:DSH_LAUNCHER_RUNTIME_ROOT = Join-Path $testRootPath 'manager'
    $env:DSH_LAUNCHER_CACHE_ROOT = Join-Path $testRootPath 'cache'
    $env:DSH_LAUNCHER_TEST_INSTALL_ROOT = Join-Path $testRootPath 'manager'
    $env:DSH_LAUNCHER_TEST_VERSION_REPORT = $versionReportPath
    # ASCII source also preserves actual Chinese characters under Windows PowerShell 5.1.
    $unicodeWorkspace = "workspace $([char]0x4E2D)$([char]0x6587) $([char]0x7A7A)$([char]0x683C)"
    $env:DSH_LAUNCHER_TEST_WORKSPACE = Join-Path $testRootPath $unicodeWorkspace

    cargo test --manifest-path 'src-tauri\Cargo.toml' `
        tests::installs_recommended_runtime_from_scratch `
        -- --ignored --exact --nocapture --test-threads=1
    $testExitCode = $LASTEXITCODE
} finally {
    $env:DSH_LAUNCHER_RUNTIME_ROOT = $oldRuntimeRoot
    $env:DSH_LAUNCHER_CACHE_ROOT = $oldCacheRoot
    $env:DSH_LAUNCHER_TEST_INSTALL_ROOT = $oldInstallRoot
    $env:DSH_LAUNCHER_TEST_WORKSPACE = $oldWorkspace
    $env:DSH_LAUNCHER_TEST_VERSION_REPORT = $oldVersionReport
}

$productionHashAfter = if (Test-Path -LiteralPath $productionPointer -PathType Leaf) {
    (Get-FileHash -LiteralPath $productionPointer -Algorithm SHA256).Hash
} else {
    $null
}
$managedProcesses = @(Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction Stop | Where-Object {
    $_.CommandLine -like '*dsh-bridge.mjs*' -and $_.CommandLine -like "*$testRootPath*"
})
$versionEvidence = if (Test-Path -LiteralPath $versionReportPath -PathType Leaf) {
    Get-Content -LiteralPath $versionReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
} else { $null }
$versionVerified = $null -ne $versionEvidence -and $versionEvidence.passed -eq $true -and
    -not [string]::IsNullOrWhiteSpace($versionEvidence.expectedVersion) -and
    $versionEvidence.packageVersion -eq $versionEvidence.expectedVersion -and
    $versionEvidence.runtimeVersion -eq $versionEvidence.expectedVersion -and
    $versionEvidence.activeVersion -eq $versionEvidence.expectedVersion
$passed = $testExitCode -eq 0 -and $versionVerified -and $managedProcesses.Count -eq 0 -and
    $productionHashBefore -eq $productionHashAfter
$finishedAt = [DateTime]::UtcNow
$report = [ordered]@{
    schemaVersion = 1
    scope = 'from-scratch Runtime integration using an isolated developer harness'
    startedAtUtc = $startedAt.ToString('o')
    finishedAtUtc = $finishedAt.ToString('o')
    durationSeconds = [math]::Round(($finishedAt - $startedAt).TotalSeconds, 3)
    testRoot = $testRootPath
    testExitCode = $testExitCode
    versionConsistency = $versionEvidence
    unicodeAndSpaceWorkspace = $true
    remainingManagedDshProcesses = $managedProcesses.Count
    productionPointer = $productionPointer
    productionPointerHashBefore = $productionHashBefore
    productionPointerHashAfter = $productionHashAfter
    productionPointerUnchanged = $productionHashBefore -eq $productionHashAfter
    artifactsKept = [bool]$KeepArtifacts -or -not $passed
    passed = $passed
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($reportPathValue, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))

if ($passed -and -not $KeepArtifacts) {
    $resolvedForCleanup = (Resolve-Path -LiteralPath $testRootPath).Path
    if ($resolvedForCleanup -ne $testRootPath -or (Split-Path -Leaf $resolvedForCleanup) -notmatch '^clean-runtime-') {
        throw "Refusing cleanup of unexpected path: $resolvedForCleanup"
    }
    Remove-Item -LiteralPath $resolvedForCleanup -Recurse -Force
}

Write-Output "Clean Runtime install report: $reportPathValue"
if (-not $passed) {
    throw "Clean Runtime installation test failed; isolated evidence remains at $testRootPath"
}
