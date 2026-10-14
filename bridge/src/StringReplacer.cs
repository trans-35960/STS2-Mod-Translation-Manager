using System.Diagnostics;
using System.Reflection;
using System.Reflection.Emit;
using HarmonyLib;
using MegaCrit.Sts2.Core.Saves;

namespace Sts2ModManagerBridge;

/// <summary>
/// Replaces string literals compiled into mod DLLs (text that never went
/// through a loc table). For every assembly the app sent strings for, the
/// methods whose IL loads one of them get a transpiler that swaps the
/// literal. The CLR keeps one copy of each literal per assembly, so matching
/// by text finds every use without depending on file offsets.
/// <para>
/// Patching happens when the assembly loads (or at start-up when it already
/// has), before most of its code runs; the app puts the bridge first in the
/// load order so that includes the mod's initializer. Values a mod already
/// built from these strings stay as they were, so changes need a restart.
/// </para>
/// <para>
/// Literals the code compares, looks up or passes on as names
/// (<see cref="StringUses.BreaksWhenTranslated"/>) are never replaced.
/// Every mod DLL also gets its literal uses written to <c>analysis/</c>, so
/// the app can show which strings are worth translating.
/// </para>
/// </summary>
public static class StringReplacer
{
    private const int MaxListedProtected = 200;

    /// <summary>Assembly or DLL file name → original → translation, for the language the game starts in.</summary>
    private static Dictionary<string, Dictionary<string, string>> _byAssembly = new(StringComparer.OrdinalIgnoreCase);

    private static readonly HashSet<string> Seen = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Per patched method, which <c>ldstr</c> (in IL order) to replace.</summary>
    private static readonly System.Collections.Concurrent.ConcurrentDictionary<MethodBase, bool[]> Replacements = new();
    private static readonly object SeenLock = new();
    private static Harmony? _harmony;

    /// <summary>Analysis of DLLs nothing is replaced in runs off the main thread, one at a time.</summary>
    private static Task _analysisQueue = Task.CompletedTask;

