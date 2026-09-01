param(
    [Parameter(Mandatory = $true)]
    [string]$DshVersion,

    [ValidateSet('recommended', 'alpha')]
    [string]$Channel = 'recommended',

    [string]$OutputDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) 'release-assets')
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$recipePath = Join-Path $projectRoot 'src-tauri\resources\compatibility-recipes.json'
$recipeDocument = Get-Content -Raw -LiteralPath $recipePath -Encoding UTF8 | ConvertFrom-Json
$recipe = $recipeDocument.recipes | Where-Object { $_.dshVersion -eq $DshVersion } | Select-Object -First 1
if (-not $recipe) {
    throw "No compatibility recipe is pinned for DSH $DshVersion."
}
$runtimeLockTemplate = Join-Path $projectRoot "src-tauri\resources\runtime-locks\$DshVersion.package-lock.json"
if (-not (Test-Path -LiteralPath $runtimeLockTemplate -PathType Leaf)) {
    throw "No reviewed full dependency lock is available for DSH ${DshVersion}: $runtimeLockTemplate"
}

$buildBase = if (Test-Path -LiteralPath 'D:\') {
    'D:\Caches\dsh-launcher\release-build'
} elseif ($env:RUNNER_TEMP) {
    Join-Path $env:RUNNER_TEMP 'dsh-launcher-release-build'
} else {
    Join-Path $env:LOCALAPPDATA 'DSH Launcher\release-build'
}
New-Item -ItemType Directory -Force -Path $buildBase | Out-Null
$work = Join-Path $buildBase ([guid]::NewGuid().ToString('N'))
$bundleRoot = Join-Path $work 'bundle'
$appRoot = Join-Path $bundleRoot 'app'
$nodeExtract = Join-Path $work 'node-extract'
$nodeArchive = Join-Path $work 'node.zip'
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

