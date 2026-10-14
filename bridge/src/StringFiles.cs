using System.Reflection;
using System.Reflection.Emit;
using System.Text.Json;

namespace Sts2ModManagerBridge;

/// <summary>One DLL string the app sent: its translation is empty when only offered for in-game editing.</summary>
public sealed record DllEntry(string ModId, string Assembly, string Original, string Translation, string? Id)
{
    /// <summary>The edit log table of this string.</summary>
    public string Table => DllTables.Prefix + Assembly;
}

public static class DllTables
{
    /// <summary>Edit log tables of DLL strings are <c>dll:{assembly}</c>, keyed by the literal.</summary>
    public const string Prefix = "dll:";
}

/// <summary>
/// Reads <c>strings/{modId}/{lang}/{assembly}.json</c>: the DLL string
/// literals the app wants replaced (or offers for in-game editing), per
/// assembly. Pure file logic so it can be tested without the game.
/// </summary>
public static class StringFiles
{
    /// <summary>Every string of every mod for <paramref name="language"/>, with per-file read errors.</summary>
    public static (List<DllEntry> Entries, List<string> Errors) Load(string stringsRoot, string language)
    {
        var entries = new List<DllEntry>();
        var errors = new List<string>();
        if (!Directory.Exists(stringsRoot))
        {
            return (entries, errors);
        }
        foreach (var modDir in Directory.GetDirectories(stringsRoot).OrderBy(path => path, StringComparer.Ordinal))
        {
            var languageDir = Path.Combine(modDir, language);
            if (!Directory.Exists(languageDir))
            {
                continue;
            }
            var modId = Path.GetFileName(modDir);
            foreach (var file in Directory.GetFiles(languageDir, "*.json").OrderBy(path => path, StringComparer.Ordinal))
            {
                var assembly = Path.GetFileNameWithoutExtension(file);
                if (assembly.StartsWith('_'))
                {
                    continue; // _meta.json
                }
                try
                {
                    foreach (var (original, translation, id) in ReadStrings(file))
                    {
                        entries.Add(new DllEntry(modId, assembly, original, translation, id));
                    }
                }
                catch (Exception exception)
                {
                    errors.Add($"{modId}/{language}/{assembly}: {exception.Message}");
                }
            }
        }
        return (entries, errors);
    }

    /// <summary>Assembly name (case-insensitive) → original → translation, for the strings that have one.</summary>
    public static Dictionary<string, Dictionary<string, string>> Replacements(IEnumerable<(DllEntry Entry, string Value)> values)
    {
        var assemblies = new Dictionary<string, Dictionary<string, string>>(StringComparer.OrdinalIgnoreCase);
        foreach (var (entry, value) in values)
        {
            if (!assemblies.TryGetValue(entry.Assembly, out var strings))
            {
                strings = new Dictionary<string, string>(StringComparer.Ordinal);
                assemblies[entry.Assembly] = strings;
            }
            if (value.Length > 0)
            {
                strings[entry.Original] = value;
            }
        }
        return assemblies;
    }

    /// <summary><c>{ "strings": [ { "original", "translation", "id" } ] }</c>; the translation may be empty.</summary>
    public static IEnumerable<(string Original, string Translation, string? Id)> ReadStrings(string file)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(file));
        if (!document.RootElement.TryGetProperty("strings", out var list) || list.ValueKind != JsonValueKind.Array)
        {
            yield break;
        }
        foreach (var item in list.EnumerateArray())
        {
            if (item.TryGetProperty("original", out var original) && original.GetString() is { Length: > 0 } from)
            {
                var to = item.TryGetProperty("translation", out var translation) ? translation.GetString() ?? "" : "";
                var id = item.TryGetProperty("id", out var idValue) && idValue.ValueKind == JsonValueKind.String
                    ? idValue.GetString()
                    : null;
                yield return (from, to, id);
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
