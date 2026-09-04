param(
    [switch]$RunProjectChecks,
    [switch]$RequireAuthenticode,
    [switch]$RequireInstaller,
    [switch]$RequireLicenseReview,
    [string]$ReportPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Initialize-DevEnvironment.ps1')

$results = [System.Collections.Generic.List[object]]::new()

function Add-QualityResult {
    param(
        [string]$Check,
        [ValidateSet('PASS', 'WARN', 'FAIL')]
        [string]$Status,
        [string]$Details
    )
    $results.Add([pscustomobject]@{
        Check = $Check
        Status = $Status
        Details = $Details
    })
}

if ($RunProjectChecks) {
    & (Join-Path $PSScriptRoot 'check.ps1')
    if ($LASTEXITCODE -ne 0) {
        Add-QualityResult -Check 'project-checks' -Status 'FAIL' -Details "scripts/check.ps1 exited with $LASTEXITCODE"
    } else {
        Add-QualityResult -Check 'project-checks' -Status 'PASS' -Details 'Frontend, formatting, and Rust tests passed.'
    }
}

foreach ($relativePath in @('LICENSE', 'README.md', 'HANDOFF.md', 'docs\CODE_SIGNING_POLICY.md', 'docs\LGPL_REVIEW.md', 'docs\PRIVACY.md', 'docs\RELEASE_REVIEW.md', 'docs\RELEASE_TEMPLATE.md', 'docs\RUNTIME_LICENSES.md', 'docs\SIGNPATH_APPLICATION.md', 'docs\THIRD_PARTY_NOTICES.md', 'docs\USER_GUIDE.md', 'docs\WINDOWS_ACCEPTANCE.md', 'docs\TRIAL_GUIDE.md', 'docs\TRIAL_RELEASE_NOTES.md', 'docs\TRIAL_FEEDBACK.md', 'docs\TEST_MACHINE_HANDOFF.md')) {
    $path = Join-Path $ProjectRoot $relativePath
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        Add-QualityResult -Check "document:$relativePath" -Status 'PASS' -Details 'Present.'
    } else {
        Add-QualityResult -Check "document:$relativePath" -Status 'FAIL' -Details 'Missing required release document.'
    }
}

foreach ($relativePath in @('scripts\clean-runtime-install.ps1', 'scripts\collect-windows-acceptance.ps1', 'scripts\test-installer.ps1', 'scripts\test-launcher-startup.ps1', 'scripts\prepare-trial-package.ps1', 'scripts\test-trial-package.ps1', 'scripts\inspect-installed-launcher.ps1', 'scripts\launcher-uninstall-registry.ps1', 'scripts\test-installer-registration.ps1')) {
    $path = Join-Path $ProjectRoot $relativePath
    $tokens = $null
    $parseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors.Count -eq 0) {
        Add-QualityResult -Check "powershell-syntax:$relativePath" -Status 'PASS' -Details 'Parsed without errors.'
    } else {
        Add-QualityResult -Check "powershell-syntax:$relativePath" -Status 'FAIL' -Details ($parseErrors.Message -join '; ')
    }
}

$readmeText = Get-Content -LiteralPath (Join-Path $ProjectRoot 'README.md') -Raw -Encoding UTF8
$codeSigningPolicyText = Get-Content -LiteralPath (Join-Path $ProjectRoot 'docs\CODE_SIGNING_POLICY.md') -Raw -Encoding UTF8
$releaseTemplateText = Get-Content -LiteralPath (Join-Path $ProjectRoot 'docs\RELEASE_TEMPLATE.md') -Raw -Encoding UTF8
$signPathAttribution = 'Free code signing provided by'
$signPathCertificateAttribution = 'certificate by'
$signPathFoundationLink = '[SignPath Foundation](https://signpath.org/)'
if ($readmeText.Contains('Code signing policy') -and
    $readmeText.Contains($signPathAttribution) -and
    $readmeText.Contains($signPathCertificateAttribution) -and
    $readmeText.Contains($signPathFoundationLink) -and
    $codeSigningPolicyText.Contains($signPathAttribution) -and
    $codeSigningPolicyText.Contains($signPathCertificateAttribution) -and
    $codeSigningPolicyText.Contains($signPathFoundationLink) -and
    $releaseTemplateText.Contains($signPathAttribution) -and
    $releaseTemplateText.Contains($signPathCertificateAttribution) -and
    $releaseTemplateText.Contains($signPathFoundationLink)) {
    Add-QualityResult -Check 'signpath-policy-copy' -Status 'PASS' -Details 'SignPath attribution and code signing policy are present on project and release pages.'
} else {
    Add-QualityResult -Check 'signpath-policy-copy' -Status 'FAIL' -Details 'Required SignPath policy or attribution is missing.'
}

