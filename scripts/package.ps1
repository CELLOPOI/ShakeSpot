$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$publishRoot = Join-Path $projectRoot 'artifacts/publish/win-x64'
$exePath = Join-Path $publishRoot 'ShakeSpot.exe'
if (-not (Test-Path -LiteralPath $exePath)) { throw 'Publish ShakeSpot before packaging.' }
$runtimeConfig = Get-Content -LiteralPath (Join-Path $projectRoot 'src/ShakeSpot/bin/Release/net10.0-windows/win-x64/ShakeSpot.runtimeconfig.json') -Raw | ConvertFrom-Json
foreach ($framework in $runtimeConfig.runtimeOptions.includedFrameworks) {
    $packageName = $framework.name.ToLowerInvariant() + '.runtime.win-x64'
    $packageRoot = Join-Path $projectRoot ('.cache/packages/' + $packageName + '/' + $framework.version)
    $noticeFiles = Get-ChildItem -LiteralPath $packageRoot -File | Where-Object Name -Match 'LICENSE|NOTICE'
    if (-not $noticeFiles) { throw "Runtime license files not found: $packageRoot" }
    foreach ($notice in $noticeFiles) {
        Copy-Item -LiteralPath $notice.FullName -Destination (Join-Path $publishRoot ($framework.name + '-' + $notice.Name)) -Force
    }
}
foreach ($document in @('VALIDATION.md', 'THIRD_PARTY_NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot $document) -Destination $publishRoot -Force
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'LEGACY.md') -Destination (Join-Path $publishRoot 'README.md') -Force
$licensesRoot = Join-Path $publishRoot 'licenses'
New-Item -ItemType Directory -Path $licensesRoot -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'licenses/Microsoft-STL-LICENSE.txt') -Destination $licensesRoot -Force
$zipPath = Join-Path $projectRoot 'artifacts/ShakeSpot-0.1.0-win-x64.zip'
Compress-Archive -Path (Join-Path $publishRoot '*') -DestinationPath $zipPath -Force
Get-Item -LiteralPath $exePath, $zipPath | Select-Object FullName, Length
