$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$artifact = Join-Path $ProjectRoot 'src-tauri\target\release\dsh-launcher.exe'
if (Test-Path -LiteralPath $artifact) {
    Remove-Item -LiteralPath $artifact -Force
}
npm.cmd run tauri build
if ($LASTEXITCODE -ne 0) {
    throw "Tauri build failed with exit code $LASTEXITCODE."
}
if (-not (Test-Path -LiteralPath $artifact)) {
    throw "Tauri did not produce the expected Release executable: $artifact"
}
Write-Output $artifact
