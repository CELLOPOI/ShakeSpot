$ErrorActionPreference = 'Stop'
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
$env:PATH = (Split-Path -Parent $compiler) + ';' + (Join-Path $sdkRoot "bin/$sdkVersion/x64") + ';' + $env:PATH
