# Sign Windows programs with Azure Trusted Signing: signtool with Microsoft's Trusted Signing
# dlib, an RFC 3161 timestamp (the certificate lives for days; the timestamp keeps the signature
# valid after it), SHA-256.
#
#   sign.ps1 <file.exe> [<file.exe> ...]
#
# scripts/package-windows.ps1 calls this with -Sign, which .github/workflows/windows.yml passes for
# a release. This repository is public: the account's values come from the environment only, and
# none of them may be written to a file that is kept or to the log.
#
# Needs, in the environment:
#   AZURE_SIGNING_ACCOUNT    the Trusted Signing account
#   AZURE_SIGNING_PROFILE    its certificate profile
#   AZURE_SIGNING_ENDPOINT   the account's region endpoint (default https://eus.codesigning.azure.net)
#   AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET   the app registration that may sign
# and signtool.exe from the Windows SDK. The dlib is fetched from NuGet into the temporary folder
# unless AZURE_SIGNING_DLIB names it.
param([Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)][string[]]$Files)
$ErrorActionPreference = "Stop"

foreach ($v in "AZURE_SIGNING_ACCOUNT", "AZURE_SIGNING_PROFILE", "AZURE_TENANT_ID", "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET") {
    if (-not [Environment]::GetEnvironmentVariable($v)) { throw "$v is not set: cannot sign." }
}
$endpoint = if ($env:AZURE_SIGNING_ENDPOINT) { $env:AZURE_SIGNING_ENDPOINT } else { "https://eus.codesigning.azure.net" }
$tmp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $signtool) { throw "signtool.exe (Windows SDK) not found." }

$dlib = $env:AZURE_SIGNING_DLIB
if (-not $dlib) {
    # This runs beside the signing secret: exactly this build, and no other.
    $version = "1.0.95"
    $sha256 = "3bfcf1e0a3cb42af1692f0a8ed45c15de070c2de86f28a59b2795d904d8a920f"
    $pkg = Join-Path $tmp "trusted-signing-client"
    if (-not (Test-Path $pkg)) {
        $nupkg = Join-Path $tmp "trusted-signing-client.zip"
        Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.trusted.signing.client/$version/microsoft.trusted.signing.client.$version.nupkg" -OutFile $nupkg
        if ((Get-FileHash -Algorithm SHA256 $nupkg).Hash -ne $sha256) { throw "The signing client is not the build this script names." }
        Expand-Archive $nupkg $pkg
    }
    $dlib = Join-Path $pkg "bin\x64\Azure.CodeSigning.Dlib.dll"
}
$metadata = Join-Path $tmp "trusted-signing-metadata.json"
@{ Endpoint = $endpoint; CodeSigningAccountName = $env:AZURE_SIGNING_ACCOUNT; CertificateProfileName = $env:AZURE_SIGNING_PROFILE } |
    ConvertTo-Json | Set-Content -Encoding utf8 $metadata

& $signtool sign /v /fd SHA256 /tr "http://timestamp.acs.microsoft.com" /td SHA256 /dlib $dlib /dmdf $metadata @Files
if ($LASTEXITCODE -ne 0) { throw "signtool failed ($LASTEXITCODE)" }
foreach ($f in $Files) {
    $sig = Get-AuthenticodeSignature $f
    if ($sig.Status -ne "Valid" -or -not $sig.TimeStamperCertificate) { throw "$f is not signed and timestamped ($($sig.Status))" }
    Write-Host "signed: $f ($($sig.SignerCertificate.Subject))"
}
