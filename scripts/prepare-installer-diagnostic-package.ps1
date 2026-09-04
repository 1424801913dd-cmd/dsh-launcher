param([Parameter(Mandatory=$true)][string]$ArtifactRoot,[Parameter(Mandatory=$true)][string]$OutputRoot)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$metadataPath = Join-Path $PSScriptRoot 'data\installer-diagnostic.json'
$metadata = Get-Content -LiteralPath $metadataPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($metadata.diagnosticOnly -ne $true -or $metadata.installationEnabled -ne $false -or
    $metadata.diagnosticId -notmatch '^0\.4\.1-installer-diagnostic-ci[0-9]+$' -or
    $metadata.sourceCommit -notmatch '^[a-f0-9]{40}$') { throw 'Invalid diagnostic identity.' }
$output = [IO.Path]::GetFullPath($OutputRoot)
if (Test-Path -LiteralPath $output) { throw 'Output already exists; refusing overwrite.' }
$artifact = (Resolve-Path -LiteralPath $ArtifactRoot).Path
$executables = @(Get-ChildItem -LiteralPath $artifact -Recurse -File -Filter '*.exe')
if ($executables.Count -ne 1 -or $executables[0].Length -ne $metadata.exeBytes -or
    (Get-FileHash -LiteralPath $executables[0].FullName).Hash -ne $metadata.exeSha256) { throw 'Diagnostic EXE does not match pinned CI bytes.' }
if ((Get-AuthenticodeSignature -LiteralPath $executables[0].FullName).Status -ne 'NotSigned') { throw 'Unexpected diagnostic signature status.' }
$ciReportPath = Join-Path $artifact '_temp\installer-diagnostic-report.json'
$ci = Get-Content -LiteralPath $ciReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($ci.passed -ne $true -or $ci.installerSha256 -ne $metadata.exeSha256 -or $ci.scenarios.Count -ne 6) { throw 'Missing complete diagnostic CI evidence.' }
foreach ($name in @('clean','old-valid','orphan-default','missing-uninstaller','silent','passive')) {
    $cases = @($ci.scenarios | Where-Object scenario -eq $name)
    if ($cases.Count -ne 1 -or $cases[0].passed -ne $true -or $cases[0].registryUnchanged -ne $true -or
        $cases[0].dataUnchanged -ne $true -or $cases[0].defaultApplicationNotCreated -ne $true) { throw "Incomplete safety case: $name" }
    $case = $cases[0]
    $decisions = @($case.decisions) -join "`n"
    if ($decisions -notmatch '(?m)^action=(install|uninstall)-blocked(?:;|$)' -or
        ($name -notin @('silent','passive') -and $case.clicks -lt 1) -or
        ($name -eq 'clean' -and $decisions -notmatch '(?m)^decision=no-existing-install$') -or
        ($name -in @('old-valid','missing-uninstaller') -and
            ($decisions -notmatch '(?m)^decision=maintenance-page;' -or $decisions -notmatch '(?m)^action=uninstall-blocked;'))) {
        throw "Missing actual dialog/boundary evidence: $name"
    }
}
$sources = [ordered]@{
    'DSH-Launcher-INSTALLER-DIAGNOSTIC.exe' = $executables[0].FullName
    'READ-ME-FIRST.md' = Join-Path $projectRoot 'docs\INSTALLER_DIAGNOSTIC_README.md'
    'CODEX-HANDOFF.md' = Join-Path $projectRoot 'docs\INSTALLER_DIAGNOSTIC_HANDOFF.md'
    'FEEDBACK.md' = Join-Path $projectRoot 'docs\INSTALLER_DIAGNOSTIC_FEEDBACK.md'
    'candidate.json' = $metadataPath
    'ci-diagnostic.json' = $ciReportPath
    'LICENSE-TAURI-MIT.txt' = Join-Path $projectRoot 'src-tauri\installer-diagnostics\LICENSE_MIT'
    'LICENSE-PROJECT.txt' = Join-Path $projectRoot 'LICENSE'
}
foreach ($source in $sources.Values) { if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing input: $source" } }
$contents = Join-Path $output 'contents'
New-Item -ItemType Directory -Path $contents -Force | Out-Null
foreach ($name in $sources.Keys) { Copy-Item -LiteralPath $sources[$name] -Destination (Join-Path $contents $name) }
$hashes = @(Get-ChildItem -LiteralPath $contents -File | Sort-Object Name | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName).Hash,$_.Name })
[IO.File]::WriteAllText((Join-Path $contents 'SHA256SUMS.txt'),($hashes -join "`n")+"`n",[Text.UTF8Encoding]::new($false))
$zip = Join-Path $output ("DSH-Launcher-{0}.zip" -f $metadata.diagnosticId)
Compress-Archive -Path (Join-Path $contents '*') -DestinationPath $zip -CompressionLevel Optimal
$report = [ordered]@{schemaVersion=1; diagnosticOnly=$true; installationEnabled=$false; diagnosticId=$metadata.diagnosticId;
    sourceCommit=$metadata.sourceCommit; ciRunUrl=$metadata.ciRunUrl; exeSha256=$metadata.exeSha256;
    zipPath=$zip; zipBytes=(Get-Item -LiteralPath $zip).Length; zipSha256=(Get-FileHash -LiteralPath $zip).Hash;
    fileCount=$hashes.Count+1; ciEvidenceVerified=$true; publicationPerformed=$false; rootCauseConfirmed=$false}
[IO.File]::WriteAllText((Join-Path $output 'package-report.json'),($report|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
$report | ConvertTo-Json -Depth 4
