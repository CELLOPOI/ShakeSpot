param(
    [ValidateSet('build', 'test', 'run', 'publish', 'desktop-checks')]
    [string]$Task = 'build',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$AppArguments = @()
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$dotnetExe = Join-Path $projectRoot '.tools/dotnet/dotnet.exe'
if (-not (Test-Path -LiteralPath $dotnetExe)) { $dotnetExe = (Get-Command dotnet -ErrorAction Stop).Source }
$env:DOTNET_CLI_HOME = Join-Path $projectRoot '.cache/dotnet-home'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$env:DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
$env:NUGET_HTTP_CACHE_PATH = Join-Path $projectRoot '.cache/nuget-http'
Push-Location -LiteralPath $projectRoot
try {
    switch ($Task) {
        'build' { & $dotnetExe build ShakeSpot.slnx -c Release --nologo }
        'test' { & $dotnetExe run --project tests/ShakeSpot.Tests -c Release }
        'run' { & $dotnetExe run --project src/ShakeSpot -c Release -- @AppArguments }
        'publish' {
            & $dotnetExe publish src/ShakeSpot/ShakeSpot.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:EnableCompressionInSingleFile=false -o artifacts/publish/win-x64 --nologo
            if ($LASTEXITCODE -eq 0) { & (Join-Path $PSScriptRoot 'package.ps1') }
        }
        'desktop-checks' { & $dotnetExe run --project tests/ShakeSpot.DesktopChecks -c Release -- @AppArguments }
    }
    $result = $LASTEXITCODE
}
finally { Pop-Location }
exit $result
