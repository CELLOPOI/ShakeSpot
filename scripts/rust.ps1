#Requires -Version 7.0
param(
    [ValidateSet('build','test','check','bench','run','publish','desktop-checks')][string]$Task = 'build',
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$AppArguments = @()
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'The application build scripts require Windows x64.' }
$projectRoot = Split-Path -Parent $PSScriptRoot
foreach ($variable in @('CARGO_HOME','RUSTUP_HOME')) {
    if (-not [Environment]::GetEnvironmentVariable($variable,'Process')) {
        $value = [Environment]::GetEnvironmentVariable($variable,'User')
        if ($value) { [Environment]::SetEnvironmentVariable($variable,$value,'Process') }
    }
}
$cargo = Get-Command cargo -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
if (-not $cargo -and $env:CARGO_HOME) {
    $candidate = Join-Path $env:CARGO_HOME 'bin/cargo.exe'
    if (Test-Path -LiteralPath $candidate) { $cargo = $candidate }
}
if (-not $cargo) { throw 'Rust is missing. Install the Rust MSVC toolchain with rustup; see README.md.' }
$outputRoot = Join-Path $projectRoot 'artifacts/rust'
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
Push-Location -LiteralPath $projectRoot
try {
    if ($Task -eq 'check') {
        & $cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed.' }
        & $cargo clippy --all-targets --locked -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Rust static checks failed.' }
        return
    }
    if ($Task -eq 'test') {
        & $cargo test --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
        return
    }
    if ($Task -eq 'bench') {
        & $cargo bench --locked --bench detector
        if ($LASTEXITCODE -ne 0) { throw 'Rust detector benchmark failed.' }
        return
    }
    & $cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust application build failed.' }
    $exe = Join-Path $projectRoot 'target/release/ShakeSpot.exe'
    Copy-Item -LiteralPath $exe -Destination (Join-Path $outputRoot 'ShakeSpot.exe') -Force
    if ($Task -eq 'run') {
        $startInfo = [Diagnostics.ProcessStartInfo]::new($exe)
        $startInfo.UseShellExecute = $false
        $startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
        foreach ($argument in $AppArguments) { $startInfo.ArgumentList.Add($argument) }
        [Diagnostics.Process]::Start($startInfo) | Out-Null
    }
    if ($Task -eq 'desktop-checks') {
        . (Join-Path $PSScriptRoot 'msvc-environment.ps1')
        $testExe = Join-Path $outputRoot 'ShakeSpot.DesktopTests.exe'
        & $compiler /nologo /std:c++20 /O2 /MT /EHsc /W4 /WX /utf-8 /DUNICODE /D_UNICODE /DWINVER=0x0A00 /D_WIN32_WINNT=0x0A00 /permissive- "/Fo$outputRoot/desktop-tests.obj" "/Fe$testExe" 'validation/desktop-tests.cpp' /link /MANIFEST:EMBED "/MANIFESTINPUT:$projectRoot/rust/app.manifest" user32.lib gdi32.lib shell32.lib advapi32.lib ole32.lib comctl32.lib psapi.lib
        if ($LASTEXITCODE -ne 0) { throw 'Desktop test driver build failed.' }
        $testTarget = if ($AppArguments.Count) { $AppArguments[0] } else { Join-Path $outputRoot 'ShakeSpot.exe' }
        & $testExe $testTarget
        if ($LASTEXITCODE -ne 0) { throw 'Desktop tests failed.' }
    }
    if ($Task -eq 'publish') {
        $metadataText = & $cargo metadata --no-deps --format-version 1 --locked
        if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo package metadata.' }
        $metadata = $metadataText | ConvertFrom-Json
        $package = $metadata.packages | Where-Object name -EQ 'shakespot' | Select-Object -First 1
        if (-not $package -or $package.license -ne 'MIT') { throw 'Missing ShakeSpot package metadata or MIT license.' }
        $version = $package.version
        $publishRoot = Join-Path $projectRoot 'artifacts/publish/rust-win-x64'
        New-Item -ItemType Directory -Path $publishRoot -Force | Out-Null
        $packageFiles = [Collections.Generic.List[string]]::new()
        Copy-Item -LiteralPath $exe -Destination (Join-Path $publishRoot 'ShakeSpot.exe') -Force
        $packageFiles.Add('ShakeSpot.exe')
        Copy-Item -LiteralPath (Join-Path $projectRoot 'packaging/使用说明.txt') -Destination (Join-Path $publishRoot '使用说明.txt') -Force
        $packageFiles.Add('使用说明.txt')
        $documents = @('LICENSE','THIRD_PARTY_NOTICES.md')
        $licenses = @('windows-rs-MIT.txt','windows-rs-APACHE-2.0.txt','windows-link-MIT.txt','windows-link-APACHE-2.0.txt','Rust-1.93.1-COPYRIGHT-library.html','Microsoft-STL-LICENSE.txt')
        foreach ($relativePath in ($documents + @($licenses | ForEach-Object { "licenses/$_" }))) {
            $destination = Join-Path $publishRoot $relativePath
            New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $projectRoot $relativePath) -Destination $destination -Force
            $packageFiles.Add($relativePath)
        }
        $rustc = Join-Path (Split-Path -Parent $cargo) 'rustc.exe'
        $sysroot = & $rustc --print sysroot
        if ($LASTEXITCODE -ne 0) { throw 'Cannot locate Rust standard library notices.' }
        Copy-Item -LiteralPath (Join-Path $sysroot 'share/doc/rust/COPYRIGHT-library.html') -Destination (Join-Path $publishRoot 'licenses/Rust-COPYRIGHT-library.html') -Force
        $packageFiles.Add('licenses/Rust-COPYRIGHT-library.html')
        $rustVersion = & $rustc --version
        if ($LASTEXITCODE -ne 0) { throw 'Cannot read Rust compiler version.' }
        $cargoVersion = & $cargo --version
        if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo version.' }
        @("version=$version",$rustVersion,$cargoVersion,'target=x86_64-pc-windows-msvc','profile=release; opt-level=3; lto=thin; crt-static',('sha256=' + (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash)) |
            Set-Content -LiteralPath (Join-Path $publishRoot 'BUILD-INFO.txt') -Encoding utf8NoBOM
        $packageFiles.Add('BUILD-INFO.txt')
        $checksums = foreach ($relativePath in $packageFiles) {
            (Get-FileHash -LiteralPath (Join-Path $publishRoot $relativePath) -Algorithm SHA256).Hash + '  ' + $relativePath
        }
        $checksums | Set-Content -LiteralPath (Join-Path $publishRoot 'SHA256SUMS.txt') -Encoding utf8NoBOM
        $packageFiles.Add('SHA256SUMS.txt')

        # 明确列出发布文件，避免旧输出目录中的配置或测试文件混入 ZIP。
        $archivePath = Join-Path $projectRoot "artifacts/ShakeSpot-$version-win-x64.zip"
        $archiveStream = [IO.File]::Open($archivePath, [IO.FileMode]::Create)
        try {
            $archive = [IO.Compression.ZipArchive]::new($archiveStream, [IO.Compression.ZipArchiveMode]::Create, $true)
            try {
                foreach ($relativePath in $packageFiles) {
                    [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, (Join-Path $publishRoot $relativePath), $relativePath, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
                }
            } finally { $archive.Dispose() }
        } finally { $archiveStream.Dispose() }
        ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash + '  ' + (Split-Path -Leaf $archivePath)) |
            Set-Content -LiteralPath "$archivePath.sha256" -Encoding utf8NoBOM
        Write-Output "Package: $archivePath"
        Write-Output "Checksum: $archivePath.sha256"
    }
}
finally { Pop-Location }
