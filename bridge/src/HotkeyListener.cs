using Godot;

namespace Sts2ModManagerBridge;

/// <summary>
/// Lives under the scene root for the whole session and opens the edit
/// panel on F8. Opening it re-reads the shared folder, so F8 also picks up
/// what the app just applied. Ctrl+F8 toggles the <c>[key]</c> prefixes
/// without opening the panel.
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
        if (@event is not InputEventKey { Pressed: true, Echo: false, Keycode: Key.F8 } key
            || key.AltPressed || key.ShiftPressed)
        {
            return;
        }
        GetViewport().SetInputAsHandled();
        if (key.CtrlPressed)
        {
            Injector.ShowKeys = !Injector.ShowKeys;
            Injector.Reload();
            return;
        }
        _panel?.Toggle();
    }
}
