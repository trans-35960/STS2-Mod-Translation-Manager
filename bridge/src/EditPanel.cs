using System.Text.RegularExpressions;
using Godot;
using MegaCrit.Sts2.Core.Localization;

namespace Sts2ModManagerBridge;

/// <summary>
/// In-game translation editor (F8): search the translated keys, fix one and
/// see it right away, or report a mistranslation for the app.
/// <para>
/// It is its own embedded <see cref="Window"/> so typing goes to it and not
/// to the game's many <c>_Input</c> hotkeys: a focused sub-window receives
/// key events before the scene's input handlers run.
/// </para>
/// </summary>
public partial class EditPanel : Window
{
    private const int MaxResults = 300;
    private static readonly Regex Markup = new(@"\[[^\]]*\]|\{[^}]*\}", RegexOptions.Compiled);

    private readonly List<CatalogEntry> _shown = new();
    private LineEdit _search = null!;
    private ItemList _results = null!;
    private Label _count = null!;
    private Label _info = null!;
    private TextEdit _source = null!;
    private TextEdit _value = null!;
    private LineEdit _note = null!;
    private Button _save = null!;
    private Button _revert = null!;
    private Button _report = null!;
    private Label _message = null!;
    private CatalogEntry? _selected;

    public EditPanel()
    {
        Title = "번역 수정 · F8/Esc 닫기 · Ctrl+Enter 저장";
        Visible = false;
        Transient = true;
        WrapControls = false;
        CloseRequested += Hide;
        WindowInput += OnWindowInput;
    }

    public override void _Ready()
    {
        var background = new PanelContainer();
        background.SetAnchorsPreset(Control.LayoutPreset.FullRect);
        AddChild(background);

        var margin = new MarginContainer();
        foreach (var side in new[] { "left", "right", "top", "bottom" })
        {
            margin.AddThemeConstantOverride("margin_" + side, 12);
        }
        background.AddChild(margin);

        var layout = new VBoxContainer();
        layout.AddThemeConstantOverride("separation", 8);
        margin.AddChild(layout);

        var searchRow = new HBoxContainer();
        _search = new LineEdit
        {
            PlaceholderText = "원문, 번역, 키로 검색 (색 태그와 {값}은 무시)",
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
            ClearButtonEnabled = true,
        };
        _search.TextChanged += _ => Refilter(keepSelection: false);
        searchRow.AddChild(_search);
        _count = new Label();
        searchRow.AddChild(_count);
        var reload = new Button { Text = "다시 불러오기" };
        reload.Pressed += () => ReloadAndShow($"공용 폴더에서 다시 불러왔습니다");
        searchRow.AddChild(reload);
        layout.AddChild(searchRow);

        var split = new HSplitContainer { SizeFlagsVertical = Control.SizeFlags.ExpandFill };
        layout.AddChild(split);

        _results = new ItemList
        {
            CustomMinimumSize = new Vector2(380, 0),
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
            TextOverrunBehavior = TextServer.OverrunBehavior.TrimEllipsis,
        };
        _results.ItemSelected += index => Select(_shown[(int)index]);
        split.AddChild(_results);

        var detail = new VBoxContainer
        {
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
            CustomMinimumSize = new Vector2(420, 0),
        };
        detail.AddThemeConstantOverride("separation", 6);
        split.AddChild(detail);

        _info = new Label { AutowrapMode = TextServer.AutowrapMode.WordSmart };
        detail.AddChild(_info);
        detail.AddChild(new Label { Text = "원문" });
        _source = new TextEdit
        {
            Editable = false,
            WrapMode = TextEdit.LineWrappingMode.Boundary,
            CustomMinimumSize = new Vector2(0, 110),
        };
        detail.AddChild(_source);
        detail.AddChild(new Label { Text = "번역" });
        _value = new TextEdit
        {
            WrapMode = TextEdit.LineWrappingMode.Boundary,
            SizeFlagsVertical = Control.SizeFlags.ExpandFill,
            CustomMinimumSize = new Vector2(0, 140),
        };
        detail.AddChild(_value);

        var editRow = new HBoxContainer();
        _save = new Button { Text = "저장하고 적용" };
        _save.Pressed += Save;
        editRow.AddChild(_save);
        _revert = new Button { Text = "게임 안 수정 취소" };
        _revert.Pressed += Revert;
        editRow.AddChild(_revert);
        detail.AddChild(editRow);

        var reportRow = new HBoxContainer();
        _note = new LineEdit
        {
            PlaceholderText = "오역 메모 (예: 문맥상 '시련'이 맞음)",
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
        };
        reportRow.AddChild(_note);
        _report = new Button { Text = "오역 신고" };
        _report.Pressed += Report;
        reportRow.AddChild(_report);
        detail.AddChild(reportRow);

        _message = new Label { AutowrapMode = TextServer.AutowrapMode.WordSmart };
        layout.AddChild(_message);

        Select(null);
    }

    /// <summary>Opens the panel with fresh translations, or closes it.</summary>
    public void Toggle()
    {
        if (Visible)
        {
            Hide();
            return;
        }
        var screen = GetParent<Node>().GetViewport().GetVisibleRect().Size;
        var size = new Vector2I(
            Math.Min(1100, (int)(screen.X * 0.9f)),
            Math.Min(700, (int)(screen.Y * 0.85f)));
        PopupCentered(size);
        ReloadAndShow("");
        _search.GrabFocus();
    }

    private void OnWindowInput(InputEvent @event)
    {
        if (@event is not InputEventKey { Pressed: true, Echo: false } key)
        {
            return;
        }
        if (key.Keycode == Key.Escape || (key.Keycode == Key.F8 && !key.CtrlPressed && !key.AltPressed && !key.ShiftPressed))
        {
            Hide();
            SetInputAsHandled();
        }
        else if (key.Keycode is Key.Enter or Key.KpEnter && key.CtrlPressed)
        {
            Save();
            SetInputAsHandled();
        }
    }

