using Godot;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Nodes.Cards;

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
/// <summary>One translated key as the edit panel lists it.</summary>
public sealed record CatalogEntry(
    string ModId,
    string Table,
    string Key,
    string? Source,
    string SheetValue,
    string Current,
    bool Edited);

public static class Injector
{
    /// <summary>Godot's NOTIFICATION_TRANSLATION_CHANGED; labels re-read their text on it.</summary>
    private const int TranslationChangedNotification = 2010;

    private static readonly Dictionary<(string Table, string Key), string?> Ledger = new();

    /// <summary>Every key the last apply translated, for the edit panel.</summary>
    public static IReadOnlyList<CatalogEntry> Catalog { get; private set; } = Array.Empty<CatalogEntry>();

    /// <summary>Language of <see cref="Catalog"/>.</summary>
    public static string CatalogLanguage { get; private set; } = "";

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
        var cards = RefreshLabels();
        BridgeMod.Log.Info($"[{BridgeMod.ModId}] reloaded {applied} entries ({manager.Language}), redrew {cards} cards");
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
            Catalog = OverlayEdits(tables, language);
            CatalogLanguage = language;

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
                    table.Sources.TryGetValue(key, out var source);
                    catalog.Add(new CatalogEntry(mod.Key, table.Table, key, source, sheetValue, table.Values[key], edited));
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

    /// <summary>
    /// Makes on-screen text pick up the new values. Plain labels listen for
    /// the translation-changed notification; cards set their text once when
    /// shown, so every card node already in the tree is redrawn.
    /// </summary>
    private static int RefreshLabels()
    {
        if (Engine.GetMainLoop() is not SceneTree tree)
        {
            return 0;
        }
        tree.Root.PropagateNotification(TranslationChangedNotification);
        var redrawn = 0;
        var pending = new Stack<Node>();
        pending.Push(tree.Root);
        while (pending.Count > 0)
        {
            var node = pending.Pop();
            if (node is NCard { Model: not null } card && card.IsNodeReady())
            {
                try
                {
                    card.UpdateVisuals(card.DisplayingPile, CardPreviewMode.Normal);
                    redrawn++;
                }
                catch (Exception exception)
                {
                    BridgeMod.Log.Warn($"[{BridgeMod.ModId}] card redraw failed: {exception.Message}");
                }
            }
            foreach (var child in node.GetChildren())
            {
                pending.Push(child);
            }
        }
        return redrawn;
    }
}