    /// <summary>Each mod DLL as read at load, by assembly name and DLL file name, for live edits.</summary>
    private static readonly System.Collections.Concurrent.ConcurrentDictionary<string, (Assembly Assembly, AssemblyStrings Read)> Reads =
        new(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// The strings the app sent, for the in-game edit panel: <c>Translation</c>
    /// is the app's value, <c>Current</c> what the game uses after edits the
    /// app has not taken in yet.
    /// </summary>
    public static IReadOnlyList<(DllEntry Entry, string Current)> Entries { get; private set; } =
        Array.Empty<(DllEntry, string)>();

    /// <summary>Per assembly results, for <c>status/bridge.json</c>.</summary>
    public static Dictionary<string, AssemblyReport> Reports { get; } = new(StringComparer.OrdinalIgnoreCase);

    public static List<string> Errors { get; } = new();

    public static string Language { get; private set; } = "";

    public static void Initialize(Harmony harmony)
    {
        _harmony = harmony;
        Language = StartLanguage();
        var (entries, errors) = StringFiles.Load(SharedPaths.Strings, Language);
        Errors.AddRange(errors);
        Entries = WithPendingEdits(entries, Language);
        _byAssembly = StringFiles.Replacements(Entries);
        AppDomain.CurrentDomain.AssemblyLoad += (_, args) => OnAssembly(args.LoadedAssembly);
        foreach (var assembly in AppDomain.CurrentDomain.GetAssemblies())
        {
            OnAssembly(assembly);
        }
    }

    /// <summary>
    /// Puts in-game edits the app has not taken in yet on top of the app's
    /// values. Like loc edits, one only holds while the app's value is still
    /// the one it replaced.
    /// </summary>
    private static List<(DllEntry Entry, string Current)> WithPendingEdits(List<DllEntry> entries, string language)
    {
        var result = new List<(DllEntry, string)>(entries.Count);
        foreach (var mod in entries.GroupBy(entry => entry.ModId, StringComparer.Ordinal))
        {
            Dictionary<(string Table, string Key), EditRecord> pending;
            try
            {
                var ack = TranslationFiles.ReadEditsAck(SharedPaths.Strings, mod.Key, language);
                pending = EditLog.Pending(EditLog.Read(EditLog.DllPathFor(SharedPaths.Edits, mod.Key, language)), ack);
            }
            catch (Exception exception)
            {
                Errors.Add($"{mod.Key}: DLL edits unreadable: {exception.Message}");
                pending = new();
            }
            foreach (var entry in mod)
            {
                var edited = pending.TryGetValue((entry.Table, entry.Original), out var edit)
                    && (edit.Before ?? "") == entry.Translation;
                result.Add((entry, edited ? edit!.After ?? "" : entry.Translation));
            }
        }
        return result;
    }

    /// <summary>
    /// Changes one string while the game runs: the methods that load it are
    /// patched again, so text built from now on uses the new value. Text a
    /// screen already built stays until it is built again. Returns the number
    /// of methods patched, or -1 when the DLL was not read at load.
    /// </summary>
    public static int ApplyLive(string assembly, string original, string translation)
    {
        if (_harmony == null || !Reads.TryGetValue(assembly, out var loaded))
        {
            return -1;
        }
        var (target, read) = loaded;
        var name = target.GetName().Name ?? assembly;
        var strings = Lookup(name) ?? Lookup(Path.GetFileNameWithoutExtension(target.Location));
        if (strings == null)
        {
            strings = new Dictionary<string, string>(StringComparer.Ordinal);
            _byAssembly[assembly] = strings;
        }
        if (translation.Length == 0)
        {
            strings.Remove(original);
        }
        else
        {
            strings[original] = translation;
        }
        Entries = Entries
            .Select(item => string.Equals(item.Entry.Assembly, assembly, StringComparison.OrdinalIgnoreCase)
                && item.Entry.Original == original
                    ? (item.Entry, translation)
                    : item)
            .ToList();
        var patched = 0;
        foreach (var (method, sites) in read.Methods)
        {
            if (!sites.Any(site => site.Literal == original)
                || method.IsGenericMethodDefinition
                || method.DeclaringType?.ContainsGenericParameters == true)
            {
                continue;
            }
            var replace = sites.Select(site => strings.ContainsKey(site.Literal) && read.CanReplace(site)).ToArray();
            try
            {
                _harmony.Unpatch(method, HarmonyPatchType.Transpiler, _harmony.Id);
                if (replace.Any(replaced => replaced))
                {
                    Replacements[method] = replace;
                    _harmony.Patch(method, transpiler: new HarmonyMethod(typeof(StringReplacer), nameof(Transpiler)));
                    patched++;
                }
                else
                {
                    Replacements.TryRemove(method, out _);
                }
            }
            catch (Exception exception)
            {
                BridgeMod.Log.Warn($"[{BridgeMod.ModId}] live edit of {method.DeclaringType?.FullName}.{method.Name} failed: {exception.Message}");
            }
        }
        return patched;
    }

    private static void Remember(Assembly assembly, string name, string fileStem, AssemblyStrings read)
    {
        Reads[name] = (assembly, read);
        Reads[fileStem] = (assembly, read);
    }

    /// <summary>The language the game will start in; loc tables are not built yet when mods load.</summary>
    private static string StartLanguage()
    {
        try
        {
            if (SaveManager.Instance?.SettingsSave?.Language is { Length: > 0 } language)
            {
                return language;
            }
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] start language unknown: {exception.Message}");
        }
        return "eng";
    }

