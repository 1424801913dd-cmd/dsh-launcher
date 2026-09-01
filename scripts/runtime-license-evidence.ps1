param(
    [string]$RuntimePointer = 'D:\Tools\dsh-launcher\active.json',
    [string]$ReportPath = '',
    [string]$ObjdumpPath = ''
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

if (-not (Test-Path -LiteralPath $RuntimePointer -PathType Leaf)) {
    throw "Runtime pointer not found: $RuntimePointer"
}

$pointer = Get-Content -LiteralPath $RuntimePointer -Raw -Encoding UTF8 | ConvertFrom-Json
$runtimeRoot = Split-Path -Parent (Split-Path -Parent $pointer.nodePath)
$packageRoot = Join-Path $runtimeRoot 'app\node_modules\@img\sharp-win32-x64'
$packageJsonPath = Join-Path $packageRoot 'package.json'
if (-not (Test-Path -LiteralPath $packageJsonPath -PathType Leaf)) {
    throw "Installed sharp Windows x64 package not found: $packageJsonPath"
}

if (-not $ObjdumpPath) {
    $objdump = Get-Command objdump -ErrorAction SilentlyContinue
    if ($objdump) {
        $ObjdumpPath = $objdump.Source
    }
}
if (-not $ObjdumpPath -or -not (Test-Path -LiteralPath $ObjdumpPath -PathType Leaf)) {
    throw 'objdump is required to record PE import evidence. Pass -ObjdumpPath explicitly.'
}

$package = Get-Content -LiteralPath $packageJsonPath -Raw -Encoding UTF8 | ConvertFrom-Json
$binaryRoot = Join-Path $packageRoot 'lib'
$binaryNames = @(
    'sharp-win32-x64-0.35.4.node',
    'libvips-cpp-8.18.6.dll',
    'libvips-42.dll'
)
$binaries = foreach ($binaryName in $binaryNames) {
    $binaryPath = Join-Path $binaryRoot $binaryName
    if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
        throw "Expected native binary not found: $binaryPath"
    }
    $output = & $ObjdumpPath -p $binaryPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "objdump failed for $binaryPath with exit code $LASTEXITCODE"
    }
    $imports = @($output | ForEach-Object {
        if ($_ -match 'DLL Name:\s*(.+)$') { $Matches[1].Trim() }
    } | Where-Object { $_ } | Sort-Object -Unique)
    [pscustomobject]@{
        name = $binaryName
        bytes = (Get-Item -LiteralPath $binaryPath).Length
        sha256 = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash
        imports = $imports
    }
}

$report = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    runtime = [ordered]@{
        id = $pointer.id
        dshVersion = $pointer.dshVersion
        root = $runtimeRoot
    }
    package = [ordered]@{
        name = $package.name
        version = $package.version
        license = $package.license
        licenseFilePresent = Test-Path -LiteralPath (Join-Path $packageRoot 'LICENSE') -PathType Leaf
        readmePresent = Test-Path -LiteralPath (Join-Path $packageRoot 'README.md') -PathType Leaf
        versionsFilePresent = Test-Path -LiteralPath (Join-Path $packageRoot 'versions.json') -PathType Leaf
    }
    binaries = @($binaries)
    interpretationBoundary = 'PE imports prove the sharp addon uses replaceable libvips DLL files. They do not prove that every LGPL dependency inside libvips-42.dll is dynamically linked or that all relinking obligations are satisfied.'
}

if (-not $ReportPath) {
    $ReportPath = Join-Path $ProjectRoot 'phase4-results\runtime-license-evidence.json'
}
$absoluteReportPath = [IO.Path]::GetFullPath($ReportPath)
$reportDirectory = Split-Path -Parent $absoluteReportPath
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
$json = $report | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText($absoluteReportPath, $json, [Text.UTF8Encoding]::new($false))

Write-Output "Runtime license evidence: $absoluteReportPath"
Write-Output "Package: $($package.name)@$($package.version) ($($package.license))"
foreach ($binary in $binaries) {
    Write-Output "$($binary.name): imports=$($binary.imports -join ', ')"
}
