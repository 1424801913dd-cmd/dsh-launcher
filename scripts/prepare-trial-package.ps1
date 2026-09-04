param(
    [Parameter(Mandatory = $true)][string]$ArtifactRoot,
    [Parameter(Mandatory = $true)][string]$OutputRoot
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$candidatePath = Join-Path $PSScriptRoot 'data\trial-candidate.json'
$candidate = Get-Content -LiteralPath $candidatePath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($candidate.schemaVersion -ne 1 -or $candidate.distribution -ne 'controlled-unsigned-trial' -or
    $candidate.remoteUpdatesEnabled -ne $false -or $candidate.candidateId -notmatch '^[A-Za-z0-9._-]+$') {
    throw 'Invalid controlled-trial candidate metadata.'
}
$artifact = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$output = [System.IO.Path]::GetFullPath($OutputRoot)
if (Test-Path -LiteralPath $output) { throw "OutputRoot already exists; refusing overwrite: $output" }

$installers = @(Get-ChildItem -LiteralPath $artifact -Recurse -File -Filter '*.exe')
if ($installers.Count -ne 1 -or $installers[0].Name -ne $candidate.installerName) {
    throw 'Expected exactly the pinned CI installer, with no additional executables.'
}
$installer = $installers[0]
$installerHash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash
if ($installerHash -ne $candidate.installerSha256 -or $installer.Length -ne $candidate.installerBytes) {
    throw 'CI installer does not match the pinned candidate hash/size.'
}
$signature = Get-AuthenticodeSignature -LiteralPath $installer.FullName
if ($signature.Status -ne 'NotSigned') { throw 'This package is reserved for the pinned unsigned trial.' }

$runtimeReportPath = Join-Path $artifact '_temp\clean-runtime-install.json'
$installReportPath = Join-Path $artifact '_temp\phase4-installer-smoke.json'
$passiveReportPath = Join-Path $artifact '_temp\phase4-installer-passive.json'
$startupReportPath = Join-Path $artifact '_temp\dsh-installer-smoke\launcher-startup.json'
$runtimeReport = Get-Content -LiteralPath $runtimeReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
$installReport = Get-Content -LiteralPath $installReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
$passiveReport = Get-Content -LiteralPath $passiveReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
$startupReport = Get-Content -LiteralPath $startupReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($runtimeReport.schemaVersion -ne 1 -or $runtimeReport.passed -ne $true -or
    $runtimeReport.testExitCode -ne 0 -or $runtimeReport.remainingManagedDshProcesses -ne 0 -or
    $runtimeReport.unicodeAndSpaceWorkspace -ne $true -or $runtimeReport.productionPointerUnchanged -ne $true) {
    throw 'From-scratch Runtime report is missing passing evidence.'
}
$versions = $runtimeReport.versionConsistency
if ($null -eq $versions -or $versions.passed -ne $true -or
    [string]::IsNullOrWhiteSpace($versions.expectedVersion) -or
    $versions.packageVersion -ne $versions.expectedVersion -or
    $versions.runtimeVersion -ne $versions.expectedVersion -or
    $versions.activeVersion -ne $versions.expectedVersion) {
    throw 'Runtime version consistency evidence is missing or inconsistent.'
}
if ($installReport.installerMode -ne 'Silent' -or $passiveReport.installerMode -ne 'Passive') {
    throw 'Both silent and passive installer evidence is required.'
}
foreach ($installReport in @($installReport, $passiveReport)) {
if ($installReport.schemaVersion -ne 1 -or $installReport.installerSha256 -ne $installerHash -or
    $installReport.expectedVersion -ne $candidate.version -or $installReport.installedVersion -ne $candidate.version -or
    $installReport.silentInstallExitCode -ne 0 -or $installReport.silentUninstallExitCode -ne 0) {
    throw 'Installer report does not match the candidate or has a failing exit code.'
}
foreach ($field in @('installedFilesVerified', 'installedExecutableRemoved', 'dataSentinelPreserved',
    'shortcutRemoved', 'launchSmokeRequested', 'launchSmokePassed',
    'uninstallRegistrationVerified', 'uninstallRegistrationRemoved')) {
    if ($installReport.$field -ne $true) { throw "Installer report missing passing check: $field" }
}
. (Join-Path $PSScriptRoot 'launcher-uninstall-registry.ps1')
foreach ($entries in @('uninstallRegistrationBeforeLaunch', 'uninstallRegistrationAfterLaunch')) {
    $registration = @($installReport.$entries)
    $location = ([string]($registration | Select-Object -First 1).InstallLocation).Trim('"')
    Assert-LauncherUninstallRegistration -Entries $registration -InstallRoot $location -ExpectedVersion $candidate.version
}
}
if ($startupReport.schemaVersion -ne 1 -or $startupReport.passed -ne $true -or
    $startupReport.initialized -ne $true -or $null -ne $startupReport.failure -or
    $startupReport.productionPointerUnchanged -ne $true -or
    $startupReport.executableSha256 -ne $candidate.installedExecutableSha256) {
    throw 'Startup report does not prove the pinned executable initialized.'
}

# Only an explicit allowlist is copied. Never ship profiles, settings, logs, or keys.
$sources = [ordered]@{
    'READ-ME-FIRST.md' = Join-Path $projectRoot 'docs\TRIAL_GUIDE.md'
    'CODEX-HANDOFF.md' = Join-Path $projectRoot 'docs\TEST_MACHINE_HANDOFF.md'
    'TRIAL_RELEASE_NOTES.md' = Join-Path $projectRoot 'docs\TRIAL_RELEASE_NOTES.md'
    'FEEDBACK.md' = Join-Path $projectRoot 'docs\TRIAL_FEEDBACK.md'
    'USER_GUIDE.md' = Join-Path $projectRoot 'docs\USER_GUIDE.md'
    'WINDOWS_ACCEPTANCE.md' = Join-Path $projectRoot 'docs\WINDOWS_ACCEPTANCE.md'
    'PRIVACY.md' = Join-Path $projectRoot 'docs\PRIVACY.md'
    'CODE_SIGNING_POLICY.md' = Join-Path $projectRoot 'docs\CODE_SIGNING_POLICY.md'
    'THIRD_PARTY_NOTICES.md' = Join-Path $projectRoot 'docs\THIRD_PARTY_NOTICES.md'
    'LICENSE.txt' = Join-Path $projectRoot 'LICENSE'
    'collect-windows-acceptance.ps1' = Join-Path $PSScriptRoot 'collect-windows-acceptance.ps1'
    'inspect-installed-launcher.ps1' = Join-Path $PSScriptRoot 'inspect-installed-launcher.ps1'
    'launcher-uninstall-registry.ps1' = Join-Path $PSScriptRoot 'launcher-uninstall-registry.ps1'
    'candidate.json' = $candidatePath
    'ci-runtime.json' = $runtimeReportPath
    'ci-installer.json' = $installReportPath
    'ci-installer-passive.json' = $passiveReportPath
    'ci-startup.json' = $startupReportPath
}
foreach ($source in $sources.Values) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing package input: $source" }
}
$contentRoot = Join-Path $output 'contents'
New-Item -ItemType Directory -Path $contentRoot -Force | Out-Null
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $contentRoot $installer.Name)
foreach ($name in $sources.Keys) {
    Copy-Item -LiteralPath $sources[$name] -Destination (Join-Path $contentRoot $name)
}
$checksums = @(Get-ChildItem -LiteralPath $contentRoot -File | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash, $_.Name
})
[System.IO.File]::WriteAllText((Join-Path $contentRoot 'SHA256SUMS.txt'),
    ($checksums -join "`n") + "`n", [System.Text.UTF8Encoding]::new($false))
$zipPath = Join-Path $output ("DSH-Launcher-{0}.zip" -f $candidate.candidateId)
Compress-Archive -Path (Join-Path $contentRoot '*') -DestinationPath $zipPath -CompressionLevel Optimal
$zipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash
$report = [ordered]@{
    schemaVersion = 1
    createdAtUtc = [DateTime]::UtcNow.ToString('o')
    candidateId = $candidate.candidateId
    sourceCommit = $candidate.sourceCommit
    ciRunUrl = $candidate.ciRunUrl
    installerSha256 = $installerHash
    authenticode = [string]$signature.Status
    zipPath = $zipPath
    zipBytes = (Get-Item -LiteralPath $zipPath).Length
    zipSha256 = $zipHash
    inputReportsVerified = $true
    contentFileCount = $checksums.Count + 1
    publicationPerformed = $false
    windows10Desktop = 'NOT_RUN'
    windows11Desktop = 'NOT_RUN'
}
[System.IO.File]::WriteAllText((Join-Path $output 'package-report.json'),
    ($report | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
Write-Output "Controlled trial package: $zipPath"
Write-Output "ZIP SHA256: $zipHash"
