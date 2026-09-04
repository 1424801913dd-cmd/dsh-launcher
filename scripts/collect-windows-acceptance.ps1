param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Windows10', 'Windows11')]
    [string]$ExpectedWindows,
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,
    [Parameter(Mandatory = $true)]
    [string]$ReportPath,
    [ValidateSet('physical', 'virtual', 'unknown')]
    [string]$EnvironmentKind = 'unknown',
    [ValidateSet('YES', 'NO', 'UNKNOWN')]
    [string]$BaselineClean = 'UNKNOWN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$Install = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$FirstRunWizard = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$DefaultPaths = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$CustomUnicodePaths = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$OfflineRetry = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$PortCollision = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$StartOpenStop = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$TrayBehavior = 'NOT_RUN',
    [ValidateSet('PASS', 'FAIL', 'NOT_RUN')]
    [string]$UninstallDataRetention = 'NOT_RUN',
    [ValidateSet('not-shown', 'warning-shown', 'reputation-warning', 'blocked', 'not-observed')]
    [string]$SmartScreen = 'not-observed',
    [string]$Notes = ''
)

$ErrorActionPreference = 'Stop'
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$reportPathValue = [System.IO.Path]::GetFullPath($ReportPath)
$os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
$build = [int]$os.BuildNumber
$detectedWindows = if ($build -ge 22000) { 'Windows11' } else { 'Windows10' }
$desktopClient = [int]$os.ProductType -eq 1
if (-not $desktopClient) {
    throw "Acceptance evidence requires Windows desktop client; detected ProductType=$($os.ProductType) ($($os.Caption))."
}
if ($detectedWindows -ne $ExpectedWindows) {
    throw "Expected $ExpectedWindows but detected $detectedWindows build $build."
}
if ($build -lt 10240 -or [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'Acceptance evidence requires Windows 10/11 x64.'
}

$webViewClientKey = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$webView2Version = $null
foreach ($registryPath in @(
    "Registry::HKEY_CURRENT_USER\$webViewClientKey",
    "Registry::HKEY_LOCAL_MACHINE\$webViewClientKey",
    "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)) {
    $value = Get-ItemPropertyValue -LiteralPath $registryPath -Name 'pv' -ErrorAction SilentlyContinue
    if ($value) {
        $webView2Version = [string]$value
        break
    }
}

$observations = [ordered]@{
    install = $Install
    firstRunWizard = $FirstRunWizard
    defaultPaths = $DefaultPaths
    customUnicodePaths = $CustomUnicodePaths
    offlineRetry = $OfflineRetry
    portCollision = $PortCollision
    startOpenStop = $StartOpenStop
    trayBehavior = $TrayBehavior
    uninstallDataRetention = $UninstallDataRetention
}
$statuses = @($observations.Values)
$complete = @($statuses | Where-Object { $_ -ne 'PASS' }).Count -eq 0 -and
    $SmartScreen -ne 'not-observed' -and $BaselineClean -eq 'YES' -and $EnvironmentKind -ne 'unknown'
$failed = @($statuses | Where-Object { $_ -eq 'FAIL' }).Count -gt 0
$signature = Get-AuthenticodeSignature -LiteralPath $installer
$report = [ordered]@{
    schemaVersion = 2
    recordedAtUtc = [DateTime]::UtcNow.ToString('o')
    evidenceKind = 'manual Windows desktop acceptance'
    machine = [ordered]@{
        environmentKind = $EnvironmentKind
        baselineClean = $BaselineClean
        caption = $os.Caption
        version = $os.Version
        build = $build
        productType = [int]$os.ProductType
        expectedWindows = $ExpectedWindows
        detectedWindows = $detectedWindows
        architecture = $os.OSArchitecture
        webView2Version = $webView2Version
    }
    installer = [ordered]@{
        path = $installer
        bytes = (Get-Item -LiteralPath $installer).Length
        sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
        authenticodeStatus = [string]$signature.Status
        signer = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
    }
    observations = $observations
    smartScreen = $SmartScreen
    notes = $Notes
    complete = $complete
    passed = $complete -and -not $failed
} | ConvertTo-Json -Depth 6
$reportParent = Split-Path -Parent $reportPathValue
New-Item -ItemType Directory -Force -Path $reportParent | Out-Null
[System.IO.File]::WriteAllText($reportPathValue, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Write-Output "Windows desktop acceptance report: $reportPathValue"

if ($failed) {
    throw 'One or more Windows desktop acceptance observations failed.'
}
if (-not $complete) {
    Write-Warning 'Acceptance incomplete: checks, SmartScreen, environment kind or clean baseline are not confirmed.'
    exit 2
}