    private static void OnAssembly(Assembly assembly)
    {
        if (assembly.IsDynamic || assembly == typeof(StringReplacer).Assembly || ModFile(assembly) is not { } file)
        {
            return;
        }
        var name = assembly.GetName().Name ?? "";
        lock (SeenLock)
        {
            if (!Seen.Add(name))
            {
                return;
            }
        }
        var fileStem = Path.GetFileNameWithoutExtension(file.Name);
        if (_byAssembly.TryGetValue(fileStem, out var strings) || _byAssembly.TryGetValue(name, out strings))
        {
            Patch(assembly, name, fileStem, file, strings);
            return;
        }
        var mvid = assembly.ManifestModule.ModuleVersionId;
        // Read every start (a few hundred ms, off the main thread): the
        // on-screen check needs the literals even when the file is current.
        _analysisQueue = _analysisQueue.ContinueWith(_ =>
        {
            try
            {
                var read = AssemblyStrings.Read(assembly);
                Remember(assembly, name, fileStem, read);
                if (!AnalysisFiles.IsCurrent(SharedPaths.Analysis, fileStem, mvid))
                {
                    AnalysisFiles.Write(SharedPaths.Analysis, fileStem, name, mvid, file.Length, read);
                }
                ShownStrings.Watch(fileStem, mvid, file.Length, read, null);
            }
            catch (Exception exception)
            {
                BridgeMod.Log.Warn($"[{BridgeMod.ModId}] {name}: string analysis failed: {exception.Message}");
            }
        }, TaskScheduler.Default);
    }

