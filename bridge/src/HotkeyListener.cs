using Godot;

namespace Sts2ModManagerBridge;

/// <summary>
/// Lives under the scene root for the whole session and reloads the shared
/// translations on F8, showing a short notice so the player knows it worked.
/// </summary>
public partial class HotkeyListener : CanvasLayer
{
    private const string NodeName = "STS2ModManagerBridgeHotkeys";
    private Label? _notice;

    public static void Attach(Node anyNode)
    {
        var root = anyNode.GetTree()?.Root;
        if (root == null || root.HasNode(NodeName))
        {
            return;
        }
        var listener = new HotkeyListener { Name = NodeName, Layer = 128 };
        // The root is busy adding the main menu while _Ready runs.
        root.CallDeferred(Node.MethodName.AddChild, listener);
    }

    public override void _Ready()
    {
        _notice = new Label
        {
            Visible = false,
            Position = new Vector2(24, 24),
        };
        _notice.AddThemeColorOverride("font_color", Colors.White);
        _notice.AddThemeColorOverride("font_outline_color", Colors.Black);
        _notice.AddThemeConstantOverride("outline_size", 6);
        _notice.AddThemeFontSizeOverride("font_size", 22);
        AddChild(_notice);
    }

    public override void _Input(InputEvent @event)
    {
        if (@event is not InputEventKey { Pressed: true, Echo: false, Keycode: Key.F8 } key
            || key.CtrlPressed || key.AltPressed || key.ShiftPressed)
        {
            return;
        }
        GetViewport().SetInputAsHandled();
        var applied = Injector.Reload();
        ShowNotice($"번역을 다시 불러왔습니다 ({applied}개)");
    }

    private void ShowNotice(string text)
    {
        if (_notice == null)
        {
            return;
        }
        _notice.Text = text;
        _notice.Visible = true;
        GetTree().CreateTimer(2.5).Timeout += () => _notice.Visible = false;
    }
}
