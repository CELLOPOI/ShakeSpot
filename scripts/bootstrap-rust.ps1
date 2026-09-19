#Requires -Version 7.0
param([string]$CargoRoot, [string]$RustupRoot)
$ErrorActionPreference = 'Stop'
if (-not $CargoRoot) { $CargoRoot = $env:CARGO_HOME }
if (-not $CargoRoot) { $CargoRoot = [Environment]::GetEnvironmentVariable('CARGO_HOME','User') }
if (-not $CargoRoot) { $CargoRoot = Join-Path $env:USERPROFILE '.cargo' }
if (-not $RustupRoot) { $RustupRoot = $env:RUSTUP_HOME }
if (-not $RustupRoot) { $RustupRoot = [Environment]::GetEnvironmentVariable('RUSTUP_HOME','User') }
if (-not $RustupRoot) { $RustupRoot = Join-Path $env:USERPROFILE '.rustup' }
$installerRoot = Join-Path $env:TEMP 'ShakeSpot-rust-setup'
New-Item -ItemType Directory -Path $installerRoot,$cargoRoot -Force | Out-Null
$backup = [ordered]@{
    CARGO_HOME = [Environment]::GetEnvironmentVariable('CARGO_HOME','User')
    RUSTUP_HOME = [Environment]::GetEnvironmentVariable('RUSTUP_HOME','User')
    Path = [Environment]::GetEnvironmentVariable('Path','User')
}
$backup | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $installerRoot ('environment-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.json')) -Encoding utf8NoBOM
$env:CARGO_HOME = $cargoRoot
$env:RUSTUP_HOME = $rustupRoot
$rustup = Join-Path $cargoRoot 'bin/rustup.exe'
if (-not (Test-Path -LiteralPath $rustup)) {
    $installer = Join-Path $installerRoot 'rustup-init.exe'
    Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $installer
    & $installer -y --profile minimal --default-toolchain none --no-modify-path
    if ($LASTEXITCODE -ne 0) { throw 'Rustup installation failed.' }
}
$installed = @(& $rustup toolchain list)
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain inspection failed.' }
if (-not ($installed -match '^stable-x86_64-pc-windows-msvc')) {
    & $rustup toolchain install stable --profile minimal
    if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain installation failed.' }
    & $rustup default stable
    if ($LASTEXITCODE -ne 0) { throw 'Rust default toolchain configuration failed.' }
}
& $rustup show
if ($LASTEXITCODE -ne 0) { throw 'Rustup inspection failed.' }
& $rustup component add rustfmt clippy --toolchain stable
if ($LASTEXITCODE -ne 0) { throw 'Rust components installation failed.' }
[Environment]::SetEnvironmentVariable('CARGO_HOME',$cargoRoot,'User')
[Environment]::SetEnvironmentVariable('RUSTUP_HOME',$rustupRoot,'User')
$cargoBin = Join-Path $cargoRoot 'bin'
$userPath = [Environment]::GetEnvironmentVariable('Path','User')
if (([string]$userPath).Split(';') -notcontains $cargoBin) {
    [Environment]::SetEnvironmentVariable('Path',($cargoBin + ';' + $userPath),'User')
}
$env:PATH = $cargoBin + ';' + $env:PATH
& $rustup run stable rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Rust compiler verification failed.' }
& $rustup run stable cargo --version
if ($LASTEXITCODE -ne 0) { throw 'Cargo verification failed.' }
