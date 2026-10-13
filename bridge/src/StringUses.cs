using System.Reflection;
using System.Reflection.Emit;
using System.Text.Json;

namespace Sts2ModManagerBridge;

/// <summary>
/// What a mod's code does with a string literal, judged by the call that
/// consumes it. Later values win when a literal is used several ways: a word
/// that is shown in one place and compared in another must stay as it is.
/// </summary>
public enum StringUse
{
    /// <summary>Stored, returned or passed to the mod's own code: could be anything.</summary>
    Unknown,

    /// <summary>Set straight on a Godot control (<c>label.Text = "…"</c>).</summary>
    Shown,
    Log,
    Exception,

    /// <summary>A loc table name or key (<c>new LocString("cards", "…")</c>).</summary>
    Loc,

    /// <summary>A node, signal, resource, input action or file path.</summary>
    Node,

    /// <summary>A type, method or field name looked up by reflection or Harmony.</summary>
    Reflection,

    /// <summary>Compared as a whole (<c>==</c>, <c>switch</c>) or a dictionary key.</summary>
    Key,

    /// <summary>Text containing a <see cref="SearchedFor"/> word: what the code parses.</summary>
    ContainsKey,

    /// <summary>Searched for inside other text (<c>Contains</c>, <c>Replace</c>) or a regex pattern.</summary>
    SearchedFor,
}

/// <summary>Classifies the string literals of methods by how they are used.</summary>
public static class StringUses
{
    /// <summary>How many instructions to follow a literal before giving up.</summary>
    private const int MaxFollow = 60;

    /// <summary>Uses that break the mod when the text changes.</summary>
    public static bool BreaksWhenTranslated(StringUse use) =>
        use is StringUse.Key or StringUse.SearchedFor or StringUse.ContainsKey or StringUse.Reflection or StringUse.Node
            or StringUse.Loc;

    /// <summary>
    /// Records the use of every literal <paramref name="method"/> loads;
    /// <paramref name="loaded"/> gets them all.
    /// </summary>
    public static void Scan(MethodBase method, IDictionary<string, StringUse> uses, ISet<string>? loaded)
    {
        var il = method.GetMethodBody()?.GetILAsByteArray();
        if (il == null)
        {
            return;
        }
        var instructions = IlStrings.Instructions(il);
        var storedInArrays = new List<string>();
        var searchesText = false;
        for (var index = 0; index < instructions.Count; index++)
        {
            var instruction = instructions[index];
            if (IsCall(instruction.OpCode))
            {
                searchesText |= IlStrings.MethodOperand(method, il, instruction) is { } called && IsTextSearch(called);
                continue;
            }
            if (instruction.OpCode != OpCodes.Ldstr || IlStrings.StringOperand(method, il, instruction) is not { } value)
            {
                continue;
            }
            loaded?.Add(value);
            var (use, consumer) = Follow(method, il, instructions, index);
            if (consumer == OpCodes.Stelem_Ref)
            {
                storedInArrays.Add(value);
            }
            Merge(uses, value, use);
        }
        // `foreach (var pattern in new[] { "…", "…" }) text = Regex.Replace(text, pattern, "")`
        if (searchesText)
        {
            foreach (var value in storedInArrays)
            {
                Merge(uses, value, StringUse.SearchedFor);
            }
        }
    }

    /// <summary>
    /// Marks text containing a word the code searches for: the code parses
    /// that text, so it must not change either. Latin words only count as
    /// whole words of three letters or more; shorter ones (<c>"E"</c>,
    /// <c>"."</c>) turn up in every sentence.
    /// </summary>
    public static void MarkTextContainingKeys(IDictionary<string, StringUse> uses)
    {
        var searched = uses
            .Where(pair => pair.Value == StringUse.SearchedFor && IsDistinctiveWord(pair.Key))
            .Select(pair => pair.Key.Trim())
            .Distinct()
            .ToList();
        if (searched.Count == 0)
        {
            return;
        }
        var latin = searched.Where(word => word.All(character => character < 0x80)).ToList();
        var other = searched.Except(latin).ToList();
        var latinPattern = latin.Count == 0
            ? null
            : new System.Text.RegularExpressions.Regex(
                $@"(?<![A-Za-z0-9_])(?:{string.Join("|", latin.Select(System.Text.RegularExpressions.Regex.Escape))})(?![A-Za-z0-9_])");
        foreach (var text in uses.Where(pair => pair.Value < StringUse.ContainsKey).Select(pair => pair.Key).ToList())
        {
            if (other.Any(word => text.Contains(word, StringComparison.Ordinal)) || latinPattern?.IsMatch(text) == true)
            {
                uses[text] = StringUse.ContainsKey;
            }
        }
    }

