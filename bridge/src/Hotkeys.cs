using System.Text.Json;
using Godot;

namespace Sts2ModManagerBridge;

public enum HotkeyAction
{
    /// <summary>Opens or closes the edit panel (works anywhere in game).</summary>
    TogglePanel,

    /// <summary>Toggles the <c>(#number)</c> prefixes (works anywhere in game).</summary>
    ToggleNumbers,

    /// <summary>Saves the translation being edited (panel only).</summary>
    Save,

    /// <summary>Copies the grammar check prompt (panel only).</summary>
    CopyCheckPrompt,
}

/// <summary>
/// User-changeable key bindings, kept in the shared folder so they survive
/// reinstalling the mod. A binding is a Godot keycode combined with its
/// modifier masks (<see cref="InputEventKey.GetKeycodeWithModifiers"/>).
/// </summary>
public static class Hotkeys
{
    private const Key CodeMask = (Key)KeyModifierMask.CodeMask;
    private const Key Ctrl = (Key)KeyModifierMask.MaskCtrl;
    private const Key Alt = (Key)KeyModifierMask.MaskAlt;
    private const Key Shift = (Key)KeyModifierMask.MaskShift;
    private const Key Meta = (Key)KeyModifierMask.MaskMeta;

    private static readonly Dictionary<HotkeyAction, Key> Defaults = new()
    {
        [HotkeyAction.TogglePanel] = Key.F8,
        [HotkeyAction.ToggleNumbers] = Key.F8 | Ctrl,
        [HotkeyAction.Save] = Key.Enter | Ctrl,
        [HotkeyAction.CopyCheckPrompt] = Key.C | Ctrl | Shift,
    };

    private static readonly Dictionary<HotkeyAction, Key> Current = Load();

    public static IEnumerable<HotkeyAction> Actions => Enum.GetValues<HotkeyAction>();

    public static string Label(HotkeyAction action) => action switch
    {
        HotkeyAction.TogglePanel => "패널 열기/닫기",
        HotkeyAction.ToggleNumbers => "화면에 #번호 표시",
        HotkeyAction.Save => "저장하고 적용",
        HotkeyAction.CopyCheckPrompt => "검수 프롬프트 복사",
        _ => action.ToString(),
    };

    public static Key Get(HotkeyAction action) => Current[action];

    public static string Text(HotkeyAction action) => Format(Current[action]);

    /// <summary>Whether a key press is the binding of <paramref name="action"/>.</summary>
    public static bool Matches(InputEventKey key, HotkeyAction action) => Combo(key) == Current[action];

    /// <summary>The pressed key with its modifiers; keypad Enter counts as Enter.</summary>
    public static Key Combo(InputEventKey key)
    {
        var combo = key.GetKeycodeWithModifiers();
        return (combo & CodeMask) == Key.KpEnter ? (combo & ~CodeMask) | Key.Enter : combo;
    }

    /// <summary>Modifier keys alone are not bindings; capture waits for the real key.</summary>
    public static bool IsModifierOnly(InputEventKey key) =>
        key.Keycode is Key.Ctrl or Key.Shift or Key.Alt or Key.Meta or Key.Capslock;

    /// <summary>
    /// Binds <paramref name="combo"/> to <paramref name="action"/>. Returns
    /// why it was refused, or null once saved.
    /// </summary>
    public static string? Set(HotkeyAction action, Key combo)
    {
        var code = combo & CodeMask;
        var modified = (combo & (Ctrl | Alt | Meta)) != 0;
        if (code == Key.Escape)
        {
            return "Esc는 패널 닫기 전용입니다.";
        }
        if (!modified && !IsFunctionKey(code))
        {
            // A plain letter would fire while typing a translation or a search.
            return "Ctrl이나 Alt를 함께 누르거나 F1~F12 같은 기능 키를 쓰세요.";
        }
        foreach (var other in Actions)
        {
            if (other != action && Current[other] == combo)
            {
                return $"'{Label(other)}'에서 이미 쓰는 키입니다.";
            }
        }
        Current[action] = combo;
        Persist();
        return null;
    }

    public static void ResetAll()
    {
        foreach (var (action, combo) in Defaults)
        {
            Current[action] = combo;
        }
        Persist();
    }

    public static string Format(Key combo)
    {
        var parts = new List<string>();
        if ((combo & Ctrl) != 0)
        {
            parts.Add("Ctrl");
        }
        if ((combo & Alt) != 0)
        {
            parts.Add("Alt");
        }
        if ((combo & Shift) != 0)
        {
            parts.Add("Shift");
        }
        if ((combo & Meta) != 0)
        {
            parts.Add("Meta");
        }
        parts.Add(OS.GetKeycodeString(combo & CodeMask));
        return string.Join("+", parts);
    }

    /// <summary>Reads <c>Ctrl+Shift+C</c>-style text; null when it is not a key.</summary>
    public static Key? Parse(string text)
    {
        var combo = Key.None;
        var parts = text.Split('+', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (parts.Length == 0)
        {
            return null;
        }
        foreach (var modifier in parts[..^1])
        {
            Key? mask = modifier.ToLowerInvariant() switch
            {
                "ctrl" or "control" => Ctrl,
                "alt" => Alt,
                "shift" => Shift,
                "meta" or "win" or "cmd" => Meta,
                _ => null,
            };
            if (mask == null)
            {
                return null;
            }
            combo |= mask.Value;
        }
        var code = OS.FindKeycodeFromString(parts[^1]);
        return code is Key.None or Key.Unknown ? null : combo | code;
    }

    private static bool IsFunctionKey(Key code) => code is >= Key.F1 and <= Key.F35
        or Key.Insert or Key.Pause or Key.Print or Key.Scrolllock;

    private static Dictionary<HotkeyAction, Key> Load()
    {
        var bindings = new Dictionary<HotkeyAction, Key>(Defaults);
        try
        {
            if (!File.Exists(SharedPaths.HotkeysFile))
            {
                return bindings;
            }
            var saved = JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(SharedPaths.HotkeysFile));
            foreach (var action in Enum.GetValues<HotkeyAction>())
            {
                if (saved != null && saved.TryGetValue(FileName(action), out var text) && Parse(text) is { } combo)
                {
                    bindings[action] = combo;
                }
            }
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] hotkeys read failed: {exception.Message}");
        }
        return bindings;
    }

    private static void Persist()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SharedPaths.HotkeysFile)!);
            var content = Actions.ToDictionary(FileName, Text);
            var temp = SharedPaths.HotkeysFile + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(content, new JsonSerializerOptions { WriteIndented = true }));
            File.Move(temp, SharedPaths.HotkeysFile, overwrite: true);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] hotkeys write failed: {exception.Message}");
        }
    }

    private static string FileName(HotkeyAction action) => action switch
    {
        HotkeyAction.TogglePanel => "toggle_panel",
        HotkeyAction.ToggleNumbers => "toggle_numbers",
        HotkeyAction.Save => "save",
        HotkeyAction.CopyCheckPrompt => "copy_check_prompt",
        _ => action.ToString(),
    };
}
