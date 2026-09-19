$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $projectRoot 'artifacts/publish/win-x64/ShakeSpot.exe'
$reportPath = Join-Path $projectRoot 'artifacts/published-smoke.json'
$profilePath = Join-Path $projectRoot 'artifacts/published-smoke-profile'
$start = [System.Diagnostics.ProcessStartInfo]::new($exePath)
$start.UseShellExecute = $false
$start.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
foreach ($argument in @('--data-dir', $profilePath, '--diagnostics', $reportPath, '--exit-after', '4')) {
    $start.ArgumentList.Add($argument)
}
$smokeProcess = [System.Diagnostics.Process]::Start($start)
try {
    if (-not $smokeProcess.WaitForExit(20000)) {
        $smokeProcess.Kill()
        throw 'Published application did not exit before timeout.'
    }
    if ($smokeProcess.ExitCode -ne 0) { throw "Published application failed: $($smokeProcess.ExitCode)" }
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    if ($report.Samples -le 0 -or $report.ExitCode -ne 0) { throw 'Published application diagnostics failed.' }
    Write-Output 'PASS Published self-contained EXE starts, samples and exits normally.'
    $report | Format-List
}
finally { $smokeProcess.Dispose() }