try {
    New-Item -ItemType Directory -Force -Path $appRoot, $nodeExtract, $OutputDirectory | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri $recipe.nodeUrl -OutFile $nodeArchive
    $actualNodeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $nodeArchive).Hash.ToLowerInvariant()
    if ($actualNodeHash -ne $recipe.nodeSha256.ToLowerInvariant()) {
        throw "Node archive SHA-256 mismatch. Expected $($recipe.nodeSha256), got $actualNodeHash."
    }
    Expand-Archive -LiteralPath $nodeArchive -DestinationPath $nodeExtract
    $nodeSource = Get-ChildItem -LiteralPath $nodeExtract -Directory | Select-Object -First 1
    if (-not $nodeSource -or -not (Test-Path -LiteralPath (Join-Path $nodeSource.FullName 'node.exe'))) {
        throw 'Node archive does not contain the expected Windows portable runtime.'
    }
    Move-Item -LiteralPath $nodeSource.FullName -Destination (Join-Path $bundleRoot 'node')

    $dependencies = [ordered]@{ '@deepseek-ai/dsh' = $DshVersion }
    foreach ($property in $recipe.supplementalDependencies.PSObject.Properties) {
        $dependencies[$property.Name] = [string]$property.Value
    }
    $packageJson = [ordered]@{
        name = 'dsh-launcher-runtime-bundle'
        private = $true
        version = '1.0.0'
        dependencies = $dependencies
    } | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText((Join-Path $appRoot 'package.json'), $packageJson + "`n", $utf8NoBom)

    Copy-Item -LiteralPath $runtimeLockTemplate -Destination (Join-Path $appRoot 'package-lock.json')

    $nodeExe = Join-Path $bundleRoot 'node\node.exe'
    $npmCli = Join-Path $bundleRoot 'node\node_modules\npm\bin\npm-cli.js'
    $npmArguments = @(
        $npmCli, 'ci', '--omit=dev', '--no-audit', '--no-fund',
        '--registry=https://registry.npmjs.org/'
    )
    if ($recipe.legacyPeerDeps) {
        $npmArguments += '--legacy-peer-deps'
    }
    & $nodeExe @npmArguments --prefix $appRoot
    if ($LASTEXITCODE -ne 0) {
        throw "npm ci failed with exit code $LASTEXITCODE."
    }

    $lockPath = Join-Path $appRoot 'package-lock.json'
    $sourceLockHash = (Get-FileHash -LiteralPath $runtimeLockTemplate -Algorithm SHA256).Hash
    $installedLockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash
    if ($installedLockHash -ne $sourceLockHash) {
        throw 'npm ci changed the reviewed Runtime dependency lock.'
    }
    $lockedIntegrity = (& $nodeExe -e "const fs=require('fs');const lock=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));process.stdout.write(lock.packages?.['node_modules/@deepseek-ai/dsh']?.integrity ?? '')" $lockPath).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($lockedIntegrity)) {
        throw 'Unable to read the installed DSH integrity from package-lock.json.'
    }
    if ($lockedIntegrity -ne $recipe.packageIntegrity) {
        throw 'Installed DSH integrity does not match the pinned compatibility recipe.'
    }
    & $nodeExe -e "const fs=require('fs');const lock=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));const expected=process.argv[2];const drift=Object.entries(lock.packages||{}).map(([p,m])=>{const marker='node_modules/';const i=p.lastIndexOf(marker);return {p,m,name:m.name||(i>=0?p.slice(i+marker.length):p)}}).filter(x=>x.name.startsWith('@deepseek-ai/dsh-')&&x.m.version!==expected).map(x=>x.name+'@'+x.m.version);if(drift.length){console.error(drift.join('\n'));process.exit(2)}" $lockPath $DshVersion
    if ($LASTEXITCODE -ne 0) {
        throw 'Reviewed Runtime lock contains drifting internal DSH package versions.'
    }
    $dshEntry = Join-Path $appRoot 'node_modules\@deepseek-ai\dsh\lib\bin.js'
    $reportedVersion = (& $nodeExe $dshEntry --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $reportedVersion -ne $DshVersion) {
        throw "dsh --version validation failed: $reportedVersion"
    }
    Push-Location $appRoot
    try {
        & $nodeExe -e "const sharp=require('sharp');const pty=require('node-pty');if(!sharp||typeof pty.spawn!=='function')process.exit(2)"
    } finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) {
        throw 'Native dependency validation failed.'
    }

    $licenseRoot = Join-Path $appRoot 'licenses'
    New-Item -ItemType Directory -Force -Path $licenseRoot | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri 'https://www.gnu.org/licenses/gpl-3.0.txt' -OutFile (Join-Path $licenseRoot 'GPL-3.0.txt')
    Invoke-WebRequest -UseBasicParsing -Uri 'https://www.gnu.org/licenses/lgpl-3.0.txt' -OutFile (Join-Path $licenseRoot 'LGPL-3.0.txt')
    $sharpPackagePath = Join-Path $appRoot 'node_modules\@img\sharp-win32-x64\package.json'
    if (-not (Test-Path -LiteralPath $sharpPackagePath -PathType Leaf)) {
        throw 'The Windows x64 sharp package is missing from the Runtime build.'
    }
    $sharpPackage = Get-Content -LiteralPath $sharpPackagePath -Raw -Encoding UTF8 | ConvertFrom-Json
    $sharpLibvipsVersion = (& $nodeExe -e "const fs=require('fs');const lock=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));const versions=[...new Set(Object.entries(lock.packages||{}).filter(([p])=>p.includes('node_modules/@img/sharp-libvips-')).map(([,m])=>m.version))];if(versions.length!==1)process.exit(2);process.stdout.write(versions[0])" $lockPath).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($sharpLibvipsVersion)) {
        throw 'Unable to determine the exact sharp-libvips packaging source version.'
    }
    $runtimeLicenseTemplate = Get-Content -LiteralPath (Join-Path $projectRoot 'docs\RUNTIME_LICENSES.md') -Raw -Encoding UTF8
    $runtimeLicenseText = $runtimeLicenseTemplate.Replace('{{SHARP_VERSION}}', [string]$sharpPackage.version).Replace('{{SHARP_LIBVIPS_VERSION}}', $sharpLibvipsVersion)
    [System.IO.File]::WriteAllText((Join-Path $appRoot 'RUNTIME_LICENSES.md'), $runtimeLicenseText, $utf8NoBom)

    $auditPointer = [ordered]@{
        id = "release-$DshVersion"
        dshVersion = $DshVersion
        nodeVersion = [string]$recipe.nodeVersion
        nodePath = $nodeExe
    } | ConvertTo-Json -Depth 5
    $auditPointerPath = Join-Path $work 'runtime-audit-pointer.json'
    [System.IO.File]::WriteAllText($auditPointerPath, $auditPointer + "`n", $utf8NoBom)
    $licenseReportPath = Join-Path $OutputDirectory "dsh-$DshVersion-windows-x86_64.license-audit.json"
    & $nodeExe (Join-Path $projectRoot 'scripts\license-audit.mjs') `
        --project-root $projectRoot `
        --runtime-pointer $auditPointerPath `
        --require-runtime `
        --runtime-only `
        --report $licenseReportPath `
        --notice (Join-Path $appRoot 'THIRD_PARTY_NOTICES.md')
    if ($LASTEXITCODE -ne 0) {
        throw "Runtime license inventory failed with exit code $LASTEXITCODE."
    }

    $metadata = [ordered]@{
        schemaVersion = 1
        dshVersion = $DshVersion
        nodeVersion = [string]$recipe.nodeVersion
        architecture = 'windows-x86_64'
        channel = $Channel
        packageIntegrity = [string]$recipe.packageIntegrity
        recipeId = [string]$recipe.id
    } | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText((Join-Path $bundleRoot 'runtime-bundle.json'), $metadata + "`n", $utf8NoBom)
    $metadataOutput = Join-Path $OutputDirectory "dsh-$DshVersion-windows-x86_64.runtime-bundle.json"
    [System.IO.File]::WriteAllText($metadataOutput, $metadata + "`n", $utf8NoBom)

    $bundlePath = Join-Path $OutputDirectory "dsh-$DshVersion-windows-x86_64.runtime.zip"
    if (Test-Path -LiteralPath $bundlePath) {
        Remove-Item -LiteralPath $bundlePath -Force
    }
    Compress-Archive -Path (Join-Path $bundleRoot '*') -DestinationPath $bundlePath -CompressionLevel Optimal
    Write-Output $bundlePath
} finally {
    $resolvedBase = (Resolve-Path -LiteralPath $buildBase).Path
    if (Test-Path -LiteralPath $work) {
        $resolvedWork = (Resolve-Path -LiteralPath $work).Path
        if (-not $resolvedWork.StartsWith($resolvedBase + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing cleanup outside release build directory: $resolvedWork"
        }
        Remove-Item -LiteralPath $resolvedWork -Recurse -Force
    }
}