    private void ReloadAndShow(string message)
    {
        var applied = Injector.Reload();
        Refilter(keepSelection: true);
        ShowMessage(Injector.Catalog.Count == 0
            ? "공용 폴더에 이 언어의 번역이 없습니다. 프로그램의 번역 도구에서 먼저 적용해 주세요."
            : message.Length > 0 ? $"{message} ({applied}개 적용)" : "");
    }

    private void Refilter(bool keepSelection)
    {
        var previous = keepSelection ? _selected : null;
        var tokens = Normalize(_search.Text).Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var matches = Injector.Catalog
            .Where(entry => tokens.All(token => Haystack(entry).Contains(token, StringComparison.Ordinal)))
            .ToList();
        _shown.Clear();
        _shown.AddRange(matches.Take(MaxResults));
        _results.Clear();
        var reselect = -1;
        for (var index = 0; index < _shown.Count; index++)
        {
            var entry = _shown[index];
            _results.AddItem((entry.Edited ? "✎ " : "") + OneLine(entry.Current));
            _results.SetItemTooltip(index, $"{entry.ModId} · {entry.Table} · {entry.Key}");
            if (previous != null && SameKey(entry, previous))
            {
                reselect = index;
            }
        }
        _count.Text = matches.Count > MaxResults ? $"{MaxResults}+ / {Injector.Catalog.Count}" : $"{matches.Count} / {Injector.Catalog.Count}";
        if (reselect >= 0)
        {
            _results.Select(reselect);
            Select(_shown[reselect]);
        }
        else if (!keepSelection || previous != null)
        {
            Select(null);
        }
    }

    private void Select(CatalogEntry? entry)
    {
        _selected = entry;
        var enabled = entry != null;
        _save.Disabled = !enabled;
        _report.Disabled = !enabled;
        _revert.Disabled = entry is not { Edited: true };
        _value.Editable = enabled;
        if (entry == null)
        {
            _info.Text = "목록에서 항목을 고르세요.";
            _source.Text = "";
            _value.Text = "";
            return;
        }
        _info.Text = $"{entry.ModId} · {entry.Table} · {entry.Key}"
            + (entry.Edited ? $"\n게임 안에서 고친 값입니다. 프로그램 값: {OneLine(entry.SheetValue)}" : "");
        _source.Text = entry.Source ?? "(원문 정보 없음 - 프로그램에서 한 번 더 적용하면 표시됩니다)";
        _value.Text = entry.Current;
    }

    private void Save()
    {
        if (_selected is not { } entry)
        {
            return;
        }
        var after = _value.Text;
        if (after == entry.Current)
        {
            ShowMessage("바뀐 내용이 없습니다.");
            return;
        }
        if (after.Trim().Length == 0)
        {
            ShowMessage("번역을 비워 둘 수는 없습니다. 원래 값으로 돌리려면 '게임 안 수정 취소'를 누르세요.");
            return;
        }
        if (!LocValidator.ValidateFormatString(after, out var error))
        {
            ShowMessage($"저장하지 않았습니다. 형식 오류: {error}");
            return;
        }
        // Typing the app's value back is the same as undoing the edit.
        var kind = after == entry.SheetValue ? EditLog.Revert : EditLog.Edit;
        if (!Write(entry, kind, kind == EditLog.Edit ? after : null, ""))
        {
            return;
        }
        ReloadAndShow("저장하고 적용했습니다");
    }

    private void Revert()
    {
        if (_selected is not { Edited: true } entry || !Write(entry, EditLog.Revert, null, ""))
        {
            return;
        }
        ReloadAndShow("프로그램 값으로 되돌렸습니다");
    }

    private void Report()
    {
        if (_selected is not { } entry)
        {
            return;
        }
        if (!Write(entry, EditLog.Report, null, _note.Text.Trim()))
        {
            return;
        }
        _note.Text = "";
        ShowMessage("오역 신고를 남겼습니다. 프로그램의 번역 도구에서 확인할 수 있습니다.");
    }

    /// <summary>
    /// Appends to the edit log. <c>before</c> is always the app's value so an
    /// edit stops applying once the app changes that key.
    /// </summary>
    private bool Write(CatalogEntry entry, string kind, string? after, string note)
    {
        try
        {
            var file = EditLog.PathFor(SharedPaths.Edits, entry.ModId, Injector.CatalogLanguage);
            var before = kind == EditLog.Report ? entry.Current : entry.SheetValue;
            EditLog.Append(file, new EditRecord(
                EditLog.NewId(DateTimeOffset.Now), kind, entry.Table, entry.Key, entry.Source, before, after, note));
            return true;
        }
        catch (Exception exception)
        {
            BridgeMod.Log.Warn($"[{BridgeMod.ModId}] edit write failed: {exception.Message}");
            ShowMessage($"기록하지 못했습니다: {exception.Message}");
            return false;
        }
    }

    private void ShowMessage(string text) => _message.Text = text;

    private static string Haystack(CatalogEntry entry) =>
        Normalize($"{entry.Key} {entry.Source} {entry.Current} {entry.SheetValue}");

    private static string Normalize(string text) =>
        Markup.Replace(text, " ").ToLowerInvariant();

    private static string OneLine(string text)
    {
        var line = Markup.Replace(text, "").Replace('\n', ' ').Trim();
        return line.Length > 80 ? line[..80] + "…" : line;
    }

    private static bool SameKey(CatalogEntry left, CatalogEntry right) =>
        left.ModId == right.ModId && left.Table == right.Table && left.Key == right.Key;
}
