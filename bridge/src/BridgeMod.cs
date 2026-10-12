using HarmonyLib;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;

namespace Sts2ModManagerBridge;

/// <summary>
/// Entry point the game's ModManager calls after loading this assembly.
/// Every hook is wrapped so a game update that renames an API turns into a
/// logged error in the status file instead of a crash.
/// </summary>
[ModInitializer(nameof(Initialize))]
public static class BridgeMod
{
    public const string ModId = "STS2ModManagerBridge";

    public static Logger Log { get; } = new(ModId, LogType.Generic);

    public static string Version =>
        typeof(BridgeMod).Assembly.GetName().Version?.ToString(3) ?? "0.0.0";

    public static void Initialize()
    {
        try
        {
            new Harmony(ModId).PatchAll(typeof(BridgeMod).Assembly);
            ModErrorFilter.DropBranchErrorsOfDisabledMods();
            Log.Info($"[{ModId}] {Version} initialized, shared folder: {SharedPaths.Root}");
        }
        catch (Exception exception)
        {
            Log.Error($"[{ModId}] init failed: {exception}");
            Status.RecordError("init", exception.Message);
            Status.Write();
        }
    }
}
