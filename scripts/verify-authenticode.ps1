param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [string]$ExpectedSigner = 'SignPath Foundation',
    [switch]$RequireTimestamp,
    [string]$ReportPath
)

$ErrorActionPreference = 'Stop'

$resolvedPath = (Resolve-Path -LiteralPath $Path).Path
$signature = Get-AuthenticodeSignature -LiteralPath $resolvedPath
$signerSubject = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { '' }
$timestampSubject = if ($signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Subject } else { '' }

if ($signature.Status -ne 'Valid') {
    throw "Authenticode validation failed for '$resolvedPath': $($signature.Status) $($signature.StatusMessage)"
}
if ($ExpectedSigner -and $signerSubject -notlike "*$ExpectedSigner*") {
    throw "Unexpected Authenticode signer for '$resolvedPath': '$signerSubject'"
}
if ($RequireTimestamp -and -not $signature.TimeStamperCertificate) {
    throw "No trusted Authenticode timestamp was found for '$resolvedPath'."
}

$result = [ordered]@{
    schemaVersion = 1
    path = $resolvedPath
    sha256 = (Get-FileHash -LiteralPath $resolvedPath -Algorithm SHA256).Hash
    status = $signature.Status.ToString()
    signerSubject = $signerSubject
    signerThumbprint = $signature.SignerCertificate.Thumbprint
    signerNotAfterUtc = $signature.SignerCertificate.NotAfter.ToUniversalTime().ToString('o')
    timestampSubject = $timestampSubject
    timestampThumbprint = if ($signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Thumbprint } else { $null }
}

if ($ReportPath) {
    $fullReportPath = [System.IO.Path]::GetFullPath($ReportPath)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $fullReportPath) | Out-Null
    $json = $result | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText($fullReportPath, $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
}

$result | Format-List
