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

    $nodeExe = Join-Path $bundleRoot 'node\node.exe'
    $npmCli = Join-Path $bundleRoot 'node\node_modules\npm\bin\npm-cli.js'
    $npmArguments = @(
        $npmCli, 'install', '--omit=dev', '--no-audit', '--no-fund', '--save-exact',
        '--registry=https://registry.npmjs.org/'
    )
    if ($recipe.legacyPeerDeps) {
        $npmArguments += '--legacy-peer-deps'
    }
    & $nodeExe @npmArguments --prefix $appRoot
    if ($LASTEXITCODE -ne 0) {
        throw "npm install failed with exit code $LASTEXITCODE."
    }

    $lock = Get-Content -Raw -LiteralPath (Join-Path $appRoot 'package-lock.json') -Encoding UTF8 | ConvertFrom-Json
    $lockedIntegrity = $lock.packages.'node_modules/@deepseek-ai/dsh'.integrity
    if ($lockedIntegrity -ne $recipe.packageIntegrity) {
        throw 'Installed DSH integrity does not match the pinned compatibility recipe.'
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
