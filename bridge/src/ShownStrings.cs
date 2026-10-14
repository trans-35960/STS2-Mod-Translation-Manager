using System.Text.Json;
using Godot;
using HarmonyLib;
using MegaCrit.Sts2.addons.mega_text;

namespace Sts2ModManagerBridge;

/// <summary>
/// Records which mod DLL literals actually reach the screen. Every text set
/// on a label, button or tooltip is checked for the literals (or, when the
/// bridge replaced them, their translations) of every mod DLL; a hit marks
/// the literal as shown in <c>seen/{dll}.json</c>, which grows over every
/// session played. The app treats shown literals as text to translate even
/// when the code only seemed to log them.
/// </summary>
public static class ShownStrings
{
    private const int MaxRememberedTexts = 20000;
    private static readonly TimeSpan FlushInterval = TimeSpan.FromSeconds(15);

    private static readonly object Lock = new();
    private static readonly List<TextMatcher.Candidate> Candidates = new();
    private static readonly Dictionary<string, SeenFile> Files = new(StringComparer.OrdinalIgnoreCase);
    private static volatile TextMatcher _matcher = new(Array.Empty<TextMatcher.Candidate>());
    private static readonly HashSet<string> CheckedTexts = new(StringComparer.Ordinal);
    private static System.Threading.Timer? _flushTimer;

    public static void Initialize(Harmony harmony)
    {
        var postfix = new HarmonyMethod(typeof(ShownStrings), nameof(AfterTextSet));
        foreach (var (type, names) in new (Type, string[])[]
                 {
                     (typeof(Label), new[] { "set_Text", "SetText" }),
                     (typeof(RichTextLabel), new[] { "set_Text", "SetText", "AppendText", "AddText", "ParseBbcode" }),
                     (typeof(Button), new[] { "set_Text", "SetText" }),
                     (typeof(Control), new[] { "set_TooltipText", "SetTooltipText" }),
                     // The game's labels: Godot's tiny setters can be inlined
                     // into them, which would bypass the patches above.
                     (typeof(MegaLabel), new[] { "SetTextAutoSize" }),
                     (typeof(MegaRichTextLabel), new[] { "set_Text", "SetTextAutoSize" }),
                 })
        {
            foreach (var name in names)
            {
                var method = AccessTools.DeclaredMethod(type, name, new[] { typeof(string) });
                if (method == null)
                {
                    continue;
                }
                try
                {
                    harmony.Patch(method, postfix: postfix);
                }
                catch (Exception exception)
                {
                    BridgeMod.Log.Warn($"[{BridgeMod.ModId}] on-screen check of {type.Name}.{name} unavailable: {exception.Message}");
                }
            }
        }
        _flushTimer = new System.Threading.Timer(_ => Flush(), null, FlushInterval, FlushInterval);
        AppDomain.CurrentDomain.ProcessExit += (_, _) => Flush();
    }

    /// <summary>
    /// Starts looking for the literals of one DLL. <paramref name="translations"/>
    /// maps literals the bridge replaced to what is shown instead.
    /// </summary>
    public static void Watch(
        string fileStem,
        Guid mvid,
        long size,
        AssemblyStrings read,
        IReadOnlyDictionary<string, string>? translations)
    {
        var file = SeenFile.Load(SharedPaths.Seen, fileStem, mvid, size);
        var added = new List<TextMatcher.Candidate>();
        foreach (var (literal, use) in read.Uses)
        {
            var shownSomewhere = !StringUses.BreaksWhenTranslated(use) || read.Mixed.Contains(literal);
            if (!shownSomewhere || file.Literals.Contains(literal))
            {
                continue;
            }
            foreach (var text in new[] { literal, translations?.GetValueOrDefault(literal) })
            {
                if (text != null && TextMatcher.Candidate.For(text, fileStem, literal) is { } candidate)
                {
                    added.Add(candidate);
                }
            }
        }
        lock (Lock)
        {
            Files[fileStem] = file;
            Candidates.RemoveAll(candidate => string.Equals(candidate.Owner, fileStem, StringComparison.OrdinalIgnoreCase));
            Candidates.AddRange(added);
            _matcher = new TextMatcher(Candidates);
            // Texts checked before these literals were known get another look.
            CheckedTexts.Clear();
        }
    }

