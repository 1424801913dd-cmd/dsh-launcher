$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'launcher-uninstall-registry.ps1')
# Synthetic entries only: never install, uninstall or modify this machine's registry.
$root = 'D:\fixture\install'
$valid = @{ Hive = 'CurrentUser'; View = 'Registry64'; DisplayName = 'DSH Launcher';
    DisplayVersion = '0.4.1'; InstallLocation = '"D:\fixture\install"';
    UninstallString = '"D:\fixture\install\uninstall.exe"'; SystemComponent = 0 }
Assert-LauncherUninstallRegistration -Entries @([pscustomobject]$valid) -InstallRoot $root -ExpectedVersion '0.4.1'
$cases = @(
    @{ Field = 'Hive'; Value = 'LocalMachine' },
    @{ Field = 'DisplayVersion'; Value = '0.4.0' },
    @{ Field = 'DisplayName'; Value = 'Other app' },
    @{ Field = 'InstallLocation'; Value = 'D:\other' },
    @{ Field = 'UninstallString'; Value = 'D:\other\uninstall.exe' },
    @{ Field = 'SystemComponent'; Value = 1 }
)
foreach ($case in $cases) {
    $entry = $valid.Clone()
    $entry[$case.Field] = $case.Value
    $rejected = $false
    try { Assert-LauncherUninstallRegistration -Entries @([pscustomobject]$entry) -InstallRoot $root -ExpectedVersion '0.4.1' }
    catch { if ($_.Exception.Message -notlike '*registration missing*') { throw }; $rejected = $true }
    if (-not $rejected) { throw "Invalid registration accepted: $($case.Field)" }
}
$rejected = $false
try { Assert-LauncherUninstallRegistration -Entries @() -InstallRoot $root -ExpectedVersion '0.4.1' }
catch { if ($_.Exception.Message -notlike '*registration missing*') { throw }; $rejected = $true }
if (-not $rejected) { throw 'Missing registration accepted.' }
Write-Output 'PASS: 8 synthetic uninstall-registration checks; no registry changes.'
