using Godot;

namespace Sts2ModManagerBridge;

/// <summary>
/// Lives under the scene root for the whole session and opens the edit
/// panel on its hotkey (F8 by default). Opening it re-reads the shared
/// folder, so it also picks up what the app just applied. The numbers hotkey
/// (Ctrl+F8) toggles the <c>(#number)</c> prefixes without opening the panel.
/// Both are changeable in the panel (<see cref="Hotkeys"/>).
/// </summary>
public partial class HotkeyListener : Node
{
    private const string NodeName = "STS2ModManagerBridgeHotkeys";
    private EditPanel? _panel;

    public static void Attach(Node anyNode)
    {
        var root = anyNode.GetTree()?.Root;
        if (root == null || root.HasNode(NodeName))
        {
            return;
        }
        var listener = new HotkeyListener { Name = NodeName };
        // The root is busy adding the main menu while _Ready runs.
        root.CallDeferred(Node.MethodName.AddChild, listener);
    }

    public override void _Ready()
    {
        _panel = new EditPanel();
        AddChild(_panel);
    }

    public override void _Input(InputEvent @event)
    {
        if (@event is not InputEventKey { Pressed: true, Echo: false } key)
        {
            return;
        }
        if (Hotkeys.Matches(key, HotkeyAction.ToggleNumbers))
        {
            GetViewport().SetInputAsHandled();
            Injector.ShowKeys = !Injector.ShowKeys;
            Injector.Reload();
        }
        else if (Hotkeys.Matches(key, HotkeyAction.TogglePanel))
        {
            GetViewport().SetInputAsHandled();
            _panel?.Toggle();
        }
    }
}