    private static void AfterTextSet(object __instance, string __0)
    {
        try
        {
            Check(__0, __instance as Node);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] on-screen check failed: {exception.Message}");
        }
    }

    private static void Check(string? text, Node? node)
    {
        var matcher = _matcher;
        if (text == null || text.Length < 2 || matcher.IsEmpty)
        {
            return;
        }
        lock (Lock)
        {
            if (CheckedTexts.Count >= MaxRememberedTexts)
            {
                CheckedTexts.Clear();
            }
            if (CheckedTexts.Contains(text))
            {
                return;
            }
        }
        var found = matcher.Find(TextMatcher.WithoutMarkup(text)).ToList();
        // Log lines in the dev console or a log window are not what players
        // read; the same text elsewhere still counts, so it is not remembered.
        if (found.Count > 0 && IsLogView(node))
        {
            return;
        }
        lock (Lock)
        {
            CheckedTexts.Add(text);
        }
        foreach (var candidate in found)
        {
            lock (Lock)
            {
                if (Files.TryGetValue(candidate.Owner, out var file) && file.Literals.Add(candidate.Literal))
                {
                    file.Dirty = true;
                }
            }
        }
    }

    /// <summary>The game's dev console, BaseLib's log window and the like.</summary>
    private static bool IsLogView(Node? node)
    {
        for (var depth = 0; node != null && depth < 12; depth++, node = node.GetParent())
        {
            var name = node.GetType().Name;
            if (name.Contains("Console", StringComparison.OrdinalIgnoreCase)
                || name.Contains("LogWindow", StringComparison.OrdinalIgnoreCase)
                || name.Contains("LogViewer", StringComparison.OrdinalIgnoreCase))
            {
                return true;
            }
        }
        return false;
    }

    private static void Flush()
    {
        List<SeenFile> dirty;
        lock (Lock)
        {
            dirty = Files.Values.Where(file => file.Dirty).ToList();
            foreach (var file in dirty)
            {
                file.Dirty = false;
            }
        }
        foreach (var file in dirty)
        {
            try
            {
                string[] literals;
                lock (Lock)
                {
                    literals = file.Literals.OrderBy(literal => literal, StringComparer.Ordinal).ToArray();
                }
                file.Write(SharedPaths.Seen, literals);
            }
            catch (Exception exception)
            {
                BridgeMod.Log.Warn($"[{BridgeMod.ModId}] {file.FileStem}: on-screen record not written: {exception.Message}");
            }
        }
    }

    /// <summary><c>seen/{dll}.json</c>: literals of one DLL build seen on screen so far.</summary>
    private sealed class SeenFile
    {
        /// <summary>Bumped when what counts as seen changes, so older records start over.</summary>
        private const int Rules = 3;

        public string FileStem { get; private init; } = "";
        public Guid Mvid { get; private init; }
        public long Size { get; private init; }
        public HashSet<string> Literals { get; } = new(StringComparer.Ordinal);
        public bool Dirty { get; set; }

        /// <summary>What earlier sessions saw of this same build; empty for another build.</summary>
        public static SeenFile Load(string root, string fileStem, Guid mvid, long size)
        {
            var file = new SeenFile { FileStem = fileStem, Mvid = mvid, Size = size };
            try
            {
                var path = Path.Combine(root, $"{fileStem}.json");
                if (File.Exists(path))
                {
                    using var document = JsonDocument.Parse(File.ReadAllText(path));
                    if (document.RootElement.TryGetProperty("mvid", out var stored) && stored.GetString() == mvid.ToString()
                        && document.RootElement.TryGetProperty("rules", out var rules) && rules.GetInt32() == Rules
                        && document.RootElement.TryGetProperty("literals", out var literals))
                    {
                        foreach (var literal in literals.EnumerateArray())
                        {
                            if (literal.GetString() is { } text)
                            {
                                file.Literals.Add(text);
                            }
                        }
                    }
                    else
                    {
                        // Another build or older rules: write it again (empty)
                        // even if nothing is seen this session.
                        file.Dirty = true;
                    }
                }
            }
            catch (Exception exception)
            {
                BridgeMod.Log.Warn($"[{BridgeMod.ModId}] {fileStem}: on-screen record unreadable, starting over: {exception.Message}");
            }
            return file;
        }

        public void Write(string root, string[] literals)
        {
            Directory.CreateDirectory(root);
            var path = Path.Combine(root, $"{FileStem}.json");
            var temp = path + ".tmp";
            var content = new
            {
                format = 1,
                rules = Rules,
                mvid = Mvid.ToString(),
                size = Size,
                updated_at = DateTimeOffset.Now.ToString("o"),
                literals,
            };
            File.WriteAllText(temp, JsonSerializer.Serialize(content, JsonOptions));
            File.Move(temp, path, overwrite: true);
        }

        private static readonly JsonSerializerOptions JsonOptions = new()
        {
            WriteIndented = true,
            Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        };
    }
}
