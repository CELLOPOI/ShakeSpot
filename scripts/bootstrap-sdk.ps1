param([string]$Version = '10.0.401')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$sdkRoot = Join-Path $projectRoot '.tools/dotnet'
$sdkExe = Join-Path $sdkRoot 'dotnet.exe'
if (Test-Path -LiteralPath (Join-Path $sdkRoot "sdk/$Version")) {
    Write-Output "SDK already available: $sdkExe"
    exit 0
}
$metadata = Invoke-RestMethod -Uri 'https://dotnetcli.blob.core.windows.net/dotnet/release-metadata/10.0/releases.json'
$sdk = $metadata.releases.sdks | Where-Object version -EQ $Version | Select-Object -First 1
if (-not $sdk) { throw "Official metadata does not contain SDK $Version." }
$asset = $sdk.files | Where-Object { $_.rid -eq 'win-x64' -and $_.name -like '*.zip' } | Select-Object -First 1
if (-not $asset) { throw 'Windows x64 SDK archive not found.' }
New-Item -ItemType Directory -Path $sdkRoot -Force | Out-Null
$archive = Join-Path $projectRoot ".tools/dotnet-sdk-$Version.zip"
Write-Output "Downloading official .NET SDK $Version (Windows x64)."
Invoke-WebRequest -Uri $asset.url -OutFile $archive
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA512).Hash -ne $asset.hash) {
    throw 'SDK archive SHA512 verification failed.'
}
Expand-Archive -LiteralPath $archive -DestinationPath $sdkRoot -Force
Remove-Item -LiteralPath $archive
& $sdkExe --version
if ($LASTEXITCODE -ne 0) { throw 'SDK startup failed.' }