    private static bool IsDistinctiveWord(string value)
    {
        var word = value.Trim();
        if (word.Any(character => character >= 0x80 && char.IsLetter(character)))
        {
            return word.Length >= 2;
        }
        return word.Count(char.IsLetter) >= 3;
    }

    private static void Merge(IDictionary<string, StringUse> uses, string value, StringUse use)
    {
        uses[value] = uses.TryGetValue(value, out var current) && current > use ? current : use;
    }

    /// <summary>
    /// Follows the literal on the evaluation stack to the instruction that
    /// takes it off. String building (<c>Concat</c>, interpolation) hands on
    /// a new string, which is followed in its place.
    /// </summary>
    private static (StringUse Use, OpCode? Consumer) Follow(MethodBase method, byte[] il, List<IlInstruction> instructions, int index)
    {
        // Values pushed after the literal that sit above it on the stack.
        var above = 0;
        var end = Math.Min(instructions.Count, index + 1 + MaxFollow);
        for (var next = index + 1; next < end; next++)
        {
            var instruction = instructions[next];
            var opcode = instruction.OpCode;
            MethodBase? called = null;
            int pops;
            int pushes;
            if (IsCall(opcode))
            {
                called = IlStrings.MethodOperand(method, il, instruction);
                if (called == null)
                {
                    return (StringUse.Unknown, opcode);
                }
                try
                {
                    pops = called.GetParameters().Length + (opcode != OpCodes.Newobj && !called.IsStatic ? 1 : 0);
                    pushes = opcode == OpCodes.Newobj || (called is MethodInfo info && info.ReturnType != typeof(void)) ? 1 : 0;
                }
                catch (Exception)
                {
                    // A parameter type from a missing assembly.
                    return (StringUse.Unknown, opcode);
                }
            }
            else if (opcode.FlowControl is FlowControl.Branch or FlowControl.Return or FlowControl.Throw
                     || opcode == OpCodes.Ret)
            {
                // `cond ? "a" : "b"` and the like: not followed across jumps.
                return (StringUse.Unknown, opcode);
            }
            else if (Pops(opcode) is { } fixedPops && Pushes(opcode) is { } fixedPushes)
            {
                pops = fixedPops;
                pushes = fixedPushes;
            }
            else
            {
                return (StringUse.Unknown, opcode);
            }
            if (pops <= above)
            {
                above += pushes - pops;
                continue;
            }
            // This instruction takes the literal (or what it became).
            if (called == null)
            {
                if (opcode == OpCodes.Dup)
                {
                    above += 1;
                    continue;
                }
                return (StringUse.Unknown, opcode);
            }
            if (IsStringBuilding(called))
            {
                if (called.DeclaringType == typeof(string))
                {
                    // `string.Concat(…)` hands on the new string.
                    above = 0;
                    continue;
                }
                // `$"…"` and StringBuilder collect pieces and hand on the
                // whole text later, from ToStringAndClear/ToString.
                var built = BuiltText(method, il, instructions, next + 1, end);
                if (built < 0)
                {
                    return (StringUse.Unknown, opcode);
                }
                next = built;
                above = 0;
                continue;
            }
            return (UseOf(called, opcode), opcode);
        }
        return (StringUse.Unknown, null);
    }

    /// <summary>The index of the call that turns the text being built into a string; -1 if none before a jump.</summary>
    private static int BuiltText(MethodBase method, byte[] il, List<IlInstruction> instructions, int from, int end)
    {
        for (var next = from; next < end; next++)
        {
            var instruction = instructions[next];
            if (instruction.OpCode.FlowControl is FlowControl.Branch or FlowControl.Cond_Branch or FlowControl.Return
                or FlowControl.Throw)
            {
                return -1;
            }
            // `sb.ToString()` compiles to a call of Object.ToString.
            if (IsCall(instruction.OpCode)
                && IlStrings.MethodOperand(method, il, instruction) is { Name: "ToStringAndClear" or "ToString" })
            {
                return next;
            }
        }
        return -1;
    }

