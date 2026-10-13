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
/// </summary>
public static class StringReplacer
{
    private const BindingFlags AllDeclared =
        BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

    /// <summary>Assembly name → original → translation, for the language the game starts in.</summary>
    private static Dictionary<string, Dictionary<string, string>> _byAssembly = new(StringComparer.OrdinalIgnoreCase);

    private const int MaxListedProtected = 200;

    private static readonly HashSet<string> Patched = new(StringComparer.OrdinalIgnoreCase);
    private static readonly object PatchLock = new();
    private static Harmony? _harmony;

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
        if (_byAssembly.Count == 0)
        {
            return;
        }
        AppDomain.CurrentDomain.AssemblyLoad += (_, args) => TryPatch(args.LoadedAssembly);
        foreach (var assembly in AppDomain.CurrentDomain.GetAssemblies())
        {
            TryPatch(assembly);
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

    private static void TryPatch(Assembly assembly)
    {
        if (assembly.IsDynamic || assembly.GetName().Name is not { } name || !_byAssembly.TryGetValue(name, out var strings))
        {
            return;
        }
        lock (PatchLock)
        {
            if (!Patched.Add(name))
            {
                return;
            }
        }
        var report = new AssemblyReport { Translations = strings.Count };
        Reports[name] = report;
        var watch = Stopwatch.StartNew();
        try
        {
            // First read every method: a literal used as a key anywhere must
            // stay as it is everywhere, or the code stops matching its own text.
            var keys = new HashSet<string>(StringComparer.Ordinal);
            var usingMethods = new List<(MethodBase Method, HashSet<string> Loaded)>();
            foreach (var method in Methods(assembly))
            {
                var loaded = new HashSet<string>(StringComparer.Ordinal);
                try
                {
                    KeyStrings.Scan(method, strings, loaded, keys);
                }
                catch (Exception exception)
                {
                    // A body that cannot be read (missing dependency) is skipped, not fatal.
                    report.Unreadable++;
                    if (report.FailedMethods.Count < 20)
                    {
                        report.FailedMethods.Add($"{method.DeclaringType?.FullName}.{method.Name}: {exception.Message}");
                    }
                    continue;
                }
                if (loaded.Count > 0)
                {
                    usingMethods.Add((method, loaded));
                }
            }
            foreach (var kept in KeyStrings.WithContainingText(keys, strings.Keys.ToList()).Where(strings.ContainsKey))
            {
                // The transpiler reads this same dictionary.
                strings.Remove(kept);
                report.ProtectedStrings.Add(kept);
            }
            report.Protected = report.ProtectedStrings.Count;
            report.ProtectedStrings.Sort(StringComparer.Ordinal);
            if (report.ProtectedStrings.Count > MaxListedProtected)
            {
                report.ProtectedStrings.RemoveRange(MaxListedProtected, report.ProtectedStrings.Count - MaxListedProtected);
            }

            var found = new HashSet<string>(StringComparer.Ordinal);
            foreach (var (method, loaded) in usingMethods)
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
        }
        catch (Exception exception)
        {
            Errors.Add($"{name}: {exception.Message}");
        }
        report.Milliseconds = watch.ElapsedMilliseconds;
        BridgeMod.Log.Info(
            $"[{BridgeMod.ModId}] {name}: replaced {report.Found}/{report.Translations} DLL strings in {report.Methods} methods ({report.Milliseconds} ms, kept {report.Protected} used as keys, failed {report.Failed}, generic {report.SkippedGeneric}, unreadable {report.Unreadable})");
    }

    private static IEnumerable<MethodBase> Methods(Assembly assembly)
    {
        Type?[] types;
        try
        {
            types = assembly.GetTypes();
        }
        catch (ReflectionTypeLoadException exception)
        {
            types = exception.Types;
        }
        foreach (var type in types)
        {
            if (type == null)
            {
                continue;
            }
            foreach (var method in type.GetMethods(AllDeclared))
            {
                if (!method.IsAbstract)
                {
                    yield return method;
                }
            }
            foreach (var constructor in type.GetConstructors(AllDeclared))
            {
                yield return constructor;
            }
            if (type.TypeInitializer is { } initializer)
            {
                yield return initializer;
            }
        }
    }

    private static IEnumerable<CodeInstruction> Transpiler(IEnumerable<CodeInstruction> instructions, MethodBase __originalMethod)
    {
        var name = __originalMethod.Module.Assembly.GetName().Name;
        _byAssembly.TryGetValue(name ?? "", out var strings);
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

    public sealed class AssemblyReport
    {
        /// <summary>Strings the app sent for this assembly.</summary>
        public int Translations { get; set; }

        /// <summary>Of those, strings found in patched methods.</summary>
        public int Found { get; set; }

        /// <summary>Strings left untranslated because the code compares or searches with them.</summary>
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