$releaseReviewText = Get-Content -LiteralPath (Join-Path $ProjectRoot 'docs\RELEASE_REVIEW.md') -Raw -Encoding UTF8
if ($readmeText.Contains('unofficial, independently developed') -and
    $readmeText.Contains('not reviewed, sponsored, or endorsed by DeepSeek') -and
    $releaseReviewText.Contains('Unofficial and independently maintained')) {
    Add-QualityResult -Check 'brand-identity-copy' -Status 'PASS' -Details 'Unofficial, independent, and no-endorsement language is present.'
} else {
    Add-QualityResult -Check 'brand-identity-copy' -Status 'FAIL' -Details 'Required unofficial/no-endorsement copy is missing.'
}

$logoText = Get-Content -LiteralPath (Join-Path $ProjectRoot 'public\dsh-logo.svg') -Raw -Encoding UTF8
if ($logoText.Contains('DSH Launcher original terminal mark')) {
    Add-QualityResult -Check 'brand-logo-source' -Status 'PASS' -Details 'Project-owned terminal mark source is present.'
} else {
    Add-QualityResult -Check 'brand-logo-source' -Status 'FAIL' -Details 'Expected project-owned logo source marker is missing.'
}

$recipeDocument = Get-Content -LiteralPath (Join-Path $ProjectRoot 'src-tauri\resources\compatibility-recipes.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$runtimeLocksValid = $true
$runtimeLockDetails = [System.Collections.Generic.List[string]]::new()
foreach ($recipe in $recipeDocument.recipes) {
    $lockPath = Join-Path $ProjectRoot "src-tauri\resources\runtime-locks\$($recipe.dshVersion).package-lock.json"
    if (-not (Test-Path -LiteralPath $lockPath -PathType Leaf)) {
        $runtimeLocksValid = $false
        $runtimeLockDetails.Add("missing $($recipe.dshVersion)")
        continue
    }
    node (Join-Path $PSScriptRoot 'prepare-runtime-lock.mjs') --project-root $ProjectRoot --dsh-version $recipe.dshVersion --check-lock $lockPath
    if ($LASTEXITCODE -ne 0) {
        $runtimeLocksValid = $false
        $runtimeLockDetails.Add("invalid $($recipe.dshVersion)")
    } else {
        $runtimeLockDetails.Add("valid $($recipe.dshVersion)")
    }
}
Add-QualityResult -Check 'runtime-full-dependency-locks' -Status $(if ($runtimeLocksValid) { 'PASS' } else { 'FAIL' }) -Details ($runtimeLockDetails -join '; ')

$licenseReportPath = Join-Path $ProjectRoot 'phase4-results\license-audit.latest.json'
$licenseArguments = @(
    (Join-Path $PSScriptRoot 'license-audit.mjs'),
    '--project-root', $ProjectRoot,
    '--report', $licenseReportPath,
    '--check-notice', (Join-Path $ProjectRoot 'docs\THIRD_PARTY_NOTICES.md')
)
$runtimePointer = 'D:\Tools\dsh-launcher\active.json'
if (Test-Path -LiteralPath $runtimePointer -PathType Leaf) {
    $licenseArguments += @('--runtime-pointer', $runtimePointer, '--require-runtime')
}
node @licenseArguments
if ($LASTEXITCODE -ne 0) {
    Add-QualityResult -Check 'license-metadata' -Status 'FAIL' -Details "License audit exited with $LASTEXITCODE."
} else {
    $licenseReport = Get-Content -LiteralPath $licenseReportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Add-QualityResult -Check 'license-metadata' -Status 'PASS' -Details (
        "app=$($licenseReport.app.packageCount); cargo=$($licenseReport.cargo.packageCount); " +
        "runtime=$(if ($licenseReport.runtime) { $licenseReport.runtime.packageCount } else { 'not-audited' }); missing=0"
    )
    $reviewCount = @($licenseReport.unresolvedReviewRequired).Count
    $reviewStatus = if ($reviewCount -eq 0) { 'PASS' } elseif ($RequireLicenseReview) { 'FAIL' } else { 'WARN' }
    $reviewDetails = if ($reviewCount -eq 0) {
        'All detected review-required license families have a recorded disposition.'
    } else {
        "$reviewCount package(s) use review-required license families; see docs\THIRD_PARTY_NOTICES.md."
    }
    Add-QualityResult -Check 'license-obligations-review' -Status $reviewStatus -Details $reviewDetails
    if ($licenseReport.runtime) {
        $runtimeLicenseStatus = if ($licenseReport.runtime.nodeLicensePresent) { 'PASS' } else { 'FAIL' }
        Add-QualityResult -Check 'runtime-license-inventory' -Status $runtimeLicenseStatus -Details (
            "$($licenseReport.runtime.id); Node license present=$($licenseReport.runtime.nodeLicensePresent)"
        )
    } else {
        Add-QualityResult -Check 'runtime-license-inventory' -Status 'WARN' -Details 'No local active Runtime was available for dependency inventory.'
    }
}

$releaseExecutable = Join-Path $ProjectRoot 'src-tauri\target\release\dsh-launcher.exe'
if (Test-Path -LiteralPath $releaseExecutable -PathType Leaf) {
    Add-QualityResult -Check 'release-executable' -Status 'PASS' -Details $releaseExecutable
} else {
    Add-QualityResult -Check 'release-executable' -Status 'FAIL' -Details 'Build the Release executable first.'
}

$bundleRoot = Join-Path $ProjectRoot 'src-tauri\target\release\bundle'
$installers = @()
if (Test-Path -LiteralPath $bundleRoot -PathType Container) {
    $installers = @(Get-ChildItem -LiteralPath $bundleRoot -File -Recurse -Force | Where-Object {
        $_.Extension -in '.exe', '.msi'
    })
}
if ($installers.Count -gt 0) {
    Add-QualityResult -Check 'installer-artifacts' -Status 'PASS' -Details "$($installers.Count) installer artifact(s) found."
} else {
    $status = if ($RequireInstaller) { 'FAIL' } else { 'WARN' }
    Add-QualityResult -Check 'installer-artifacts' -Status $status -Details 'No local installer artifact found.'
}

$artifacts = @()
if (Test-Path -LiteralPath $releaseExecutable -PathType Leaf) {
    $artifacts += Get-Item -LiteralPath $releaseExecutable
}
$artifacts += $installers
foreach ($artifact in $artifacts) {
    $signature = Get-AuthenticodeSignature -LiteralPath $artifact.FullName
    $valid = $signature.Status -eq 'Valid'
    $status = if ($valid) { 'PASS' } elseif ($RequireAuthenticode) { 'FAIL' } else { 'WARN' }
    $signer = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { 'none' }
    Add-QualityResult -Check "authenticode:$($artifact.Name)" -Status $status -Details "$($signature.Status); signer=$signer"
}

$invalidArtifactCount = @($artifacts | Where-Object {
    (Get-AuthenticodeSignature -LiteralPath $_.FullName).Status -ne 'Valid'
}).Count
$allArtifactsSigned = $artifacts.Count -gt 0 -and $invalidArtifactCount -eq 0

$codeSigningCertificates = @()
foreach ($store in @('Cert:\CurrentUser\My', 'Cert:\LocalMachine\My')) {
    $codeSigningCertificates += @(Get-ChildItem -Path $store -CodeSigningCert -ErrorAction SilentlyContinue | Where-Object {
        $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date)
    })
}
if ($allArtifactsSigned) {
    Add-QualityResult -Check 'code-signing-certificate' -Status 'PASS' -Details 'All discovered artifacts have valid Authenticode signatures; the private key may be held by an external signing service.'
} elseif ($codeSigningCertificates.Count -gt 0) {
    Add-QualityResult -Check 'code-signing-certificate' -Status 'PASS' -Details "$($codeSigningCertificates.Count) usable certificate(s) found."
} else {
    $status = if ($RequireAuthenticode) { 'FAIL' } else { 'WARN' }
    Add-QualityResult -Check 'code-signing-certificate' -Status $status -Details 'No unexpired code-signing certificate with a private key was found.'
}

$results | Format-Table -AutoSize

if ($ReportPath) {
    $fullReportPath = [System.IO.Path]::GetFullPath((Join-Path $ProjectRoot $ReportPath))
    $reportParent = Split-Path -Parent $fullReportPath
    New-Item -ItemType Directory -Force -Path $reportParent | Out-Null
    $report = [ordered]@{
        schemaVersion = 1
        generatedAtUtc = [DateTime]::UtcNow.ToString('o')
        machine = $env:COMPUTERNAME
        osVersion = [Environment]::OSVersion.VersionString
        requireAuthenticode = [bool]$RequireAuthenticode
        requireInstaller = [bool]$RequireInstaller
        requireLicenseReview = [bool]$RequireLicenseReview
        results = $results
    } | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText($fullReportPath, $report + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
    Write-Output "Report: $fullReportPath"
}

if ($results.Status -contains 'FAIL') {
    exit 1
}
exit 0
