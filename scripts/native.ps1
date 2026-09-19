param(
    [ValidateSet('build','test','run','publish','desktop-checks')][string]$Task = 'build',
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$AppArguments = @()
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$nativeRoot = Join-Path $projectRoot 'native'
$outputRoot = Join-Path $projectRoot 'artifacts/native'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsRoot) { throw 'Install Visual Studio Build Tools with Desktop development with C++ and Windows SDK.' }
$vcVersion = (Get-Content -LiteralPath (Join-Path $vsRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') -Raw).Trim()
$vcRoot = Join-Path $vsRoot "VC/Tools/MSVC/$vcVersion"
$sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10'
$sdkVersion = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'Lib') -Directory | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'um/x64/user32.lib') } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty Name
$env:INCLUDE = @("$vcRoot/include", "$sdkRoot/Include/$sdkVersion/ucrt", "$sdkRoot/Include/$sdkVersion/shared", "$sdkRoot/Include/$sdkVersion/um") -join ';'
$env:LIB = @("$vcRoot/lib/x64", "$sdkRoot/Lib/$sdkVersion/ucrt/x64", "$sdkRoot/Lib/$sdkVersion/um/x64") -join ';'
$compiler = Join-Path $vcRoot 'bin/Hostx64/x64/cl.exe'
$rc = Join-Path $sdkRoot "bin/$sdkVersion/x64/rc.exe"
$env:PATH = (Split-Path -Parent $compiler) + ';' + (Split-Path -Parent $rc) + ';' + $env:PATH
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
Push-Location -LiteralPath $nativeRoot
try {
    $common = @('/nologo','/std:c++20','/O2','/MT','/EHsc','/W4','/WX','/utf-8','/DUNICODE','/D_UNICODE','/DWINVER=0x0A00','/D_WIN32_WINNT=0x0A00','/permissive-')
    if ($Task -in @('build','test','run','publish','desktop-checks')) {
        & $rc /nologo "/fo$outputRoot/resources.res" resources.rc
        if ($LASTEXITCODE -ne 0) { throw 'Resource compilation failed.' }
        & $compiler @common "/Fo$outputRoot/main.obj" "/Fe$outputRoot/ShakeSpot.Native.exe" main.cpp "$outputRoot/resources.res" /link /SUBSYSTEM:WINDOWS /MANIFEST:EMBED "/MANIFESTINPUT:$nativeRoot/native.manifest" user32.lib gdi32.lib shell32.lib advapi32.lib ole32.lib comctl32.lib wtsapi32.lib psapi.lib
        if ($LASTEXITCODE -ne 0) { throw 'Native application build failed.' }
    }
    if ($Task -eq 'test' -or $Task -eq 'build') {
        & $compiler @common "/Fo$outputRoot/core-tests.obj" "/Fe$outputRoot/ShakeSpot.Native.Tests.exe" core-tests.cpp
        if ($LASTEXITCODE -ne 0) { throw 'Native core tests build failed.' }
        if ($Task -eq 'test') { & "$outputRoot/ShakeSpot.Native.Tests.exe"; if ($LASTEXITCODE -ne 0) { throw 'Native core tests failed.' } }
    }
    if ($Task -eq 'run') {
        $startInfo = [System.Diagnostics.ProcessStartInfo]::new("$outputRoot/ShakeSpot.Native.exe")
        $startInfo.UseShellExecute = $false
        $startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
        foreach ($argument in $AppArguments) { $startInfo.ArgumentList.Add($argument) }
        [System.Diagnostics.Process]::Start($startInfo) | Out-Null
    }
    if ($Task -eq 'desktop-checks') {
        & $compiler @common "/Fo$outputRoot/desktop-tests.obj" "/Fe$outputRoot/ShakeSpot.Native.DesktopTests.exe" desktop-tests.cpp /link /MANIFEST:EMBED "/MANIFESTINPUT:$nativeRoot/native.manifest" user32.lib gdi32.lib shell32.lib advapi32.lib ole32.lib comctl32.lib psapi.lib
        if ($LASTEXITCODE -ne 0) { throw 'Desktop tests build failed.' }
        & "$outputRoot/ShakeSpot.Native.DesktopTests.exe" @AppArguments
        if ($LASTEXITCODE -ne 0) { throw 'Native desktop tests failed.' }
    }
    if ($Task -eq 'publish') {
        $publishRoot = Join-Path $projectRoot 'artifacts/publish/native-win-x64'
        New-Item -ItemType Directory -Path $publishRoot -Force | Out-Null
        Copy-Item -LiteralPath "$outputRoot/ShakeSpot.Native.exe" -Destination $publishRoot -Force
        foreach ($document in @('README.md','NATIVE.md','NATIVE-VALIDATION.md','LEGACY.md','VALIDATION.md','THIRD_PARTY_NOTICES.md')) {
            Copy-Item -LiteralPath (Join-Path $projectRoot $document) -Destination $publishRoot -Force
        }
        $licensesRoot = Join-Path $publishRoot 'licenses'
        New-Item -ItemType Directory -Path $licensesRoot -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $projectRoot 'licenses/Microsoft-STL-LICENSE.txt') -Destination $licensesRoot -Force
        Compress-Archive -Path "$publishRoot/*" -DestinationPath (Join-Path $projectRoot 'artifacts/ShakeSpot-0.2.0-native-win-x64.zip') -Force
    }
}
finally { Pop-Location }
