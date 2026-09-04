param([Parameter(Mandatory=$true)][string]$ArtifactRoot)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$root=Join-Path $projectRoot ('phase4-results\diagnostic-package-test-' + [Guid]::NewGuid().ToString('N'))
$prepare=Join-Path $PSScriptRoot 'prepare-installer-diagnostic-package.ps1'
$checks=[Collections.Generic.List[string]]::new()
function Assert-Check($condition,[string]$name) { if (-not $condition) { throw $name }; $checks.Add($name) }
function Expect-Reject([scriptblock]$action,[string]$message) {
    $rejected=$false
    try { & $action | Out-Null } catch { if ($_.Exception.Message -notlike "*$message*") { throw }; $rejected=$true }
    Assert-Check $rejected "Rejected: $message"
}
$happy=Join-Path $root 'happy'
& $prepare -ArtifactRoot $ArtifactRoot -OutputRoot $happy | Out-Null
$report=Get-Content (Join-Path $happy 'package-report.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Check ($report.diagnosticOnly -and -not $report.installationEnabled -and -not $report.publicationPerformed) 'Clearly marked read-only with no publication'
$expanded=Join-Path $root 'expanded'
Expand-Archive -LiteralPath $report.zipPath -DestinationPath $expanded
Assert-Check (@(Get-ChildItem -LiteralPath $expanded -File).Count -eq 9) 'Exact allowlisted nine-file package'
foreach ($line in Get-Content (Join-Path $expanded 'SHA256SUMS.txt')) {
    if ($line -notmatch '^([A-F0-9]{64})  ([^/\\]+)$') { throw 'Invalid checksum entry.' }
    $hash=$Matches[1]; $name=$Matches[2]
    Assert-Check ((Get-FileHash -LiteralPath (Join-Path $expanded $name)).Hash -eq $hash) "ZIP checksum: $name"
}
Expect-Reject { & $prepare -ArtifactRoot $ArtifactRoot -OutputRoot $happy } 'refusing overwrite'
$fixture=Join-Path $root 'fixture'
Copy-Item -LiteralPath (Resolve-Path $ArtifactRoot).Path -Destination $fixture -Recurse
$exe=Get-ChildItem -LiteralPath $fixture -Recurse -Filter '*.exe' -File
$original=[IO.File]::ReadAllBytes($exe.FullName)
$damaged=[byte[]]$original.Clone(); $damaged[$damaged.Length-1]=$damaged[$damaged.Length-1] -bxor 1
[IO.File]::WriteAllBytes($exe.FullName,$damaged)
$rejectedOutput=Join-Path $root 'must-not-exist'
Expect-Reject { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } 'does not match pinned CI bytes'
[IO.File]::WriteAllBytes($exe.FullName,$original)
$fixtureReport=Join-Path $fixture '_temp\installer-diagnostic-report.json'
$originalReport=[IO.File]::ReadAllText($fixtureReport)
foreach ($mutation in @('passed','count','registry','hash','clicks','boundary','maintenance','data')) {
    $ci=$originalReport | ConvertFrom-Json
    switch ($mutation) {
        'passed' { $ci.passed=$false }
        'count' { $ci.scenarios=@() }
        'registry' { $ci.scenarios[0].registryUnchanged=$false }
        'hash' { $ci.installerSha256='BAD' }
        'clicks' { $ci.scenarios[0].clicks=0 }
        'boundary' { $ci.scenarios[0].decisions=@('decision=no-existing-install') }
        'maintenance' { ($ci.scenarios | Where-Object scenario -eq 'old-valid').decisions=@('action=install-blocked; section=EarlyChecks') }
        'data' { $ci.scenarios[0].dataUnchanged=$false }
    }
    [IO.File]::WriteAllText($fixtureReport,($ci|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    $expected=if ($mutation -in @('registry','data')) {'Incomplete safety case'} elseif ($mutation -in @('clicks','boundary','maintenance')) {'Missing actual dialog/boundary evidence'} else {'Missing complete diagnostic CI evidence'}
    Expect-Reject { & $prepare -ArtifactRoot $fixture -OutputRoot $rejectedOutput } $expected
}
Assert-Check (-not (Test-Path -LiteralPath $rejectedOutput)) 'Rejected inputs never create output'
$result=[ordered]@{scope='Diagnostic packaging regression; synthetic mutations are not desktop evidence';passed=$true;checks=@($checks.ToArray());checkCount=$checks.Count}
[IO.File]::WriteAllText((Join-Path $root 'regression-report.json'),($result|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
Write-Output "PASS: $($checks.Count) diagnostic package checks. $root"
