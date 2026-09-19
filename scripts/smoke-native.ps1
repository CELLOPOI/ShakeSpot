$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $projectRoot 'artifacts/publish/native-win-x64/ShakeSpot.Native.exe'
$profilePath = Join-Path $projectRoot 'artifacts/native/published-smoke-profile'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class NativeSmoke {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string name, string title);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, UIntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr w, IntPtr l, uint flags, uint timeout, out UIntPtr result);
    public static long Query(IntPtr window, uint field) {
        UIntPtr result;
        if (SendMessageTimeout(window, 0x801F, (UIntPtr)field, IntPtr.Zero, 2, 1000, out result) == IntPtr.Zero) throw new Exception("Application did not respond.");
        return checked((long)result.ToUInt64());
    }
}
'@
if ([NativeSmoke]::FindWindow('ShakeSpot.Native.Control.v2', 'ShakeSpot Native') -ne [IntPtr]::Zero) { throw 'Close the existing native application first.' }
$startInfo = [Diagnostics.ProcessStartInfo]::new($exePath)
$startInfo.UseShellExecute = $false
$startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
$startInfo.ArgumentList.Add('--data-dir')
$startInfo.ArgumentList.Add($profilePath)
$application = [Diagnostics.Process]::Start($startInfo)
$guardian = $null
$window = [IntPtr]::Zero
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Start-Sleep -Milliseconds 50
        $window = [NativeSmoke]::FindWindow('ShakeSpot.Native.Control.v2', 'ShakeSpot Native')
        if ($application.HasExited) { throw "Published application exited early: $($application.ExitCode)" }
    } while ($window -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
    if ($window -eq [IntPtr]::Zero) { throw 'Published application did not create its control window.' }
    $guardianId = [NativeSmoke]::Query($window, 4)
    $guardian = [Diagnostics.Process]::GetProcessById($guardianId)
    if ($guardian.Path -ne $exePath) { throw 'Guardian did not use the published executable.' }
    [NativeSmoke]::PostMessage($window, 0x801E, [UIntPtr]2, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 200
    if ([NativeSmoke]::Query($window, 6) -ne 0 -or [NativeSmoke]::Query($window, 1) -ne 0) { throw 'Published pause command failed.' }
    [NativeSmoke]::PostMessage($window, 0x801E, [UIntPtr]3, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 200
    if ([NativeSmoke]::Query($window, 6) -ne 1) { throw 'Published resume command failed.' }
    [NativeSmoke]::PostMessage($window, 0x801E, [UIntPtr]5, [IntPtr]::Zero) | Out-Null
    if (-not $application.WaitForExit(5000) -or -not $guardian.WaitForExit(5000)) { throw 'Published processes did not exit.' }
    if ($application.ExitCode -ne 0) { throw 'Published application returned an error.' }
    $result = [ordered]@{ executable = $exePath; bytes = (Get-Item -LiteralPath $exePath).Length; sha256 = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash; guardianUsedPublishedExe = $true; pauseResume = $true; mainAndGuardianExited = $true; exitCode = $application.ExitCode }
    $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $projectRoot 'artifacts/native/published-smoke.json') -Encoding utf8NoBOM
    $result | ConvertTo-Json
}
finally {
    if (-not $application.HasExited) {
        if ($window -ne [IntPtr]::Zero) { [NativeSmoke]::PostMessage($window, 0x801E, [UIntPtr]5, [IntPtr]::Zero) | Out-Null }
        if (-not $application.WaitForExit(3000)) { $application.Kill(); $application.WaitForExit() }
    }
    if ($guardian) { $guardian.WaitForExit(5000) | Out-Null; $guardian.Dispose() }
    $application.Dispose()
}
