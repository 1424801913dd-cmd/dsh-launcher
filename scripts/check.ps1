$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

npm.cmd run check
if ($LASTEXITCODE -ne 0) { throw "npm check failed with exit code $LASTEXITCODE." }
npm.cmd run build
if ($LASTEXITCODE -ne 0) { throw "frontend build failed with exit code $LASTEXITCODE." }
cargo fmt --manifest-path 'src-tauri\Cargo.toml' --check
if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed with exit code $LASTEXITCODE." }
cargo test --manifest-path 'src-tauri\Cargo.toml'
if ($LASTEXITCODE -ne 0) { throw "cargo test failed with exit code $LASTEXITCODE." }
