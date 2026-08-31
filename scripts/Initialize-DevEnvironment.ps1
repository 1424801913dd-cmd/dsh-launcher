$ProjectRoot = Split-Path -Parent $PSScriptRoot

$env:RUSTUP_HOME = 'D:\Tools\rust\rustup'
$env:CARGO_HOME = 'D:\Tools\rust\cargo'
$env:CARGO_TARGET_DIR = Join-Path $ProjectRoot 'src-tauri\target'
$env:npm_config_cache = 'D:\Caches\npm'
$env:Path = 'D:\Tools\node-v24.19.0-win-x64;D:\Tools\rust\cargo\bin;' + $env:Path

Set-Location -LiteralPath $ProjectRoot
