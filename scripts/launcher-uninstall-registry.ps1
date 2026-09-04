# Shared read-only inspection; no registry writes and no command execution.
function Get-LauncherUninstallRegistration {
    $subKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\DSH Launcher'
    foreach ($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser, [Microsoft.Win32.RegistryHive]::LocalMachine)) {
        foreach ($view in @([Microsoft.Win32.RegistryView]::Registry32, [Microsoft.Win32.RegistryView]::Registry64)) {
            $baseKey = $null
            $key = $null
            try {
                $baseKey = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
                $key = $baseKey.OpenSubKey($subKey, $false)
                if ($null -ne $key) {
                    [pscustomobject]@{
                        Hive = [string]$hive
                        View = [string]$view
                        Key = $subKey
                        DisplayName = $key.GetValue('DisplayName')
                        DisplayVersion = $key.GetValue('DisplayVersion')
                        InstallLocation = $key.GetValue('InstallLocation')
                        UninstallString = $key.GetValue('UninstallString')
                        SystemComponent = $key.GetValue('SystemComponent', 0)
                    }
                }
            } finally {
                if ($null -ne $key) { $key.Dispose() }
                if ($null -ne $baseKey) { $baseKey.Dispose() }
            }
        }
    }
}

function Assert-LauncherUninstallRegistration {
    param([object[]]$Entries, [string]$InstallRoot, [string]$ExpectedVersion)
    $matching = @($Entries | Where-Object {
        $_.Hive -eq 'CurrentUser' -and $_.DisplayName -eq 'DSH Launcher' -and
        $_.DisplayVersion -eq $ExpectedVersion -and
        ([string]$_.InstallLocation).Trim('"') -eq $InstallRoot -and
        ([string]$_.UninstallString).Trim('"') -eq (Join-Path $InstallRoot 'uninstall.exe') -and
        $_.SystemComponent -ne 1
    })
    if ($matching.Count -eq 0) {
        throw 'Current-user uninstall registration missing, hidden or inconsistent with installed version/path.'
    }
}
