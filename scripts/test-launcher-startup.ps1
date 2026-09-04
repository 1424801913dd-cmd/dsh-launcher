param(
    [Parameter(Mandatory = $true)][string]$ExecutablePath,
    [Parameter(Mandatory = $true)][string]$TestRoot,
    [Parameter(Mandatory = $true)][string]$ReportPath
)

$ErrorActionPreference = 'Stop'
if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; refusing startup smoke test.'
}
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$root = [System.IO.Path]::GetFullPath($TestRoot)
$reportFile = [System.IO.Path]::GetFullPath($ReportPath)
if ($root -eq [System.IO.Path]::GetPathRoot($root)) { throw 'TestRoot must not be a drive root.' }
foreach ($name in @('runtime', 'cache', 'settings.json')) {
    if (Test-Path -LiteralPath (Join-Path $root $name)) {
        throw "Startup test requires fresh runtime, cache and settings paths: $root"
    }
}
$logPath = Join-Path $root 'cache\logs\launcher.log'
$productionPointer = 'D:\Tools\dsh-launcher\active.json'
$beforeHash = if (Test-Path -LiteralPath $productionPointer) {
    (Get-FileHash -LiteralPath $productionPointer -Algorithm SHA256).Hash
} else { $null }
$unicodeUserName = "$([char]0x7528)$([char]0x6237) Name"
$unicodeWorkspaceName = "DSH $([char]0x5DE5)$([char]0x4F5C)$([char]0x533A)"
$overrides = @{
    DSH_LAUNCHER_RUNTIME_ROOT = Join-Path $root 'runtime'
    DSH_LAUNCHER_CACHE_ROOT = Join-Path $root 'cache'
    DSH_LAUNCHER_SETTINGS_PATH = Join-Path $root 'settings.json'
    DSH_LAUNCHER_DSH_HOME = Join-Path $root 'local-app-data\DeepSeek Harness\home'
    DSH_LAUNCHER_WORKSPACE = Join-Path $root "$unicodeUserName\Documents\$unicodeWorkspaceName"
}
$savedEnvironment = @{}
$launchedProcess = $null
$initialized = $false
$failure = $null
try {
    foreach ($key in $overrides.Keys) {
        $savedEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
        [Environment]::SetEnvironmentVariable($key, $overrides[$key], 'Process')
    }
    # Preserve the real Windows profile; isolate only launcher-owned data.
    $launchedProcess = Start-Process -FilePath $executable -PassThru -WindowStyle Hidden
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($launchedProcess.HasExited) {
            throw "Launcher exited unexpectedly with code $($launchedProcess.ExitCode)."
        }
        if (Test-Path -LiteralPath $logPath -PathType Leaf) {
            $logText = Get-Content -LiteralPath $logPath -Raw -Encoding UTF8
            if ($logText -like '*DSH Launcher*') { $initialized = $true; break }
        }
        Start-Sleep -Milliseconds 250
    }
    if (-not $initialized) { throw "Launcher did not initialize within 30 seconds: $logPath" }
    # A log written during setup alone must not mask a subsequent startup crash.
    if ($launchedProcess.WaitForExit(3000)) {
        throw "Launcher exited after initialization with code $($launchedProcess.ExitCode)."
    }
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($launchedProcess -and -not $launchedProcess.HasExited) {
        Stop-Process -Id $launchedProcess.Id -Force
        if (-not $launchedProcess.WaitForExit(10000)) { $failure = 'Test launcher failed to stop.' }
    }
    foreach ($key in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($key, $savedEnvironment[$key], 'Process')
    }
}
$afterHash = if (Test-Path -LiteralPath $productionPointer) {
    (Get-FileHash -LiteralPath $productionPointer -Algorithm SHA256).Hash
} else { $null }
if ($beforeHash -ne $afterHash) { $failure = 'Production active pointer changed during startup smoke.' }
$report = [ordered]@{
    schemaVersion = 1
    testedAtUtc = [DateTime]::UtcNow.ToString('o')
    scope = 'Launcher process initialization only; not a desktop UI or clean-machine acceptance test'
    executable = $executable
    executableSha256 = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
    testRoot = $root
    logPath = $logPath
    initialized = $initialized
    productionPointerHashBefore = $beforeHash
    productionPointerHashAfter = $afterHash
    productionPointerUnchanged = $beforeHash -eq $afterHash
    artifactsKept = $true
    failure = $failure
    passed = $initialized -and $null -eq $failure
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $reportFile) | Out-Null
[System.IO.File]::WriteAllText($reportFile, ($report | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
Write-Output "Launcher startup report: $reportFile"
if (-not $report.passed) { throw "Startup smoke failed: $failure" }
