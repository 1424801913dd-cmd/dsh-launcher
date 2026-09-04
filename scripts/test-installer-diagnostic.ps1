param(
    [Parameter(Mandatory=$true)][string]$InstallerPath,
    [Parameter(Mandatory=$true)][string]$TestRoot,
    [Parameter(Mandatory=$true)][string]$ReportPath
)
$ErrorActionPreference = 'Stop'
# Fixtures intentionally write the app's HKCU identity. Never run on a personal host.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Diagnostic interaction fixtures require a disposable GitHub-hosted runner.'
}
$root = [IO.Path]::GetFullPath($TestRoot)
$runnerRoot = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $root.StartsWith($runnerRoot, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $root)) {
    throw 'TestRoot must be a fresh directory inside RUNNER_TEMP.'
}
. (Join-Path $PSScriptRoot 'launcher-uninstall-registry.ps1')
if (@(Get-LauncherUninstallRegistration).Count -gt 0) { throw 'Existing installation registration found.' }
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\DSH Launcher'
$locationKey = 'HKCU:\Software\dshlauncher\DSH Launcher'
if (Test-Path -LiteralPath $locationKey) { throw 'Existing installation location key found.' }
$defaultApp = Join-Path $env:LOCALAPPDATA 'DSH Launcher'
if (Test-Path -LiteralPath $defaultApp) { throw 'Existing default application directory found.' }
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class DshDiagnosticWin32 {
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint message, IntPtr w, IntPtr l);
  public static IntPtr Dialog(uint pid) {
    IntPtr result = IntPtr.Zero;
    EnumWindows((h,p) => { uint found; GetWindowThreadProcessId(h,out found);
      if(found == pid && GetDlgItem(h,1) != IntPtr.Zero) { result=h; return false; } return true; },IntPtr.Zero);
    return result;
  }
}
'@
$results = @()
foreach ($scenario in @('clean','old-valid','orphan-default','missing-uninstaller','silent','passive')) {
    $caseRoot = Join-Path $root $scenario
    New-Item -ItemType Directory -Path $caseRoot -Force | Out-Null
    $probe = Join-Path $caseRoot 'DSH-Launcher-INSTALLER-DIAGNOSTIC.exe'
    Copy-Item -LiteralPath $installer -Destination $probe
    $sentinel = Join-Path $caseRoot 'must-survive.txt'
    [IO.File]::WriteAllText($sentinel, 'retained fixture')
    $seeded = $scenario -in @('old-valid','orphan-default','missing-uninstaller')
    $process = $null
    try {
        if ($seeded) {
            New-Item -Path $uninstallKey -Force | Out-Null
            if ($scenario -eq 'orphan-default') {
                Set-Item -LiteralPath $uninstallKey -Value 'fixture orphan'
            } else {
                $uninstaller = Join-Path $caseRoot 'uninstall.exe'
                if ($scenario -eq 'old-valid') { Copy-Item -LiteralPath $probe -Destination $uninstaller }
                New-ItemProperty -LiteralPath $uninstallKey -Name DisplayName -Value 'DSH Launcher' -PropertyType String | Out-Null
                New-ItemProperty -LiteralPath $uninstallKey -Name DisplayVersion -Value '0.4.0' -PropertyType String | Out-Null
                New-ItemProperty -LiteralPath $uninstallKey -Name UninstallString -Value ('"' + $uninstaller + '"') -PropertyType String | Out-Null
                New-Item -Path $locationKey -Force | Out-Null
                Set-Item -LiteralPath $locationKey -Value $caseRoot
            }
        }
        $before = @(Get-LauncherUninstallRegistration) | ConvertTo-Json -Depth 5 -Compress
        $launch = @{FilePath=$probe; PassThru=$true; WindowStyle='Hidden'}
        if ($scenario -eq 'silent') { $launch.ArgumentList='/S' }
        if ($scenario -eq 'passive') { $launch.ArgumentList='/P' }
        $process = Start-Process @launch
        $log = Join-Path $caseRoot ("installer-diagnostic-{0}.log" -f $process.Id)
        $text = ''
        $clicks = 0
        for ($attempt=0; $attempt -lt 100; $attempt++) {
            Start-Sleep -Milliseconds 200
            if (Test-Path -LiteralPath $log) { $text = Get-Content -LiteralPath $log -Raw }
            if ($text -match 'action=(uninstall|install)-blocked') { break }
            if ($process.HasExited) { break }
            $dialog = [DshDiagnosticWin32]::Dialog([uint32]$process.Id)
            if ($dialog -ne [IntPtr]::Zero) {
                $next = [DshDiagnosticWin32]::GetDlgItem($dialog,1)
                if ([DshDiagnosticWin32]::IsWindowEnabled($next)) {
                    [DshDiagnosticWin32]::PostMessage($dialog,0x111,[IntPtr]1,$next) | Out-Null
                    $clicks++
                    Start-Sleep -Milliseconds 300
                }
            }
        }
        if ($text -notmatch 'diagnosticOnly=true' -or $text -notmatch 'action=(uninstall|install)-blocked') {
            throw "No safe action boundary reached for $scenario. Log: $text"
        }
        if ($scenario -eq 'clean' -and $text -notmatch 'decision=no-existing-install') { throw 'Clean fixture misclassified.' }
        if ($scenario -in @('old-valid','missing-uninstaller') -and
            ($text -notmatch 'decision=maintenance-page' -or $text -notmatch 'action=uninstall-blocked')) {
            throw "Old installation fixture did not traverse the actual maintenance/uninstall choice: $scenario"
        }
        if ($scenario -notin @('silent','passive') -and $clicks -eq 0) { throw 'No actual dialog navigation occurred.' }
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force; $process.WaitForExit() }
        $after = @(Get-LauncherUninstallRegistration) | ConvertTo-Json -Depth 5 -Compress
        if ($before -ne $after -or (Test-Path -LiteralPath $defaultApp) -or
            [IO.File]::ReadAllText($sentinel) -ne 'retained fixture' -or
            @(Get-ChildItem -LiteralPath $caseRoot -Filter 'installer-diagnostic-*.log').Count -ne 1) {
            throw "Diagnostic caused unexpected changes in $scenario"
        }
        $results += [pscustomobject]@{ scenario=$scenario; passed=$true; clicks=$clicks; logPath=$log;
            registryUnchanged=$true; dataUnchanged=$true; defaultApplicationNotCreated=$true;
            decisions=@($text -split "`r?`n" | Where-Object { $_ -match '^(decision|action)=' }) }
    } finally {
        if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force; $process.WaitForExit() }
        # Exact keys created only after both identities were proven absent on this disposable runner.
        if ($seeded -and (Test-Path -LiteralPath $uninstallKey)) { Remove-Item -LiteralPath $uninstallKey -Recurse -Force }
        if ($seeded -and (Test-Path -LiteralPath $locationKey)) { Remove-Item -LiteralPath $locationKey -Recurse -Force }
    }
}
$report = [ordered]@{schemaVersion=1; scope='Diagnostic-only NSIS actual dialog navigation on disposable Windows runner';
    passed=$true; installerSha256=(Get-FileHash -LiteralPath $installer).Hash; scenarios=$results }
[IO.File]::WriteAllText([IO.Path]::GetFullPath($ReportPath), ($report | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
Write-Output "PASS: 6 diagnostic UI/safety scenarios; $ReportPath"
