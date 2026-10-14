using System.Text.Encodings.Web;
using System.Text.Json;

namespace Sts2ModManagerBridge;

/// <summary>
/// One line of <c>edits/{modId}/{lang}.jsonl</c>: an in-game fix
/// (<c>edit</c>), the undo of one (<c>revert</c>), or a mistranslation
/// report (<c>report</c>) for the app to pick up.
/// </summary>
public sealed record EditRecord(
    string Id,
    string Kind,
    string Table,
    string Key,
    string? Source,
    string? Before,
    string? After,
    string Note);

/// <summary>
/// The append-only log of in-game edits. Only this mod writes it; the app
/// reads it and records the last id it took in as <c>edits_ack</c>. Pure
/// file logic so it can be tested without the game.
/// </summary>
public static class EditLog
{
    public const string Edit = "edit";
    public const string Revert = "revert";
    public const string Report = "report";

    private static readonly object IdLock = new();
    private static string _lastId = "";

    public static string PathFor(string editsRoot, string modId, string language) =>
        Path.Combine(editsRoot, modId, language + ".jsonl");

    /// <summary>DLL string edits have their own log (and ack), so taking in a loc sheet never acks them.</summary>
    public static string DllPathFor(string editsRoot, string modId, string language) =>
        Path.Combine(editsRoot, modId, language + ".dll.jsonl");

    /// <summary>Records in file order; lines that do not parse are skipped.</summary>
    public static List<EditRecord> Read(string file)
    {
        var records = new List<EditRecord>();
        if (!File.Exists(file))
        {
            return records;
        }
        foreach (var line in File.ReadLines(file))
        {
            if (string.IsNullOrWhiteSpace(line))
            {
                continue;
            }
            try
            {
                if (JsonSerializer.Deserialize<EditRecord>(line, JsonOptions) is { Id.Length: > 0 } record)
                {
                    records.Add(record);
                }
            }
            catch (JsonException)
            {
                // A line cut off by a crash; the rest of the log still counts.
            }
        }
        return records;
    }

    /// <summary>
    /// The value each key should show on top of the app's translation: the
    /// latest edit after <paramref name="ack"/>. An edit only holds while the
    /// app's value is still the one it replaced (<see cref="EditRecord.Before"/>);
    /// once the app changes that key, the app wins.
    /// </summary>
    public static Dictionary<(string Table, string Key), EditRecord> Pending(
        IEnumerable<EditRecord> records,
        string? ack)
    {
        var pending = new Dictionary<(string Table, string Key), EditRecord>();
        foreach (var record in records)
        {
            if (ack != null && string.CompareOrdinal(record.Id, ack) <= 0)
            {
                continue;
            }
            if (record.Kind == Edit && record.After != null)
            {
                pending[(record.Table, record.Key)] = record;
            }
            else if (record.Kind == Revert)
            {
                pending.Remove((record.Table, record.Key));
            }
        }
        return pending;
    }

    public static void Append(string file, EditRecord record)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        File.AppendAllText(file, JsonSerializer.Serialize(record, JsonOptions) + "\n");
    }

    /// <summary>Sortable, unique id: local time to the millisecond plus a counter.</summary>
    public static string NewId(DateTimeOffset now)
    {
        lock (IdLock)
        {
            var stamp = now.ToString("yyyyMMdd'T'HHmmssfff");
            var sequence = 1;
            if (_lastId.StartsWith(stamp, StringComparison.Ordinal)
                && int.TryParse(_lastId.AsSpan(stamp.Length + 1), out var last))
            {
                sequence = last + 1;
            }
            var id = $"{stamp}-{sequence:0000}";
            if (string.CompareOrdinal(id, _lastId) <= 0)
            {
                id = _lastId[..(_lastId.Length - 4)] + (int.Parse(_lastId[^4..]) + 1).ToString("0000");
            }
            _lastId = id;
            return id;
        }
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };
}
