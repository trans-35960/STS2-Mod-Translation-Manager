using Godot;
using MegaCrit.Sts2.Core.Localization;

namespace Sts2ModManagerBridge;

/// <summary>
/// Puts the shared translations into the game's loc tables.
/// <para>
/// <see cref="LocTable.MergeWith"/> can only add or overwrite, never remove,
/// so the value each key had before we touched it is kept in a ledger. A key
/// that drops out of the translations goes back to that value on the next
/// reload. The ledger is reset whenever the game rebuilds its tables
/// (<see cref="LocManager.SetLanguage"/>).
/// </para>
/// </summary>
public static class Injector
{
    /// <summary>Godot's NOTIFICATION_TRANSLATION_CHANGED; labels re-read their text on it.</summary>
    private const int TranslationChangedNotification = 2010;

    private static readonly Dictionary<(string Table, string Key), string?> Ledger = new();

    /// <summary>Called after the game (re)built every table for a language.</summary>
    public static void OnTablesRebuilt(LocManager manager)
    {
        Ledger.Clear();
        Apply(manager);
    }

    /// <summary>Re-reads the shared folder and applies it to the current tables (F8).</summary>
    public static int Reload()
    {
        var manager = LocManager.Instance;
        if (manager == null)
        {
            return 0;
        }
        var applied = Apply(manager);
        RefreshLabels();
        return applied;
    }

    private static int Apply(LocManager manager)
    {
        var language = manager.Language;
        Status.BeginApply(language);
        try
        {
            var format = TranslationFiles.ReadFormat(SharedPaths.FormatFile);
            if (format == null)
            {
                Status.Write();
                return 0;
            }
            if (format != SharedPaths.SupportedFormat)
            {
                Status.RecordError("format", $"공용 폴더 형식 {format}은(는) 이 브리지가 지원하지 않습니다 (지원: {SharedPaths.SupportedFormat}). 프로그램과 브리지 버전을 맞춰 주세요.");
                Status.Write();
                return 0;
            }

            var (tables, readErrors) = TranslationFiles.Load(SharedPaths.Translations, language);
            foreach (var error in readErrors)
            {
                Status.RecordError("read", error);
            }

            var applied = new HashSet<(string Table, string Key)>();
            foreach (var group in tables.GroupBy(table => table.Table, StringComparer.Ordinal))
            {
                var table = TryGetTable(manager, group.Key);
                if (table == null)
                {
                    foreach (var missing in group)
                    {
                        Status.RecordMissingTable(missing.ModId, missing.Table);
                    }
                    continue;
                }
                var valid = new Dictionary<string, string>(StringComparer.Ordinal);
                foreach (var source in group)
                {
                    foreach (var (key, value) in source.Values)
                    {
                        if (!LocValidator.ValidateFormatString(value, out var error))
                        {
                            Status.RecordInvalid(source.ModId, source.Table, key, error ?? "");
                            continue;
                        }
                        if (!Ledger.ContainsKey((group.Key, key)))
                        {
                            Ledger[(group.Key, key)] = table.HasEntry(key) ? table.GetRawText(key) : null;
                        }
                        valid[key] = value;
                        applied.Add((group.Key, key));
                        Status.CountApplied(source.ModId);
                    }
                }
                if (valid.Count > 0)
                {
                    table.MergeWith(valid);
                }
            }

            RestoreDropped(manager, applied);
            Status.Write();
            return applied.Count;
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Error($"[{BridgeMod.ModId}] apply failed: {exception}");
            Status.RecordError("apply", exception.Message);
            Status.Write();
            return 0;
        }
    }

    /// <summary>Puts back the original text of keys we no longer translate.</summary>
    private static void RestoreDropped(LocManager manager, HashSet<(string Table, string Key)> applied)
    {
        foreach (var group in Ledger.Where(entry => !applied.Contains(entry.Key)).GroupBy(entry => entry.Key.Table).ToList())
        {
            var table = TryGetTable(manager, group.Key);
            if (table == null)
            {
                continue;
            }
            var restore = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var ((tableName, key), original) in group)
            {
                if (original == null)
                {
                    // Keys we added cannot be removed; they disappear when the
                    // game rebuilds its tables (language change or restart).
                    Status.RecordError("restore", $"{tableName}/{key}: 원래 없던 키라 재시작 후 사라집니다");
                    continue;
                }
                restore[key] = original;
                Ledger.Remove((tableName, key));
            }
            if (restore.Count > 0)
            {
                table.MergeWith(restore);
            }
        }
    }

    private static LocTable? TryGetTable(LocManager manager, string name)
    {
        try
        {
            return manager.GetTable(name);
        }
        catch
        {
            return null; // GetTable throws when the table does not exist
        }
    }

    private static void RefreshLabels()
    {
        if (Engine.GetMainLoop() is SceneTree tree)
        {
            tree.Root.PropagateNotification(TranslationChangedNotification);
        }
    }
}
