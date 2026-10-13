using System.Reflection;
using System.Reflection.Emit;
using System.Text.Json;

namespace Sts2ModManagerBridge;

/// <summary>
/// Reads <c>strings/{modId}/{lang}/{assembly}.json</c>: the DLL string
/// literals the app wants replaced, per assembly. Pure file logic so it can be
/// tested without the game.
/// </summary>
public static class StringFiles
{
    /// <summary>Assembly name (case-insensitive) → original → translation, with per-file read errors.</summary>
    public static (Dictionary<string, Dictionary<string, string>> Assemblies, List<string> Errors) Load(
        string stringsRoot,
        string language)
    {
        var assemblies = new Dictionary<string, Dictionary<string, string>>(StringComparer.OrdinalIgnoreCase);
        var errors = new List<string>();
        if (!Directory.Exists(stringsRoot))
        {
            return (assemblies, errors);
        }
        foreach (var modDir in Directory.GetDirectories(stringsRoot).OrderBy(path => path, StringComparer.Ordinal))
        {
            var languageDir = Path.Combine(modDir, language);
            if (!Directory.Exists(languageDir))
            {
                continue;
            }
            foreach (var file in Directory.GetFiles(languageDir, "*.json").OrderBy(path => path, StringComparer.Ordinal))
            {
                var assembly = Path.GetFileNameWithoutExtension(file);
                try
                {
                    if (!assemblies.TryGetValue(assembly, out var strings))
                    {
                        strings = new Dictionary<string, string>(StringComparer.Ordinal);
                        assemblies[assembly] = strings;
                    }
                    foreach (var (original, translation) in ReadStrings(file))
                    {
                        strings[original] = translation;
                    }
                }
                catch (Exception exception)
                {
                    errors.Add($"{Path.GetFileName(modDir)}/{language}/{assembly}: {exception.Message}");
                }
            }
        }
        return (assemblies, errors);
    }

    /// <summary><c>{ "strings": [ { "original", "translation" } ] }</c></summary>
    public static IEnumerable<(string Original, string Translation)> ReadStrings(string file)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(file));
        if (!document.RootElement.TryGetProperty("strings", out var list) || list.ValueKind != JsonValueKind.Array)
        {
            yield break;
        }
        foreach (var item in list.EnumerateArray())
        {
            if (item.TryGetProperty("original", out var original)
                && item.TryGetProperty("translation", out var translation)
                && original.GetString() is { Length: > 0 } from
                && translation.GetString() is { Length: > 0 } to)
            {
                yield return (from, to);
            }
        }
    }
}

/// <summary>
/// Finds the string literals (<c>ldstr</c>) a method's IL loads, by walking
/// its body with the standard opcode table. Lets the bridge patch only the
/// methods that actually use a string it replaces.
/// </summary>
public static class IlStrings
{
    private static readonly OpCode[] OneByte = new OpCode[0x100];
    private static readonly OpCode[] TwoByte = new OpCode[0x100];

    static IlStrings()
    {
        foreach (var field in typeof(OpCodes).GetFields(BindingFlags.Public | BindingFlags.Static))
        {
            if (field.GetValue(null) is not OpCode opcode)
            {
                continue;
            }
            var value = (ushort)opcode.Value;
            if (value < 0x100)
            {
                OneByte[value] = opcode;
            }
            else if ((value & 0xff00) == 0xfe00)
            {
                TwoByte[value & 0xff] = opcode;
            }
        }
    }

    /// <summary>Strings loaded by <paramref name="method"/>; empty when it has no IL body.</summary>
    public static IEnumerable<string> Of(MethodBase method)
    {
        var il = method.GetMethodBody()?.GetILAsByteArray();
        if (il == null)
        {
            yield break;
        }
        foreach (var instruction in Instructions(il))
        {
            if (instruction.OpCode == OpCodes.Ldstr && StringOperand(method, il, instruction) is { } value)
            {
                yield return value;
            }
        }
    }

