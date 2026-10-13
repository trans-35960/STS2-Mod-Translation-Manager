namespace Sts2ModManagerBridge;

/// <summary>
/// The folder the app and this mod share. Its location is fixed so the mod
/// finds it no matter where the app runs from; it lives outside the mods
/// folder so reinstalling the mod never loses translations.
/// </summary>
public static class SharedPaths
{
    public const int SupportedFormat = 1;

    public static string Root { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "STS2ModTranslationManager",
        "shared");

    public static string Translations => Path.Combine(Root, "translations");

    public static string Strings => Path.Combine(Root, "strings");

    public static string Edits => Path.Combine(Root, "edits");

    /// <summary>How each mod DLL uses its string literals (written by this mod, read by the app).</summary>
    public static string Analysis => Path.Combine(Root, "analysis");

    public static string StatusFile => Path.Combine(Root, "status", "bridge.json");

    public static string FormatFile => Path.Combine(Root, "format.json");
}