    private static bool IsCall(OpCode opcode) =>
        opcode == OpCodes.Call || opcode == OpCodes.Callvirt || opcode == OpCodes.Newobj;

    private static int? Pops(OpCode opcode) => opcode.StackBehaviourPop switch
    {
        StackBehaviour.Pop0 => 0,
        StackBehaviour.Pop1 or StackBehaviour.Popi or StackBehaviour.Popref => 1,
        StackBehaviour.Pop1_pop1 or StackBehaviour.Popi_pop1 or StackBehaviour.Popi_popi or StackBehaviour.Popi_popi8
            or StackBehaviour.Popi_popr4 or StackBehaviour.Popi_popr8 or StackBehaviour.Popref_pop1
            or StackBehaviour.Popref_popi => 2,
        StackBehaviour.Popi_popi_popi or StackBehaviour.Popref_popi_popi or StackBehaviour.Popref_popi_popi8
            or StackBehaviour.Popref_popi_popr4 or StackBehaviour.Popref_popi_popr8 or StackBehaviour.Popref_popi_popref
            or StackBehaviour.Popref_popi_pop1 => 3,
        _ => null,
    };

    private static int? Pushes(OpCode opcode) => opcode.StackBehaviourPush switch
    {
        StackBehaviour.Push0 => 0,
        StackBehaviour.Push1 or StackBehaviour.Pushi or StackBehaviour.Pushi8 or StackBehaviour.Pushr4
            or StackBehaviour.Pushr8 or StackBehaviour.Pushref => 1,
        StackBehaviour.Push1_push1 => 2,
        _ => null,
    };

    private static bool IsStringBuilding(MethodBase called)
    {
        var fullName = called.DeclaringType?.FullName ?? "";
        return (fullName == "System.String"
                && called.Name is "Concat" or "Format" or "Join" or "Trim" or "TrimStart" or "TrimEnd" or "ToUpper"
                    or "ToLower" or "ToUpperInvariant" or "ToLowerInvariant" or "PadLeft" or "PadRight" or "Insert")
            || fullName.StartsWith("System.Runtime.CompilerServices.DefaultInterpolatedStringHandler", StringComparison.Ordinal)
            || fullName == "System.Text.StringBuilder"
            || fullName == "System.Text.CompositeFormat";
    }

    private static StringUse UseOf(MethodBase called, OpCode opcode)
    {
        var type = called.DeclaringType;
        if (type == null)
        {
            return StringUse.Unknown;
        }
        var name = called.Name;
        var fullName = type.FullName ?? "";
        var definition = type.IsGenericType ? type.GetGenericTypeDefinition().FullName ?? fullName : fullName;
        if (IsTextSearch(called))
        {
            return StringUse.SearchedFor;
        }
        if (IsLookup(fullName, definition, name))
        {
            return StringUse.Key;
        }
        if (opcode == OpCodes.Newobj && typeof(Exception).IsAssignableFrom(type))
        {
            return StringUse.Exception;
        }
        if (IsReflection(fullName, name))
        {
            return StringUse.Reflection;
        }
        if (IsLog(type, fullName, name))
        {
            return StringUse.Log;
        }
        if (type.Name is "LocString" or "LocManager" or "LocTable" || fullName.Contains(".Localization.", StringComparison.Ordinal))
        {
            return StringUse.Loc;
        }
        if (IsNodeOrPath(fullName, name))
        {
            return StringUse.Node;
        }
        if (fullName.StartsWith("Godot.", StringComparison.Ordinal) && IsControlText(name))
        {
            return StringUse.Shown;
        }
        return StringUse.Unknown;
    }

