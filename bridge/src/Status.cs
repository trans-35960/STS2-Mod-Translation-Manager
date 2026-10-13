using System.Text.Json;
using Godot;

namespace Sts2ModManagerBridge;

/// <summary>
/// <c>status/bridge.json</c>: what the last apply did, so the app can tell
/// whether translations actually reached the game and why not.
/// </summary>
public static class Status
{
    private const int MaxListedIssues = 200;

    private static StatusFile _current = new();

    public static void BeginApply(string language)
    {
        _current = new StatusFile
        {
            BridgeVersion = BridgeMod.Version,
            GameVersion = ReadGameVersion(),
            Language = language,
            AppliedAt = DateTimeOffset.Now.ToString("o"),
        };
    }

    public static void CountApplied(string modId)
    {
        _current.Mods.TryGetValue(modId, out var count);
        _current.Mods[modId] = count + 1;
    }

    public static void RecordInvalid(string modId, string table, string key, string error)
    {
        if (_current.Invalid.Count < MaxListedIssues)
        {
            _current.Invalid.Add(new InvalidEntry(modId, table, key, error));
        }
        _current.InvalidCount++;
    }

    public static void RecordMissingTable(string modId, string table)
    {
        var name = $"{modId}/{table}";
        if (!_current.MissingTables.Contains(name))
        {
            _current.MissingTables.Add(name);
        }
    }

    public static void RecordError(string stage, string message)
    {
        if (_current.Errors.Count < MaxListedIssues)
        {
            _current.Errors.Add($"{stage}: {message}");
        }
    }

    public static void Write()
    {
        try
        {
            var directory = Path.GetDirectoryName(SharedPaths.StatusFile)!;
            Directory.CreateDirectory(directory);
            _current.DllStrings = StringReplacer.Reports;
            _current.DllStringsLanguage = StringReplacer.Language;
            foreach (var error in StringReplacer.Errors.Take(MaxListedIssues - _current.Errors.Count))
            {
                _current.Errors.Add($"strings: {error}");
            }
            var temp = SharedPaths.StatusFile + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(_current, JsonOptions));
            File.Move(temp, SharedPaths.StatusFile, overwrite: true);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] status write failed: {exception.Message}");
        }
    }

    private static string? ReadGameVersion()
    {
        try
        {
            var file = Path.Combine(Path.GetDirectoryName(OS.GetExecutablePath()) ?? "", "release_info.json");
            if (!File.Exists(file))
            {
                return null;
            }
            using var document = JsonDocument.Parse(File.ReadAllText(file));
            return document.RootElement.TryGetProperty("version", out var version) ? version.GetString() : null;
        }
        catch
        {
            return null;
        }
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    public sealed record InvalidEntry(string ModId, string Table, string Key, string Error);

    private sealed class StatusFile
    {
        public string BridgeVersion { get; set; } = "";
        public string? GameVersion { get; set; }
        public string Language { get; set; } = "";
        public string AppliedAt { get; set; } = "";

        /// <summary>Mod id → number of keys applied.</summary>
        public SortedDictionary<string, int> Mods { get; set; } = new(StringComparer.Ordinal);

        public int InvalidCount { get; set; }
        public List<InvalidEntry> Invalid { get; set; } = new();
        public List<string> MissingTables { get; set; } = new();
        public List<string> Errors { get; set; } = new();

        /// <summary>DLL string replacement per assembly (done once at start-up).</summary>
        public Dictionary<string, StringReplacer.AssemblyReport> DllStrings { get; set; } = new();

        public string DllStringsLanguage { get; set; } = "";
    }
}
