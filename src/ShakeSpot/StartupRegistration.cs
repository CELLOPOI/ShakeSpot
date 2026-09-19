using System;
using System.IO;
using Microsoft.Win32;

namespace ShakeSpot;

internal static class StartupRegistration
{
    private const string KeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "ShakeSpot";

    internal static bool IsEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(KeyPath);
        return string.Equals(key?.GetValue(ValueName) as string, Command(), StringComparison.OrdinalIgnoreCase);
    }

    internal static void SetEnabled(bool enabled)
    {
        using var key = Registry.CurrentUser.CreateSubKey(KeyPath, writable: true);
        if (enabled) key.SetValue(ValueName, Command(), RegistryValueKind.String);
        else key.DeleteValue(ValueName, throwOnMissingValue: false);
    }

    private static string Command()
    {
        var executable = Environment.ProcessPath ?? throw new InvalidOperationException("Executable path unavailable.");
        if (!string.Equals(Path.GetFileName(executable), "ShakeSpot.exe", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("Run ShakeSpot.exe to configure Windows startup.");
        return $"\"{executable}\"";
    }
}
