using HarmonyLib;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Nodes.Screens.MainMenu;

namespace Sts2ModManagerBridge;

/// <summary>
/// The game rebuilds every loc table (its own and every mod's) inside
/// SetLanguage. Running last after it makes our values the final ones.
/// </summary>
[HarmonyPatch(typeof(LocManager), nameof(LocManager.SetLanguage))]
public static class SetLanguagePatch
{
    [HarmonyPriority(Priority.Last)]
    public static void Postfix(LocManager __instance)
    {
        try
        {
            Injector.OnTablesRebuilt(__instance);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] SetLanguage hook failed: {exception.Message}");
        }
    }
}

/// <summary>
/// The first SetLanguage at boot can run before other mods have loaded their
/// tables; by the time the main menu is ready they all have. This is also
/// where the F8 listener joins the scene tree.
/// </summary>
[HarmonyPatch(typeof(NMainMenu), "_Ready")]
public static class MainMenuReadyPatch
{
    public static void Postfix(NMainMenu __instance)
    {
        try
        {
            Injector.Reload();
            HotkeyListener.Attach(__instance);
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] main menu hook failed: {exception.Message}");
        }
    }
}