    /// <summary>The opcodes of an IL body with where each operand starts.</summary>
    public static List<IlInstruction> Instructions(byte[] il)
    {
        var instructions = new List<IlInstruction>();
        var position = 0;
        while (position < il.Length)
        {
            OpCode opcode;
            if (il[position] == 0xfe && position + 1 < il.Length)
            {
                opcode = TwoByte[il[position + 1]];
                position += 2;
            }
            else
            {
                opcode = OneByte[il[position]];
                position += 1;
            }
            instructions.Add(new IlInstruction(opcode, position));
            position += OperandSize(opcode, il, position);
        }
        return instructions;
    }

    public static string? StringOperand(MethodBase method, byte[] il, IlInstruction instruction)
    {
        if (instruction.OperandPosition + 4 > il.Length)
        {
            return null;
        }
        try
        {
            return method.Module.ResolveString(BitConverter.ToInt32(il, instruction.OperandPosition));
        }
        catch (ArgumentException)
        {
            // Not a user-string token; skip it like the runtime would reject it.
            return null;
        }
    }

    /// <summary>The method a call/callvirt/newobj targets; null when it cannot be resolved.</summary>
    public static MethodBase? MethodOperand(MethodBase method, byte[] il, IlInstruction instruction)
    {
        if (instruction.OperandPosition + 4 > il.Length)
        {
            return null;
        }
        try
        {
            var typeArguments = method.DeclaringType?.IsGenericType == true ? method.DeclaringType.GetGenericArguments() : null;
            var methodArguments = method.IsGenericMethod ? method.GetGenericArguments() : null;
            return method.Module.ResolveMethod(BitConverter.ToInt32(il, instruction.OperandPosition), typeArguments, methodArguments);
        }
        catch (Exception)
        {
            // A missing dependency; the call simply tells us nothing.
            return null;
        }
    }

    private static int OperandSize(OpCode opcode, byte[] il, int position) => opcode.OperandType switch
    {
        OperandType.InlineNone => 0,
        OperandType.ShortInlineBrTarget or OperandType.ShortInlineI or OperandType.ShortInlineVar => 1,
        OperandType.InlineVar => 2,
        OperandType.InlineI8 or OperandType.InlineR => 8,
        OperandType.InlineSwitch when position + 4 <= il.Length => 4 + 4 * BitConverter.ToInt32(il, position),
        _ => 4,
    };
}

public readonly record struct IlInstruction(OpCode OpCode, int OperandPosition);

/// <summary>
/// Finds string literals the code compares or searches with instead of
/// showing: <c>switch</c> cases, <c>text.Contains("…")</c>, dictionary keys,
/// regex patterns. Translating one of those breaks the mod (a parser no
/// longer recognises its own words), so the bridge leaves them alone, along
/// with any text that contains one, since that text is what gets parsed.
/// </summary>
public static class KeyStrings
{
    /// <summary>How far past a literal to look for the call that consumes it.</summary>
    private const int Lookahead = 5;

    private static readonly HashSet<string> StringLookups = new(StringComparer.Ordinal)
    {
        "Equals", "op_Equality", "op_Inequality", "Compare", "CompareOrdinal", "CompareTo",
        "Contains", "StartsWith", "EndsWith", "IndexOf", "LastIndexOf", "Replace", "Split",
    };

    private static readonly HashSet<string> CollectionLookups = new(StringComparer.Ordinal)
    {
        "ContainsKey", "TryGetValue", "get_Item", "set_Item", "Contains", "Remove", "TryAdd", "Add",
        "GetValueOrDefault", "IndexOf",
    };

    private static readonly string[] KeyedCollections =
    {
        "System.Collections.Generic.Dictionary`2", "System.Collections.Generic.IDictionary`2",
        "System.Collections.Generic.IReadOnlyDictionary`2", "System.Collections.Generic.SortedDictionary`2",
        "System.Collections.Generic.HashSet`1", "System.Collections.Generic.ISet`1",
        "System.Collections.Generic.IReadOnlySet`1", "System.Collections.Generic.SortedSet`1",
        "System.Collections.Concurrent.ConcurrentDictionary`2", "System.Collections.Frozen.FrozenDictionary`2",
        "System.Collections.Frozen.FrozenSet`1",
    };

