param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,
    [Parameter(Mandatory = $true)]
    [string]$TestRoot,
    [Parameter(Mandatory = $true)]
    [string]$ReportPath,
    [switch]$IsolatedEnvironment,
    [switch]$RequireAuthenticode,
    [switch]$LaunchSmoke
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

if (-not $IsolatedEnvironment) {
    throw 'Installer smoke test is destructive to the registered app identity and requires -IsolatedEnvironment.'
}
if (Test-Path -LiteralPath (Join-Path $projectRoot 'DSH Launcher\dsh-launcher.exe')) {
    throw 'A production DSH Launcher installation exists inside this workspace; refusing host installer smoke test.'
}
if (Get-Process -Name 'dsh-launcher' -ErrorAction SilentlyContinue) {
    throw 'DSH Launcher is running; refusing installer smoke test.'
}

$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$testRootPath = [System.IO.Path]::GetFullPath($TestRoot)
$reportPathValue = [System.IO.Path]::GetFullPath($ReportPath)
$testRootDrive = [System.IO.Path]::GetPathRoot($testRootPath)
if ($testRootPath -eq $testRootDrive -or $testRootPath.Length -le ($testRootDrive.Length + 8)) {
    throw "Unsafe installer test root: $testRootPath"
}

$installRoot = Join-Path $testRootPath 'install'
$launchProfileRoot = Join-Path $testRootPath 'launch-profile'
$preservedDataRoot = Join-Path $launchProfileRoot 'local-app-data\DeepSeek Harness\home'
$sentinel = Join-Path $preservedDataRoot 'must-survive-uninstall.txt'
$expectedVersion = (Get-Content -Raw -Encoding UTF8 (Join-Path $projectRoot 'package.json') | ConvertFrom-Json).version
New-Item -ItemType Directory -Force -Path $preservedDataRoot | Out-Null
[System.IO.File]::WriteAllText($sentinel, 'preserve', [System.Text.UTF8Encoding]::new($false))

$installerProcess = Start-Process -FilePath $installer -ArgumentList @('/S', "/D=$installRoot") -PassThru -Wait -WindowStyle Hidden
if ($installerProcess.ExitCode -ne 0) {
    throw "Installer exited with code $($installerProcess.ExitCode)."
}

$installedExecutable = Join-Path $installRoot 'dsh-launcher.exe'
$uninstaller = Join-Path $installRoot 'uninstall.exe'
foreach ($requiredFile in @(
    $installedExecutable,
    $uninstaller,
    (Join-Path $installRoot 'resources\dsh-bridge.mjs'),
    (Join-Path $installRoot 'resources\runtime-update-config.json'),
    (Join-Path $installRoot 'resources\CODE_SIGNING_POLICY.md'),
    (Join-Path $installRoot 'resources\DSH_LAUNCHER_LICENSE.txt'),
    (Join-Path $installRoot 'resources\PRIVACY.md'),
    (Join-Path $installRoot 'resources\RELEASE_REVIEW.md'),
    (Join-Path $installRoot 'resources\THIRD_PARTY_NOTICES.md'),
    (Join-Path $installRoot 'resources\USER_GUIDE.md')
)) {
    if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
        throw "Installed file missing: $requiredFile"
    }
}
$installedVersion = (Get-Item -LiteralPath $installedExecutable).VersionInfo.ProductVersion
if ($installedVersion -ne $expectedVersion) {
    throw "Installed version $installedVersion does not match expected $expectedVersion."
}
if ($RequireAuthenticode) {
    & (Join-Path $PSScriptRoot 'verify-authenticode.ps1') -Path $installer -RequireTimestamp
    & (Join-Path $PSScriptRoot 'verify-authenticode.ps1') -Path $installedExecutable -RequireTimestamp
}

$launchSmokePassed = $false
$launchLogPath = Join-Path $launchProfileRoot 'cache\logs\launcher.log'
if ($LaunchSmoke) {
    & (Join-Path $PSScriptRoot 'test-launcher-startup.ps1') `
        -ExecutablePath $installedExecutable `
        -TestRoot $launchProfileRoot `
        -ReportPath (Join-Path $testRootPath 'launcher-startup.json')
    $launchSmokePassed = $true
}

$uninstallerProcess = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru -Wait -WindowStyle Hidden
if ($uninstallerProcess.ExitCode -ne 0) {
    throw "Uninstaller exited with code $($uninstallerProcess.ExitCode)."
}
for ($attempt = 0; $attempt -lt 50 -and (Test-Path -LiteralPath $installedExecutable); $attempt++) {
    Start-Sleep -Milliseconds 200
}
if (Test-Path -LiteralPath $installedExecutable) {
    throw 'Installed executable remains after silent uninstall.'
}
if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) {
    throw 'Data-retention sentinel was removed by uninstall.'
}

$shortcutRoot = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\DSH Launcher'
if (Test-Path -LiteralPath $shortcutRoot) {
    throw "Start menu shortcut directory remains after uninstall: $shortcutRoot"
}

$reportParent = Split-Path -Parent $reportPathValue
New-Item -ItemType Directory -Force -Path $reportParent | Out-Null
$report = [ordered]@{
    schemaVersion = 1
    testedAtUtc = [DateTime]::UtcNow.ToString('o')
    installer = $installer
    installerSha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
    expectedVersion = $expectedVersion
    installedVersion = $installedVersion
    silentInstallExitCode = $installerProcess.ExitCode
    silentUninstallExitCode = $uninstallerProcess.ExitCode
    installedFilesVerified = $true
    installedExecutableRemoved = $true
    dataSentinelPreserved = $true
    shortcutRemoved = $true
    authenticodeRequired = [bool]$RequireAuthenticode
    launchSmokeRequested = [bool]$LaunchSmoke
    launchSmokePassed = if ($LaunchSmoke) { $launchSmokePassed } else { $null }
    isolatedLaunchLog = if ($LaunchSmoke) { $launchLogPath } else { $null }
} | ConvertTo-Json -Depth 4
[System.IO.File]::WriteAllText($reportPathValue, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Installer smoke report: $reportPathValue"
