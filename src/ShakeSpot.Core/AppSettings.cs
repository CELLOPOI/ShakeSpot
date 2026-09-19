using System.Text.Json;

namespace ShakeSpot.Core;

public sealed record AppSettings
{
    public int Sensitivity { get; init; } = 3;
    public double MaximumScale { get; init; } = 4;
    public int DurationMs { get; init; } = 1100;
    public bool StartWithWindows { get; init; }
    public bool Enabled { get; init; } = true;

    public AppSettings Normalize() => this with
    {
        Sensitivity = Math.Clamp(Sensitivity, 1, 5),
        MaximumScale = double.IsFinite(MaximumScale) ? Math.Clamp(MaximumScale, 2, 8) : 4,
        DurationMs = Math.Clamp(DurationMs, 500, 3000)
    };
}

public sealed class SettingsStore(string directory)
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };
    public string FilePath { get; } = Path.Combine(directory, "settings.json");

    public AppSettings Load(out string? warning)
    {
        warning = null;
        try
        {
            if (!File.Exists(FilePath)) return new();
            return (JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(FilePath)) ?? new()).Normalize();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            warning = ex.Message;
            return new();
        }
    }

    public void Save(AppSettings settings)
    {
        Directory.CreateDirectory(directory);
        var temporary = FilePath + ".tmp";
        File.WriteAllText(temporary, JsonSerializer.Serialize(settings.Normalize(), JsonOptions));
        File.Move(temporary, FilePath, overwrite: true);
    }
}