    /// <summary>The DLL file of an assembly loaded from a mod folder; null for the game and the runtime.</summary>
    private static FileInfo? ModFile(Assembly assembly)
    {
        try
        {
            var location = assembly.Location;
            if (string.IsNullOrEmpty(location))
            {
                return null;
            }
            var normalized = location.Replace('\\', '/');
            var isMod = normalized.Contains("/mods/", StringComparison.OrdinalIgnoreCase)
                || normalized.Contains("/workshop/content/", StringComparison.OrdinalIgnoreCase);
            return isMod ? new FileInfo(location) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static void Patch(Assembly assembly, string name, string fileStem, FileInfo file, Dictionary<string, string> strings)
    {
        var report = new AssemblyReport { Translations = strings.Count };
        Reports[name] = report;
        var watch = Stopwatch.StartNew();
        try
        {
            // Every method is read first: text containing a word the code
            // searches for must stay as it is everywhere.
            var read = AssemblyStrings.Read(assembly);
            Remember(assembly, name, fileStem, read);
            // Text only an unreadable method loads still must not hide a searched-for word.
            foreach (var original in strings.Keys)
            {
                read.Uses.TryAdd(original, StringUse.Unknown);
            }
            StringUses.MarkTextContainingKeys(read.Uses);
            report.Unreadable = read.Unreadable;
            report.FailedMethods.AddRange(read.FailedMethods);

            // Each load is decided on its own: a word compared in one method
            // and shown by another is replaced only where it is shown.
            var replacedAt = new HashSet<string>(StringComparer.Ordinal);
            var keptAt = new HashSet<string>(StringComparer.Ordinal);
            foreach (var (method, sites) in read.Methods)
            {
                var replace = sites.Select(site => strings.ContainsKey(site.Literal) && read.CanReplace(site)).ToArray();
                if (!replace.Any(replaced => replaced))
                {
                    keptAt.UnionWith(sites.Select(site => site.Literal).Where(strings.ContainsKey));
                    continue;
                }
                if (method.IsGenericMethodDefinition || method.DeclaringType?.ContainsGenericParameters == true)
                {
                    // Harmony cannot patch open generic code reliably.
                    report.SkippedGeneric++;
                    continue;
                }
                try
                {
                    Replacements[method] = replace;
                    _harmony!.Patch(method, transpiler: new HarmonyMethod(typeof(StringReplacer), nameof(Transpiler)));
                    report.Methods++;
                    foreach (var (site, replaced) in sites.Zip(replace))
                    {
                        if (strings.ContainsKey(site.Literal))
                        {
                            (replaced ? replacedAt : keptAt).Add(site.Literal);
                        }
                    }
                }
                catch (Exception exception)
                {
                    Replacements.TryRemove(method, out _);
                    report.Failed++;
                    if (report.FailedMethods.Count < 20)
                    {
                        report.FailedMethods.Add($"{method.DeclaringType?.FullName}.{method.Name}: {exception.Message}");
                    }
                }
            }
            report.Found = replacedAt.Count;
            var protectedStrings = strings.Keys
                .Where(original => !replacedAt.Contains(original)
                    && (keptAt.Contains(original) || StringUses.BreaksWhenTranslated(read.Uses[original])))
                .ToList();
            report.Protected = protectedStrings.Count;
            report.ProtectedStrings.AddRange(Listed(protectedStrings));
            var partial = replacedAt.Where(keptAt.Contains).ToList();
            report.Partial = partial.Count;
            report.PartialStrings.AddRange(Listed(partial));
            try
            {
                AnalysisFiles.Write(SharedPaths.Analysis, fileStem, name, assembly.ManifestModule.ModuleVersionId, file.Length, read);
                ShownStrings.Watch(fileStem, assembly.ManifestModule.ModuleVersionId, file.Length, read, strings);
            }
            catch (Exception exception)
            {
                Errors.Add($"{name}: analysis not written: {exception.Message}");
            }
        }
        catch (Exception exception)
        {
            Errors.Add($"{name}: {exception.Message}");
        }
        report.Milliseconds = watch.ElapsedMilliseconds;
        BridgeMod.Log.Info(
            $"[{BridgeMod.ModId}] {name}: replaced {report.Found}/{report.Translations} DLL strings in {report.Methods} methods ({report.Milliseconds} ms, kept {report.Protected} used as names or keys, {report.Partial} only where shown, failed {report.Failed}, generic {report.SkippedGeneric}, unreadable {report.Unreadable})");
    }

    private static List<string> Listed(IEnumerable<string> literals) =>
        literals.OrderBy(literal => literal, StringComparer.Ordinal).Take(MaxListedProtected).ToList();

    /// <summary>Swaps the literals of the loads <see cref="Replacements"/> allows, counted in IL order.</summary>
    private static IEnumerable<CodeInstruction> Transpiler(IEnumerable<CodeInstruction> instructions, MethodBase __originalMethod)
    {
        var assembly = __originalMethod.Module.Assembly;
        var strings = Lookup(assembly.GetName().Name)
            ?? Lookup(Path.GetFileNameWithoutExtension(assembly.Location));
        Replacements.TryGetValue(__originalMethod, out var replace);
        var load = 0;
        foreach (var instruction in instructions)
        {
            if (instruction.opcode == OpCodes.Ldstr && instruction.operand is string original)
            {
                if (strings != null
                    && replace != null
                    && load < replace.Length
                    && replace[load]
                    && strings.TryGetValue(original, out var translation))
                {
                    instruction.operand = translation;
                }
                load++;
            }
            yield return instruction;
        }
    }

    private static Dictionary<string, string>? Lookup(string? name) =>
        name != null && _byAssembly.TryGetValue(name, out var strings) ? strings : null;

    public sealed class AssemblyReport
    {
        /// <summary>Strings the app sent for this assembly.</summary>
        public int Translations { get; set; }

        /// <summary>Of those, strings found in patched methods.</summary>
        public int Found { get; set; }

        /// <summary>Strings left untranslated because the code uses them as names or keys.</summary>
        public int Protected { get; set; }

        /// <summary>The first of those, for the app to show.</summary>
        public List<string> ProtectedStrings { get; } = new();

        /// <summary>Strings replaced where they are shown but kept where the code compares them.</summary>
        public int Partial { get; set; }

        public List<string> PartialStrings { get; } = new();

        public int Methods { get; set; }
        public int Failed { get; set; }
        public int SkippedGeneric { get; set; }

        /// <summary>Methods whose IL could not be read.</summary>
        public int Unreadable { get; set; }
        public long Milliseconds { get; set; }
        public List<string> FailedMethods { get; } = new();
    }
}