    /// <summary>
    /// Adds the <paramref name="candidates"/> <paramref name="method"/>
    /// loads to <paramref name="loaded"/> and those it uses as keys to
    /// <paramref name="keys"/>.
    /// </summary>
    public static void Scan(MethodBase method, IReadOnlyDictionary<string, string> candidates, ISet<string> loaded, ISet<string> keys)
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
            if (instruction.OpCode != OpCodes.Ldstr
                || IlStrings.StringOperand(method, il, instruction) is not { } value
                || !candidates.ContainsKey(value))
            {
                continue;
            }
            loaded.Add(value);
            if (index + 1 < instructions.Count && instructions[index + 1].OpCode == OpCodes.Stelem_Ref)
            {
                storedInArrays.Add(value);
            }
            if (ConsumedByLookup(method, il, instructions, index))
            {
                keys.Add(value);
            }
        }
        // `foreach (var pattern in new[] { "…", "…" }) Regex.Replace(text, pattern, "")`
        if (searchesText)
        {
            keys.UnionWith(storedInArrays);
        }
    }

    /// <summary>The keys plus every candidate that contains one.</summary>
    public static HashSet<string> WithContainingText(ISet<string> keys, IEnumerable<string> candidates)
    {
        var result = new HashSet<string>(keys, StringComparer.Ordinal);
        var parts = keys.Where(key => key.Length >= 2).ToList();
        foreach (var candidate in candidates)
        {
            if (!result.Contains(candidate) && parts.Any(key => candidate.Contains(key, StringComparison.Ordinal)))
            {
                result.Add(candidate);
            }
        }
        return result;
    }

    /// <summary>The first call after the literal, past plain argument loads, is a lookup.</summary>
    private static bool ConsumedByLookup(MethodBase method, byte[] il, List<IlInstruction> instructions, int index)
    {
        var end = Math.Min(instructions.Count, index + 1 + Lookahead);
        for (var next = index + 1; next < end; next++)
        {
            var instruction = instructions[next];
            if (IsCall(instruction.OpCode))
            {
                return IlStrings.MethodOperand(method, il, instruction) is { } called && IsLookup(called);
            }
            if (!IsArgumentLoad(instruction.OpCode))
            {
                return false;
            }
        }
        return false;
    }

    private static bool IsCall(OpCode opcode) =>
        opcode == OpCodes.Call || opcode == OpCodes.Callvirt || opcode == OpCodes.Newobj;

    private static bool IsArgumentLoad(OpCode opcode)
    {
        var name = opcode.Name ?? "";
        return name.StartsWith("ldarg", StringComparison.Ordinal)
            || name.StartsWith("ldloc", StringComparison.Ordinal)
            || name.StartsWith("ldc.", StringComparison.Ordinal)
            || name.StartsWith("conv.", StringComparison.Ordinal)
            || name is "ldnull" or "ldstr" or "ldfld" or "ldflda" or "ldsfld" or "ldsflda" or "box" or "dup";
    }

    private static bool IsLookup(MethodBase called)
    {
        var type = called.DeclaringType;
        if (type == null)
        {
            return false;
        }
        var name = called.Name;
        var fullName = type.FullName ?? "";
        if (fullName == "System.String")
        {
            return StringLookups.Contains(name);
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
        var definition = type.IsGenericType ? type.GetGenericTypeDefinition().FullName ?? "" : fullName;
        if (KeyedCollections.Contains(definition))
        {
            return CollectionLookups.Contains(name);
        }
        // `list.Contains("…")` is a membership test; `list.Add("…")` is not.
        return definition is "System.Collections.Generic.List`1" or "System.Collections.Generic.ICollection`1"
            && name is "Contains" or "IndexOf" or "Remove";
    }

    private static bool IsTextSearch(MethodBase called)
    {
        var fullName = called.DeclaringType?.FullName ?? "";
        return fullName.StartsWith("System.Text.RegularExpressions.", StringComparison.Ordinal)
            || (fullName == "System.String"
                && called.Name is "Contains" or "IndexOf" or "Replace" or "Split" or "StartsWith" or "EndsWith");
    }
}
