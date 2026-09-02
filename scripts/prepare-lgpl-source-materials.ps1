#requires -Version 5.1
# Assemblies the LGPL / MPL Corresponding-Source materials archive for the
# managed Runtime's libvips Windows x64 web static DLL family.
#
# Input:  scripts/data/lgpl-source-manifest.json (review-verified pins + SHA-256)
# Output: release-assets/lgpl-source-materials-<target>.tar.gz
#         release-assets/lgpl-source-materials-<target>.json (report)
#
# The archive contains, for the exact binary set shipped in
# @img/sharp-win32-x64@<version> lib/ (libvips-42.dll family):
#   - sources/   exact upstream source tarballs of the statically included
#                LGPL components (and cairo under its MPL 2.0 path)
#   - repos/     snapshots of the three build-chain repositories that
#                reproduce the vips-dev-x64-web-*-static.zip packaging
#   - PROVENANCE.md, SHA256SUMS.txt
#
# -SkipDownload assembles from the local evidence cache
# (.tmp-lgpl-review\sources and the three repo checkouts). Without it the
# script downloads every entry and verifies SHA-256; any mismatch fails
# closed. This script performs no license judgment itself; see
# docs/LGPL_REVIEW.md.

param(
    [string]$OutputDir = 'release-assets',
    [string]$StagingDir = '.tmp-lgpl-review\staging',
    [switch]$SkipDownload,
    [switch]$KeepStaging,
    [string[]]$ExcludeSource = @()
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $PSScriptRoot 'data\lgpl-source-manifest.json'
$tarExe = Join-Path $env:SystemRoot 'System32\tar.exe'

function Assert-Hash([string]$file, [string]$expected, [string]$label) {
    $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
    if ($actual -ne $expected) {
        throw "SHA-256 mismatch for $label`: expected $expected, got $actual ($file)."
    }
    Write-Host "  verified $label : $expected"
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Manifest not found: $manifestPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json

$targetName = 'lgpl-source-materials-vips-8.18.6'
$bundleRoot = Join-Path $StagingDir $targetName
$sourcesRoot = Join-Path $bundleRoot 'sources'
$reposRoot = Join-Path $bundleRoot 'repos'
$bundleDir = Split-Path -Parent $bundleRoot
New-Item -ItemType Directory -Force -Path $sourcesRoot, $reposRoot | Out-Null
if (Test-Path -LiteralPath (Join-Path $bundleRoot 'PROVENANCE.md')) {
    Remove-Item -LiteralPath (Join-Path $bundleRoot 'PROVENANCE.md') -Force
}

# 1. Exact upstream sources -------------------------------------------------
$pending = @()
foreach ($entry in $manifest.sources) {
    $label = "$($entry.name)-$($entry.version)"
    if ($ExcludeSource -contains $entry.name) {
        Write-Host "  skipped (excluded): $label"
        $pending += $label
        continue
    }
    $target = Join-Path $sourcesRoot $entry.file
    if ($SkipDownload) {
        $cache = Join-Path $repoRoot ".tmp-lgpl-review\sources\$($entry.file)"
        if (-not (Test-Path -LiteralPath $cache -PathType Leaf)) {
            throw "SkipDownload requested but local cache entry is missing: $cache (run without -SkipDownload with network access)."
        }
        Copy-Item -LiteralPath $cache -Destination $target -Force
    } else {
        Write-Host "downloading $($entry.url)"
        Invoke-WebRequest -UseBasicParsing -Uri $entry.url -OutFile $target
    }
    Assert-Hash $target $entry.sha256 $label
}

# 2. Build-chain repository snapshots ---------------------------------------
foreach ($repo in $manifest.repos) {
    $archiveName = "$($repo.name.Replace('/','-'))-$($repo.pin).tar.gz"
    $target = Join-Path $reposRoot $archiveName
    if ($SkipDownload) {
        $local = Join-Path $repoRoot ".tmp-lgpl-review\$($repo.localDir)"
        if (-not (Test-Path -LiteralPath $local -PathType Container)) {
            throw "SkipDownload requested but local repo checkout is missing: $local (run without -SkipDownload with network access)."
        }
        # Archive the checkout with its original top-level directory name so
        # the snapshot keeps the same <dir> name as the downloaded codeload form.
        $parentDir = Split-Path -Parent $local
        $leafDir = Split-Path -Leaf $local
        & $tarExe -czf $target -C $parentDir $leafDir
        if ($LASTEXITCODE -ne 0) { throw "Failed to archive local repo: $local" }
    } else {
        Write-Host "downloading $($repo.url)"
        Invoke-WebRequest -UseBasicParsing -Uri $repo.url -OutFile $target
    }
    Write-Host "  repo $($repo.name) @ $($repo.pin) : $((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash)"
}

# 3. Provenance + checksums inside the archive ------------------------------
$provenance = @"
# Corresponding-source materials for $($manifest.target)

Generated: $(Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
Manifest: $($manifestPath.Replace($repoRoot, '.'))
Review: docs/LGPL_REVIEW.md

This archive contains the exact upstream source tarballs and build-chain
repository snapshots for the statically included LGPL components (and the
MPL 2.0 path of cairo) inside the Windows x64 libvips DLL family shipped as
part of the managed Runtime bundle (`@img/sharp-win32-x64` lib/ directory).

All source checksums were verified against the recipe records of
libvips/build-win64-mxe v8.18.6 and its MXE fork base before archiving;
see scripts/data/lgpl-source-manifest.json.

Build chain:
  libvips 8.18.6 + dependencies (sources/)
    -> build-win64-mxe v8.18.6 (repos/) -> vips-dev-x64-web-8.18.6-static.zip
    -> sharp-libvips v1.3.3 build/win.sh (repos/) -> @img/sharp-win32-x64 lib/

License notices: every component tarball carries its own COPYING / LICENSE /
COPYING.LIB text. GNU GPL-3.0 and GNU LGPL-3.0 full texts are shipped in the
Runtime bundle at app/licenses/, and per-component notices are recorded in
app/THIRD_PARTY_NOTICES.md and app/RUNTIME_LICENSES.md.

This archive is a distribution-material record; it is not a legal opinion.
"@
[System.IO.File]::WriteAllText((Join-Path $bundleRoot 'PROVENANCE.md'), ($provenance + "`n"), (New-Object System.Text.UTF8Encoding($false)))

$allFiles = Get-ChildItem -LiteralPath $sourcesRoot -File -Recurse | ForEach-Object { $_.FullName }
$allFiles += Get-ChildItem -LiteralPath $reposRoot -File -Recurse | ForEach-Object { $_.FullName }
$sumLines = foreach ($f in $allFiles) {
    $rel = $f.Substring($bundleRoot.Length + 1).Replace('\', '/')
    '{0}  {1}' -f (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash, $rel
}
[System.IO.File]::WriteAllLines((Join-Path $bundleRoot 'SHA256SUMS.txt'), $sumLines, (New-Object System.Text.UTF8Encoding($false)))

# 4. Create the archive ------------------------------------------------------
if (-not [System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir = Join-Path $repoRoot $OutputDir
}
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$archivePath = Join-Path $OutputDir "$targetName.tar.gz"
Push-Location $bundleDir
try {
    & $tarExe -czf $archivePath $targetName
} finally {
    Pop-Location
}
if ($LASTEXITCODE -ne 0) { throw "Failed to create archive: $archivePath" }

$archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
Write-Host ''
Write-Host "Archive: $archivePath"
Write-Host "Archive SHA-256: $archiveHash"

$report = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    target = $manifest.target
    archive = $archivePath
    archiveSha256 = $archiveHash
    sources = @($manifest.sources | ForEach-Object { "$($_.name)-$($_.version)" })
    pendingSources = $pending
    repositories = @($manifest.repos | ForEach-Object { "$($_.name)@$($_.pin)" })
} | ConvertTo-Json -Depth 4
$reportPath = "$archivePath.json"
[System.IO.File]::WriteAllText($reportPath, $report + "`n", (New-Object System.Text.UTF8Encoding($false)))
Write-Host "Report: $reportPath"

if (-not $KeepStaging) {
    Remove-Item -LiteralPath $bundleRoot -Recurse -Force
}
