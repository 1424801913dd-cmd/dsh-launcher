$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

npm.cmd run tauri dev
