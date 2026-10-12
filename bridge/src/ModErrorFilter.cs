using MegaCrit.Sts2.Core.Modding;

namespace Sts2ModManagerBridge;

/// <summary>
/// The game checks Steam branch support for every workshop mod before it
/// drops the ones disabled in settings, so a mod the player switched off still
/// shows "only supports the public branch" on the main menu. Those errors are
/// removed from disabled mods; mods that are on keep them.
/// </summary>
public static class ModErrorFilter
{
    private const string BranchErrorKey = "MOD_ERROR.STEAM_BRANCH_UNSUPPORTED";

    public static void DropBranchErrorsOfDisabledMods()
    {
        var removed = 0;
        foreach (var mod in ModManager.Mods)
        {
            if (mod.state != ModLoadState.Disabled || mod.errors == null)
            {
                continue;
            }
            removed += mod.errors.RemoveAll(error => error.LocEntryKey == BranchErrorKey);
            if (mod.errors.Count == 0)
            {
                mod.errors = null;
            }
        }
        if (removed > 0)
        {
            BridgeMod.Log.Info($"[{BridgeMod.ModId}] hid {removed} branch errors of disabled mods");
        }
    }
}