    private static bool IsLookup(string fullName, string definition, string name)
    {
        if (fullName == "System.String")
        {
            return name is "Equals" or "op_Equality" or "op_Inequality" or "Compare" or "CompareOrdinal" or "CompareTo"
                or "Contains" or "StartsWith" or "EndsWith" or "IndexOf" or "LastIndexOf" or "Replace" or "Split";
        }
        if (fullName.StartsWith("System.Text.RegularExpressions.", StringComparison.Ordinal))
        {
            return true;
        }
        if (fullName is "System.MemoryExtensions" or "System.Linq.Enumerable")
        {
            return name is "Equals" or "Contains" or "StartsWith" or "EndsWith" or "IndexOf" or "SequenceEqual";
        }
        if (fullName == "System.Collections.Generic.CollectionExtensions")
        {
            return name is "GetValueOrDefault" or "TryAdd" or "Remove";
        }
        if (KeyedCollections.Contains(definition))
        {
            return name is "ContainsKey" or "TryGetValue" or "get_Item" or "set_Item" or "Contains" or "Remove"
                or "TryAdd" or "Add" or "GetValueOrDefault" or "GetOrAdd" or "TryRemove";
        }
        // `list.Contains("…")` is a membership test; `list.Add("…")` is not.
        return definition is "System.Collections.Generic.List`1" or "System.Collections.Generic.ICollection`1"
            && name is "Contains" or "IndexOf" or "Remove";
    }

    private static readonly HashSet<string> KeyedCollections = new(StringComparer.Ordinal)
    {
        "System.Collections.Generic.Dictionary`2", "System.Collections.Generic.IDictionary`2",
        "System.Collections.Generic.IReadOnlyDictionary`2", "System.Collections.Generic.SortedDictionary`2",
        "System.Collections.Generic.HashSet`1", "System.Collections.Generic.ISet`1",
        "System.Collections.Generic.IReadOnlySet`1", "System.Collections.Generic.SortedSet`1",
        "System.Collections.Concurrent.ConcurrentDictionary`2", "System.Collections.Frozen.FrozenDictionary`2",
        "System.Collections.Frozen.FrozenSet`1",
    };

    private static bool IsReflection(string fullName, string name) =>
        fullName is "System.Type" or "System.Activator" or "System.AppDomain"
        || fullName.StartsWith("System.Reflection.", StringComparison.Ordinal)
        || fullName.StartsWith("HarmonyLib.", StringComparison.Ordinal)
        || (fullName == "System.Enum" && name is "Parse" or "TryParse" or "IsDefined");

    private static bool IsLog(Type type, string fullName, string name)
    {
        if (fullName is "Godot.GD")
        {
            return name.StartsWith("Print", StringComparison.Ordinal) || name.StartsWith("Push", StringComparison.Ordinal);
        }
        if (fullName is "System.Console" or "System.Diagnostics.Debug" or "System.Diagnostics.Trace")
        {
            return true;
        }
        var typeName = type.Name;
        var isLogger = typeName.Contains("Logger", StringComparison.Ordinal) || typeName is "Log" or "Logging"
            || typeName.EndsWith("Log", StringComparison.Ordinal);
        return isLogger
            && name is "Info" or "Warn" or "Warning" or "Error" or "Debug" or "Trace" or "Verbose" or "Fatal" or "Log"
                or "Msg" or "Print" or "Write" or "WriteLine" or "LogInfo" or "LogWarning" or "LogError" or "LogDebug"
                or "VeryDebug";
    }

    private static bool IsNodeOrPath(string fullName, string name)
    {
        if (fullName is "Godot.StringName" or "Godot.NodePath" or "Godot.ResourceLoader" or "Godot.ProjectSettings"
            or "Godot.FileAccess" or "Godot.DirAccess" or "Godot.Input" or "Godot.InputMap" or "Godot.ClassDB"
            or "Godot.Engine" or "Godot.OS")
        {
            return true;
        }
        if (fullName.StartsWith("System.IO.", StringComparison.Ordinal))
        {
            return true;
        }
        if (fullName == "Godot.GD")
        {
            return name.StartsWith("Load", StringComparison.Ordinal);
        }
        if (fullName.StartsWith("Godot.", StringComparison.Ordinal))
        {
            return name.StartsWith("GetNode", StringComparison.Ordinal)
                || name is "HasNode" or "FindChild" or "FindChildren" or "Connect" or "Disconnect" or "EmitSignal"
                    or "IsConnected" or "Call" or "CallDeferred" or "Callv" or "Get" or "Set" or "HasMethod"
                    or "HasSignal" or "SetMeta" or "GetMeta" or "HasMeta" or "RemoveMeta" or "AddToGroup"
                    or "IsInGroup" or "RemoveFromGroup" or "Play" or "PlayBackwards" or "HasAnimation"
                    or "GetAnimation" or "SetShaderParameter" or "GetShaderParameter" or "set_Name";
        }
        return false;
    }

