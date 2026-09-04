param([Parameter(Mandatory = $true)][string]$ReportPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'launcher-uninstall-registry.ps1')
$entries = @(Get-LauncherUninstallRegistration)
$reportFile = [System.IO.Path]::GetFullPath($ReportPath)
if (Test-Path -LiteralPath $reportFile) { throw 'Report already exists; choose a new report path.' }
$report = [ordered]@{
    schemaVersion = 1
    checkedAtUtc = [DateTime]::UtcNow.ToString('o')
    scope = 'Read-only inspection of the Tauri NSIS DSH Launcher uninstall key in HKCU/HKLM 32/64-bit views'
    note = 'Run as the same Windows account that installed the app; review local paths before sharing. No repairs performed.'
    registrations = $entries
    registrationFound = $entries.Count -gt 0
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $reportFile) | Out-Null
[System.IO.File]::WriteAllText($reportFile, ($report | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
Write-Output "Read-only uninstall registration report: $reportFile"
