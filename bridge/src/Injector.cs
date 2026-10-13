using Godot;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Nodes.Cards;

namespace Sts2ModManagerBridge;

/// <summary>
/// One translated key as the edit panel lists it. <see cref="Number"/> is
/// unique across every mod in the game (the tool's own ids restart per mod
/// and file), short enough to put on screen, and stable while the
/// translations stay the same.
/// </summary>
public sealed record CatalogEntry(
    int Number,
    string ModId,
    string Table,
    string Key,
    EntryInfo Info,
    string SheetValue,
    string Current,
    bool Edited)
{
    public string Tag => $"#{Number}";
}

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

    /// <summary>What each key showed after the last apply, to find it on screen.</summary>
    private static readonly Dictionary<(string Table, string Key), string> Shown = new();

    /// <summary>Old text → new text from the last apply, for <see cref="RefreshLabels"/>.</summary>
    private static readonly Dictionary<string, string> Changed = new(StringComparer.Ordinal);

    /// <summary>Shortest old text replaced inside a longer label text; shorter ones only match whole labels.</summary>
    private const int MinPartialLength = 4;

    /// <summary>Every key the last apply translated, for the edit panel.</summary>
    public static IReadOnlyList<CatalogEntry> Catalog { get; private set; } = Array.Empty<CatalogEntry>();

    /// <summary>
    /// Prefix every translated text with <c>[#number]</c> so a line seen in
    /// game can be found in the edit panel. Off again after a restart.
    /// </summary>
    public static bool ShowKeys { get; set; }

    /// <summary>Language of <see cref="Catalog"/>.</summary>
    public static string CatalogLanguage { get; private set; } = "";

    /// <summary>Called after the game (re)built every table for a language.</summary>
    public static void OnTablesRebuilt(LocManager manager)
    {
        Ledger.Clear();
        Shown.Clear();
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
        var (cards, labels) = RefreshLabels();
        BridgeMod.Log.Info($"[{BridgeMod.ModId}] reloaded {applied} entries ({manager.Language}), redrew {cards} cards, updated {labels} labels");
        return applied;
    }

    private static int Apply(LocManager manager)
    {
        var language = manager.Language;
        Status.BeginApply(language);
        Changed.Clear();
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
            // Translations of mods the game did not load would only clutter
            // the panel; their tables are not there to translate anyway.
            var loaded = LoadedModIds();
            if (loaded != null)
            {
                tables.RemoveAll(table => !loaded.Contains(table.ModId));
            }
            Catalog = OverlayEdits(tables, language);
            CatalogLanguage = language;
            if (ShowKeys)
            {
                var tags = Catalog.ToDictionary(entry => (entry.ModId, entry.Table, entry.Key), entry => entry.Tag);
                foreach (var table in tables)
                {
                    foreach (var key in table.Values.Keys.ToList())
                    {
                        if (tags.TryGetValue((table.ModId, table.Table, key), out var tag))
                        {
                            table.Values[key] = $"[{tag}] {table.Values[key]}";
                        }
                    }
                }
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
                        var previous = Shown.TryGetValue((group.Key, key), out var shown) ? shown : Ledger[(group.Key, key)];
                        if (previous != null && previous != value)
                        {
                            Changed[previous] = value;
                        }
                        Shown[(group.Key, key)] = value;
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

    /// <summary>
    /// Puts in-game edits the app has not taken in yet on top of the app's
    /// values, and lists every key for the edit panel.
    /// </summary>
    private static List<CatalogEntry> OverlayEdits(List<TableTranslations> tables, string language)
    {
        var catalog = new List<CatalogEntry>();
        foreach (var mod in tables.GroupBy(table => table.ModId, StringComparer.Ordinal))
        {
            Dictionary<(string Table, string Key), EditRecord> pending;
            try
            {
                var ack = TranslationFiles.ReadEditsAck(SharedPaths.Translations, mod.Key, language);
                pending = EditLog.Pending(EditLog.Read(EditLog.PathFor(SharedPaths.Edits, mod.Key, language)), ack);
            }
            catch (Exception exception)
            {
                Status.RecordError("edits", $"{mod.Key}: {exception.Message}");
                pending = new();
            }
            foreach (var table in mod)
            {
                foreach (var key in table.Values.Keys.ToList())
                {
                    var sheetValue = table.Values[key];
                    var edited = pending.TryGetValue((table.Table, key), out var edit) && edit.Before == sheetValue;
                    if (edited)
                    {
                        table.Values[key] = edit!.After!;
                    }
                    var info = table.Info.TryGetValue(key, out var found) ? found : EntryInfo.Empty;
                    catalog.Add(new CatalogEntry(catalog.Count + 1, mod.Key, table.Table, key, info, sheetValue, table.Values[key], edited));
                }
            }
        }
        return catalog;
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
                if (Shown.Remove((tableName, key), out var shown) && shown != original)
                {
                    Changed[shown] = original;
                }
            }
            if (restore.Count > 0)
            {
                table.MergeWith(restore);
            }
        }
    }

    /// <summary>Manifest ids of the mods the game loaded, or null if that cannot be read.</summary>
    private static HashSet<string>? LoadedModIds()
    {
        try
        {
            return ModManager.GetLoadedMods()
                .Select(mod => mod.manifest?.id)
                .OfType<string>()
                .ToHashSet(StringComparer.Ordinal);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] loaded mods unknown: {exception.Message}");
            return null;
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

    /// <summary>
    /// Makes on-screen text pick up the new values. Cards are redrawn from
    /// their model. Every other label set its text once when its screen was
    /// built, so text that matches a value this apply changed is swapped for
    /// the new one: whole labels always, parts of longer labels only for
    /// texts long enough not to hit unrelated words.
    /// </summary>
    private static (int Cards, int Labels) RefreshLabels()
    {
        if (Engine.GetMainLoop() is not SceneTree tree)
        {
            return (0, 0);
        }
        tree.Root.PropagateNotification(TranslationChangedNotification);
        var partial = Changed
            .Where(change => change.Key.Length >= MinPartialLength && !change.Key.Contains('{'))
            .OrderByDescending(change => change.Key.Length)
            .ToList();
        var (cards, labels) = (0, 0);
        var pending = new Stack<Node>();
        pending.Push(tree.Root);
        while (pending.Count > 0)
        {
            var node = pending.Pop();
            try
            {
                switch (node)
                {
                    case NCard { Model: not null } card when card.IsNodeReady():
                        card.UpdateVisuals(card.DisplayingPile, CardPreviewMode.Normal);
                        cards++;
                        break;
                    case Label label when Replace(label.Text, partial) is { } text:
                        label.Text = text;
                        labels++;
                        break;
                    case RichTextLabel rich when Replace(rich.Text, partial) is { } text:
                        rich.Text = text;
                        labels++;
                        break;
                }
            }
            catch (Exception exception)
            {
                BridgeMod.Log.Warn($"[{BridgeMod.ModId}] refresh of {node.Name} failed: {exception.Message}");
            }
            foreach (var child in node.GetChildren())
            {
                pending.Push(child);
            }
        }
        return (cards, labels);
    }

    /// <summary>The label text with changed values swapped, or null when nothing matched.</summary>
    private static string? Replace(string text, List<KeyValuePair<string, string>> partial)
    {
        if (text.Length == 0 || Changed.Count == 0)
        {
            return null;
        }
        if (Changed.TryGetValue(text, out var whole))
        {
            return whole;
        }
        var result = text;
        foreach (var (old, replacement) in partial)
        {
            if (result.Contains(old, StringComparison.Ordinal))
            {
                result = result.Replace(old, replacement, StringComparison.Ordinal);
            }
        }
        return result == text ? null : result;
    }
}