    private static bool IsControlText(string name) =>
        name is "set_Text" or "set_TooltipText" or "AppendText" or "AddText" or "set_PlaceholderText" or "AddItem"
            or "SetItemText" or "set_Title" or "AddCheckItem" or "AddRadioCheckItem" or "AddIconItem"
            or "set_DialogText" or "SetTabTitle" or "ParseBbcode" or "set_OkButtonText" or "set_CancelButtonText"
            or "SetItemTooltip" or "AddIconCheckItem";

    private static bool IsTextSearch(MethodBase called)
    {
        var fullName = called.DeclaringType?.FullName ?? "";
        return fullName.StartsWith("System.Text.RegularExpressions.", StringComparison.Ordinal)
            || (fullName == "System.String"
                && called.Name is "Contains" or "IndexOf" or "Replace" or "Split" or "StartsWith" or "EndsWith");
    }
}

/// <summary>
/// Every method of an assembly read once: the use of each literal and which
/// methods load which literals.
/// </summary>
public sealed class AssemblyStrings
{
    private const BindingFlags AllDeclared =
        BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

    public Dictionary<string, StringUse> Uses { get; } = new(StringComparer.Ordinal);
    public List<(MethodBase Method, HashSet<string> Loaded)> Methods { get; } = new();
    public int Unreadable { get; private set; }
    public List<string> FailedMethods { get; } = new();

    public static AssemblyStrings Read(Assembly assembly)
    {
        var result = new AssemblyStrings();
        foreach (var method in AllMethods(assembly))
        {
            var loaded = new HashSet<string>(StringComparer.Ordinal);
            try
            {
                StringUses.Scan(method, result.Uses, loaded);
            }
            catch (Exception exception)
            {
                // A body that cannot be read (missing dependency) is skipped, not fatal.
                result.Unreadable++;
                if (result.FailedMethods.Count < 20)
                {
                    result.FailedMethods.Add($"{method.DeclaringType?.FullName}.{method.Name}: {exception.Message}");
                }
                continue;
            }
            if (loaded.Count > 0)
            {
                result.Methods.Add((method, loaded));
            }
        }
        StringUses.MarkTextContainingKeys(result.Uses);
        return result;
    }

    private static IEnumerable<MethodBase> AllMethods(Assembly assembly)
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
}

/// <summary>
/// <c>analysis/{file}.json</c>: how each literal of a mod DLL is used, for
/// the app to tell text worth translating from names, keys and logs.
/// Unknown uses are left out. Keyed by the DLL file name, which is what the
/// app's DLL sheet knows.
/// </summary>
public static class AnalysisFiles
{
    public const int Format = 1;

    public static string PathFor(string root, string fileStem) => Path.Combine(root, $"{fileStem}.json");

    /// <summary>Whether the file already describes this build of the assembly.</summary>
    public static bool IsCurrent(string root, string fileStem, Guid mvid)
    {
        try
        {
            var path = PathFor(root, fileStem);
            if (!File.Exists(path))
            {
                return false;
            }
            using var document = JsonDocument.Parse(File.ReadAllText(path));
            return document.RootElement.TryGetProperty("format", out var format) && format.GetInt32() == Format
                && document.RootElement.TryGetProperty("mvid", out var stored) && stored.GetString() == mvid.ToString()
                && document.RootElement.TryGetProperty("bridge_version", out var version) && version.GetString() == BridgeMod.Version;
        }
        catch (Exception)
        {
            return false;
        }
    }

    public static void Write(string root, string fileStem, string assemblyName, Guid mvid, long size, AssemblyStrings strings)
    {
        Directory.CreateDirectory(root);
        var uses = new SortedDictionary<string, string>(StringComparer.Ordinal);
        foreach (var (text, use) in strings.Uses)
        {
            if (use != StringUse.Unknown)
            {
                uses[text] = JsonNamingPolicy.SnakeCaseLower.ConvertName(use.ToString());
            }
        }
        var file = new
        {
            format = Format,
            assembly = assemblyName,
            mvid = mvid.ToString(),
            size,
            bridge_version = BridgeMod.Version,
            analyzed_at = DateTimeOffset.Now.ToString("o"),
            literals = strings.Uses.Count,
            unreadable_methods = strings.Unreadable,
            uses,
        };
        var path = PathFor(root, fileStem);
        var temp = path + ".tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(file, JsonOptions));
        File.Move(temp, path, overwrite: true);
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };
}
