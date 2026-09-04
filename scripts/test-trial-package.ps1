param([Parameter(Mandatory = $true)][string]$ArtifactRoot)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path $projectRoot ("phase4-results\trial-tests-" + [Guid]::NewGuid().ToString('N'))
$artifact = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$prepare = Join-Path $PSScriptRoot 'prepare-trial-package.ps1'
$results = [System.Collections.Generic.List[string]]::new()
function Assert-Test($condition, [string]$message) {
    if (-not $condition) { throw $message }
    $results.Add($message)
}
function Assert-Rejected([scriptblock]$action, [string]$expected) {
    $rejected = $false
    try { & $action } catch {
        if ($_.Exception.Message -notlike "*$expected*") { throw }
        $rejected = $true
    }
    Assert-Test $rejected "Rejects: $expected"
}

$happyRoot = Join-Path $testRoot 'happy'
& $prepare -ArtifactRoot $artifact -OutputRoot $happyRoot
$packageReport = Get-Content (Join-Path $happyRoot 'package-report.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Test ($packageReport.inputReportsVerified -and -not $packageReport.publicationPerformed) 'Verified inputs without publishing'
$expanded = Join-Path $testRoot 'expanded'
Expand-Archive -LiteralPath $packageReport.zipPath -DestinationPath $expanded
$files = @(Get-ChildItem -LiteralPath $expanded -File)
Assert-Test ($files.Count -eq $packageReport.contentFileCount) 'ZIP file count matches package report'
Assert-Test (Test-Path -LiteralPath (Join-Path $expanded 'CODEX-HANDOFF.md') -PathType Leaf) 'ZIP includes test-machine handoff'
Assert-Test ((Get-Content (Join-Path $expanded 'READ-ME-FIRST.md') -Raw -Encoding UTF8).Contains('CODEX-HANDOFF.md')) 'Readme points to handoff before testing'
foreach ($line in Get-Content (Join-Path $expanded 'SHA256SUMS.txt') -Encoding UTF8) {
    if ($line -notmatch '^([A-F0-9]{64})  ([^/\\]+)$') { throw 'Invalid checksum entry.' }
    $expectedHash = $Matches[1]
    $fileName = $Matches[2]
    Assert-Test ((Get-FileHash -LiteralPath (Join-Path $expanded $fileName)).Hash -eq $expectedHash) "ZIP checksum: $fileName"
}
Assert-Rejected { & $prepare -ArtifactRoot $artifact -OutputRoot $happyRoot } 'OutputRoot already exists'

$fixture = Join-Path $testRoot 'fixture'
Copy-Item -LiteralPath $artifact -Destination $fixture -Recurse
$fixtureInstaller = Get-ChildItem -LiteralPath $fixture -Recurse -Filter '*.exe' -File
$originalBytes = [System.IO.File]::ReadAllBytes($fixtureInstaller.FullName)
$damagedBytes = [byte[]]$originalBytes.Clone()
$damagedBytes[$damagedBytes.Length - 1] = $damagedBytes[$damagedBytes.Length - 1] -bxor 1
[System.IO.File]::WriteAllBytes($fixtureInstaller.FullName, $damagedBytes)
$rejectedOutput = Join-Path $testRoot 'must-not-exist'
Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'pinned candidate hash/size'
Assert-Test (-not (Test-Path -LiteralPath $rejectedOutput)) 'Tampered installer creates no package'
[System.IO.File]::WriteAllBytes($fixtureInstaller.FullName, $originalBytes)
$fixtureReportPath = Join-Path $fixture '_temp\phase4-installer-smoke.json'
$fixtureReport = Get-Content $fixtureReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
$fixtureReport.dataSentinelPreserved = $false
[System.IO.File]::WriteAllText($fixtureReportPath, ($fixtureReport | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))
Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'dataSentinelPreserved'
Assert-Test (-not (Test-Path -LiteralPath $rejectedOutput)) 'Failing report creates no package'
Copy-Item -LiteralPath (Join-Path $artifact '_temp\phase4-installer-smoke.json') -Destination $fixtureReportPath
foreach ($field in @('uninstallRegistrationVerified', 'uninstallRegistrationRemoved')) {
    $fixtureReport = Get-Content $fixtureReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $fixtureReport.$field = $false
    [System.IO.File]::WriteAllText($fixtureReportPath, ($fixtureReport | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
    Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } $field
    Copy-Item -LiteralPath (Join-Path $artifact '_temp\phase4-installer-smoke.json') -Destination $fixtureReportPath
}
$fixturePassivePath = Join-Path $fixture '_temp\phase4-installer-passive.json'
$fixturePassive = Get-Content $fixturePassivePath -Raw -Encoding UTF8 | ConvertFrom-Json
$fixturePassive.installerMode = 'Silent'
[System.IO.File]::WriteAllText($fixturePassivePath, ($fixturePassive | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'Both silent and passive'
Copy-Item -LiteralPath (Join-Path $artifact '_temp\phase4-installer-passive.json') -Destination $fixturePassivePath
$fixtureRuntimePath = Join-Path $fixture '_temp\clean-runtime-install.json'
foreach ($field in @('packageVersion', 'runtimeVersion', 'activeVersion')) {
    $fixtureRuntime = Get-Content $fixtureRuntimePath -Raw -Encoding UTF8 | ConvertFrom-Json
    $fixtureRuntime.versionConsistency.$field = '0.0.0-defect'
    [System.IO.File]::WriteAllText($fixtureRuntimePath, ($fixtureRuntime | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
    Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'Runtime version consistency'
    Copy-Item -LiteralPath (Join-Path $artifact '_temp\clean-runtime-install.json') -Destination $fixtureRuntimePath
}
[System.IO.File]::WriteAllText((Join-Path $fixture '_temp\dsh-installer-smoke\launcher-startup.json'), '{}')
Assert-Rejected { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'Startup report'

# Synthetic collector regression: these scoped mocks do not inspect or change the OS.
# Reports below are test fixtures, never desktop acceptance evidence.
$fakeProductType = 1
function Get-CimInstance {
    [pscustomobject]@{ BuildNumber = '26200'; ProductType = $fakeProductType;
        Caption = 'Synthetic Windows 11'; Version = '10.0.26200'; OSArchitecture = '64-bit' }
}
function Get-ItemPropertyValue { '152.0.4191.53' }
function Get-AuthenticodeSignature { [pscustomobject]@{ Status = 'NotSigned'; SignerCertificate = $null } }
$collector = Join-Path $PSScriptRoot 'collect-windows-acceptance.ps1'
$collectorArgs = @{
    ExpectedWindows = 'Windows11'; InstallerPath = $fixtureInstaller.FullName
    ReportPath = Join-Path $testRoot 'synthetic-collector.json'
    EnvironmentKind = 'physical'; BaselineClean = 'YES'; SmartScreen = 'not-shown'
    Install = 'PASS'; FirstRunWizard = 'PASS'; DefaultPaths = 'PASS'; CustomUnicodePaths = 'PASS'
    VersionConsistency = 'PASS'; UninstallEntry = 'PASS'
    OfflineRetry = 'PASS'; PortCollision = 'PASS'; StartOpenStop = 'PASS'; TrayBehavior = 'PASS'
    UninstallDataRetention = 'PASS'; Notes = 'SYNTHETIC TEST ONLY - NOT DESKTOP ACCEPTANCE EVIDENCE'
}
foreach ($kind in @('physical', 'virtual')) {
    $collectorArgs.EnvironmentKind = $kind
    & $collector @collectorArgs
    $record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Test ($record.passed -and $record.machine.environmentKind -eq $kind) "Collector accepts confirmed $kind baseline"
}
foreach ($baseline in @('NO', 'UNKNOWN')) {
    $collectorArgs.BaselineClean = $baseline
    & $collector @collectorArgs
    $record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Test (-not $record.complete -and -not $record.passed) "Collector refuses unclean/unconfirmed baseline: $baseline"
}
$collectorArgs.BaselineClean = 'YES'
$collectorArgs.EnvironmentKind = 'unknown'
& $collector @collectorArgs
$record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Test (-not $record.complete) 'Collector requires environment kind'
$collectorArgs.EnvironmentKind = 'physical'
foreach ($observation in @('VersionConsistency', 'UninstallEntry')) {
    $collectorArgs[$observation] = 'NOT_RUN'
    & $collector @collectorArgs
    $record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Test (-not $record.complete -and $record.schemaVersion -eq 3) "Collector requires defect regression observation: $observation"
    $collectorArgs[$observation] = 'PASS'
}
$collectorArgs.Install = 'NOT_RUN'
& $collector @collectorArgs
$record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Test (-not $record.passed) 'Collector never treats NOT_RUN as PASS'
$collectorArgs.Install = 'FAIL'
Assert-Rejected { & $collector @collectorArgs } 'observations failed'
$collectorArgs.Install = 'PASS'
$collectorArgs.SmartScreen = 'not-observed'
& $collector @collectorArgs
$record = Get-Content $collectorArgs.ReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Test (-not $record.complete) 'Collector requires actual SmartScreen observation'
$fakeProductType = 3
Assert-Rejected { & $collector @collectorArgs } 'Windows desktop client'
$fakeProductType = 1
$collectorArgs.ExpectedWindows = 'Windows10'
Assert-Rejected { & $collector @collectorArgs } 'Expected Windows10'

$report = [ordered]@{ schemaVersion = 1; scope = 'Synthetic packaging and collector regression only';
    passed = $true; checkCount = $results.Count; checks = @($results.ToArray()); testRoot = $testRoot }
[System.IO.File]::WriteAllText((Join-Path $testRoot 'regression-report.json'),
    ($report | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
Write-Output "PASS: $($results.Count) trial regression checks; artifacts retained at $testRoot"
exit 0
