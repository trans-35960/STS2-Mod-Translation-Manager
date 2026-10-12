using System.Text.Json;

namespace Sts2ModManagerBridge;

/// <summary>
/// One loc table's translations for one mod: key → value, and key → source
/// text when the app wrote it (<c>_source/{table}.json</c>).
/// </summary>
public sealed record TableTranslations(
    string ModId,
    string Table,
    Dictionary<string, string> Values,
    Dictionary<string, string> Sources);

/// <summary>
/// Reads <c>translations/{modId}/{lang}/{table}.json</c> (flat string maps the
/// app writes). Pure file logic so it can be tested without the game.
/// </summary>
public static class TranslationFiles
{
    /// <summary>
    /// Format version the app wrote, or null when the folder was never set up.
    /// </summary>
    public static int? ReadFormat(string formatFile)
    {
        if (!File.Exists(formatFile))
        {
            return null;
        }
        using var document = JsonDocument.Parse(File.ReadAllText(formatFile));
        return document.RootElement.TryGetProperty("format", out var format) && format.TryGetInt32(out var value)
            ? value
            : null;
    }

    /// <summary>All tables for <paramref name="language"/>, with per-file read errors.</summary>
    public static (List<TableTranslations> Tables, List<string> Errors) Load(string translationsRoot, string language)
    {
        var tables = new List<TableTranslations>();
        var errors = new List<string>();
        if (!Directory.Exists(translationsRoot))
        {
            return (tables, errors);
        }
        foreach (var modDir in Directory.GetDirectories(translationsRoot).OrderBy(path => path, StringComparer.Ordinal))
        {
            var languageDir = Path.Combine(modDir, language);
            if (!Directory.Exists(languageDir))
            {
                continue;
            }
            var modId = Path.GetFileName(modDir);
            foreach (var file in Directory.GetFiles(languageDir, "*.json").OrderBy(path => path, StringComparer.Ordinal))
            {
                var table = Path.GetFileNameWithoutExtension(file);
                if (table.StartsWith('_'))
                {
                    continue; // _meta.json and other bookkeeping files
                }
                try
                {
                    var sourceFile = Path.Combine(languageDir, "_source", table + ".json");
                    var sources = File.Exists(sourceFile)
                        ? ReadFlatMap(sourceFile)
                        : new Dictionary<string, string>(StringComparer.Ordinal);
                    tables.Add(new TableTranslations(modId, table, ReadFlatMap(file), sources));
                }
                catch (Exception exception)
                {
                    errors.Add($"{modId}/{language}/{table}: {exception.Message}");
                }
            }
        }
        return (tables, errors);
    }

    /// <summary>Last in-game edit id the app took in (<c>_meta.json</c> <c>edits_ack</c>).</summary>
    public static string? ReadEditsAck(string translationsRoot, string modId, string language)
    {
        var file = Path.Combine(translationsRoot, modId, language, "_meta.json");
        if (!File.Exists(file))
        {
            return null;
        }
        try
        {
            using var document = JsonDocument.Parse(File.ReadAllText(file));
            return document.RootElement.TryGetProperty("edits_ack", out var ack) && ack.ValueKind == JsonValueKind.String
                ? ack.GetString()
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    public static Dictionary<string, string> ReadFlatMap(string file)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(file));
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var property in document.RootElement.EnumerateObject())
        {
            if (property.Value.ValueKind == JsonValueKind.String && property.Value.GetString() is { Length: > 0 } text)
            {
                values[property.Name] = text;
            }
        }
        return values;
    }
}
