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
    private static readonly object SeenLock = new();
    private static Harmony? _harmony;

    /// <summary>Analysis of DLLs nothing is replaced in runs off the main thread, one at a time.</summary>
    private static Task _analysisQueue = Task.CompletedTask;

    /// <summary>Per assembly results, for <c>status/bridge.json</c>.</summary>
    public static Dictionary<string, AssemblyReport> Reports { get; } = new(StringComparer.OrdinalIgnoreCase);

    public static List<string> Errors { get; } = new();

    public static string Language { get; private set; } = "";

    public static void Initialize(Harmony harmony)
    {
        _harmony = harmony;
        Language = StartLanguage();
        var (assemblies, errors) = StringFiles.Load(SharedPaths.Strings, Language);
        Errors.AddRange(errors);
        _byAssembly = assemblies;
        AppDomain.CurrentDomain.AssemblyLoad += (_, args) => OnAssembly(args.LoadedAssembly);
        foreach (var assembly in AppDomain.CurrentDomain.GetAssemblies())
        {
            OnAssembly(assembly);
        }
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
        if (AnalysisFiles.IsCurrent(SharedPaths.Analysis, fileStem, mvid))
        {
            return;
        }
        _analysisQueue = _analysisQueue.ContinueWith(_ =>
        {
            try
            {
                AnalysisFiles.Write(SharedPaths.Analysis, fileStem, name, mvid, file.Length, AssemblyStrings.Read(assembly));
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
            // Every method is read first: a literal used as a name or key
            // anywhere must stay as it is everywhere.
            var read = AssemblyStrings.Read(assembly);
            // Text only an unreadable method loads still must not hide a searched-for word.
            foreach (var original in strings.Keys)
            {
                read.Uses.TryAdd(original, StringUse.Unknown);
            }
            StringUses.MarkTextContainingKeys(read.Uses);
            report.Unreadable = read.Unreadable;
            report.FailedMethods.AddRange(read.FailedMethods);
            foreach (var original in strings.Keys.ToList())
            {
                if (read.Uses.TryGetValue(original, out var use) && StringUses.BreaksWhenTranslated(use))
                {
                    // The transpiler reads this same dictionary.
                    strings.Remove(original);
                    report.ProtectedStrings.Add(original);
                }
            }
            report.Protected = report.ProtectedStrings.Count;
            report.ProtectedStrings.Sort(StringComparer.Ordinal);
            if (report.ProtectedStrings.Count > MaxListedProtected)
            {
                report.ProtectedStrings.RemoveRange(MaxListedProtected, report.ProtectedStrings.Count - MaxListedProtected);
            }

            var found = new HashSet<string>(StringComparer.Ordinal);
            foreach (var (method, loaded) in read.Methods)
            {
                var hits = loaded.Where(strings.ContainsKey).ToList();
                if (hits.Count == 0)
                {
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
                    _harmony!.Patch(method, transpiler: new HarmonyMethod(typeof(StringReplacer), nameof(Transpiler)));
                    report.Methods++;
                    found.UnionWith(hits);
                }
                catch (Exception exception)
                {
                    report.Failed++;
                    if (report.FailedMethods.Count < 20)
                    {
                        report.FailedMethods.Add($"{method.DeclaringType?.FullName}.{method.Name}: {exception.Message}");
                    }
                }
            }
            report.Found = found.Count;
            try
            {
                AnalysisFiles.Write(SharedPaths.Analysis, fileStem, name, assembly.ManifestModule.ModuleVersionId, file.Length, read);
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
            $"[{BridgeMod.ModId}] {name}: replaced {report.Found}/{report.Translations} DLL strings in {report.Methods} methods ({report.Milliseconds} ms, kept {report.Protected} used as names or keys, failed {report.Failed}, generic {report.SkippedGeneric}, unreadable {report.Unreadable})");
    }

    private static IEnumerable<CodeInstruction> Transpiler(IEnumerable<CodeInstruction> instructions, MethodBase __originalMethod)
    {
        var assembly = __originalMethod.Module.Assembly;
        var strings = Lookup(assembly.GetName().Name)
            ?? Lookup(Path.GetFileNameWithoutExtension(assembly.Location));
        foreach (var instruction in instructions)
        {
            if (strings != null
                && instruction.opcode == OpCodes.Ldstr
                && instruction.operand is string original
                && strings.TryGetValue(original, out var translation))
            {
                instruction.operand = translation;
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

        public int Methods { get; set; }
        public int Failed { get; set; }
        public int SkippedGeneric { get; set; }

        /// <summary>Methods whose IL could not be read.</summary>
        public int Unreadable { get; set; }
        public long Milliseconds { get; set; }
        public List<string> FailedMethods { get; } = new();
    }
}
