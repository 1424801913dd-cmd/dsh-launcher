$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$bundlerCacheTarget = 'D:\Caches\dsh-launcher\tauri-bundler-tools'
$bundlerCacheLink = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'tauri'
New-Item -ItemType Directory -Force -Path $bundlerCacheTarget | Out-Null
if (-not (Test-Path -LiteralPath $bundlerCacheLink)) {
    New-Item -ItemType Junction -Path $bundlerCacheLink -Target $bundlerCacheTarget | Out-Null
}
$cacheLink = Get-Item -LiteralPath $bundlerCacheLink
if (($cacheLink.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0 -or
    -not (@($cacheLink.Target) -contains $bundlerCacheTarget)) {
    throw "Tauri bundler cache must be a junction to $bundlerCacheTarget; refusing C-drive cache growth."
}

npm.cmd run tauri -- build `
    --config 'src-tauri\tauri.phase4.conf.json' `
    --bundles nsis `
    --no-sign `
    --ci
if ($LASTEXITCODE -ne 0) {
    throw "Unsigned NSIS build failed with exit code $LASTEXITCODE."
}

$bundleRoot = Join-Path $ProjectRoot 'src-tauri\target\release\bundle\nsis'
$installers = @(Get-ChildItem -LiteralPath $bundleRoot -Filter '*.exe' -File -ErrorAction Stop)
if ($installers.Count -ne 1) {
    throw "Expected exactly one NSIS installer under $bundleRoot, found $($installers.Count)."
}
$signature = Get-AuthenticodeSignature -LiteralPath $installers[0].FullName
if ($signature.Status -ne 'NotSigned') {
    throw "Local unsigned build produced unexpected signature status: $($signature.Status)."
}

[pscustomobject]@{
    Installer = $installers[0].FullName
    Bytes = $installers[0].Length
    Authenticode = $signature.Status
    BundlerCache = $bundlerCacheTarget
} | Format-List
