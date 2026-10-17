using System.Text.RegularExpressions;
using Godot;
using MegaCrit.Sts2.Core.Localization;

namespace Sts2ModManagerBridge;

/// <summary>
/// In-game translation editor (F8 by default): search the translated keys,
/// fix one and see it right away, copy a grammar check prompt for an AI
/// chat, or report a mistranslation for the app.
/// <para>
/// It is its own embedded <see cref="Window"/> so typing goes to it and not
/// to the game's many <c>_Input</c> hotkeys: a focused sub-window receives
/// key events before the scene's input handlers run.
/// </para>
/// </summary>
public partial class EditPanel : Window
{
    private const int MaxListResults = 300;
    private const int MaxTreeResults = 5000;
    private static readonly Regex Markup = new(@"\[[^\]]*\]|\{[^}]*\}", RegexOptions.Compiled);

    private readonly List<CatalogEntry> _shown = new();
    private LineEdit _search = null!;
    private ItemList _results = null!;
    private Tree _tree = null!;
    private Button _viewToggle = null!;
    private bool _treeView;
    private Label _count = null!;
    private CheckBox _showKeys = null!;
    private Label _info = null!;
    private OptionButton _referenceLanguage = null!;
    private TextEdit _reference = null!;
    private TextEdit _value = null!;
    private LineEdit _note = null!;
    private Button _save = null!;
    private Button _revert = null!;
    private Button _report = null!;
    private Label _message = null!;
    private VBoxContainer _hotkeyArea = null!;
    private readonly Dictionary<HotkeyAction, Button> _hotkeyButtons = new();
    private CatalogEntry? _selected;

    /// <summary>The action waiting for its new key in the hotkey settings.</summary>
    private HotkeyAction? _capturing;

    /// <summary>
    /// The reference picked last ("" = the source text, else a language
    /// folder), kept while moving between entries.
    /// </summary>
    private string _preferredReference = "";

    public EditPanel()
    {
        UpdateTitle();
        Visible = false;
        // A separate OS window: in full screen a click on the game would put
        // the game in front and hide the panel. Always-on-top keeps it
        // visible while open (Godot ignores it on transient windows).
        Transient = false;
        AlwaysOnTop = true;
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
            PlaceholderText = "원문, 번역, 다른 언어, 키, 화면의 #번호로 검색 (색 태그와 {값}은 무시)",
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
            ClearButtonEnabled = true,
        };
        _search.TextChanged += _ => Refilter(keepSelection: false);
        searchRow.AddChild(_search);
        _count = new Label();
        searchRow.AddChild(_count);
        _viewToggle = new Button { Text = "트리 보기", TooltipText = "모드와 파일별 트리 / 전체 목록 전환" };
        _viewToggle.Pressed += () =>
        {
            _treeView = !_treeView;
            Refilter(keepSelection: true);
        };
        searchRow.AddChild(_viewToggle);
        _showKeys = new CheckBox { Text = "화면에 #번호 표시" };
        _showKeys.Toggled += pressed =>
        {
            Injector.ShowKeys = pressed;
            ReloadAndShow(pressed ? "화면의 번역 문장 앞에 (#번호)를 붙였습니다. #번호로 검색하세요" : "#번호 표시를 껐습니다");
        };
        searchRow.AddChild(_showKeys);
        var reload = new Button { Text = "다시 불러오기" };
        reload.Pressed += () => ReloadAndShow("공용 폴더에서 다시 불러왔습니다");
        searchRow.AddChild(reload);
        var hotkeys = new Button { Text = "단축키", TooltipText = "게임이나 다른 모드와 겹치는 단축키를 바꿉니다" };
        hotkeys.Pressed += () =>
        {
            _hotkeyArea.Visible = !_hotkeyArea.Visible;
            StopCapture();
        };
        searchRow.AddChild(hotkeys);
        layout.AddChild(searchRow);
        layout.AddChild(BuildHotkeyArea());

        var split = new HSplitContainer { SizeFlagsVertical = Control.SizeFlags.ExpandFill };
        layout.AddChild(split);

        var listArea = new VBoxContainer
        {
            CustomMinimumSize = new Vector2(380, 0),
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
        };
        split.AddChild(listArea);
        _results = new ItemList
        {
            SizeFlagsVertical = Control.SizeFlags.ExpandFill,
            TextOverrunBehavior = TextServer.OverrunBehavior.TrimEllipsis,
        };
        _results.ItemSelected += index => Select(_shown[(int)index]);
        listArea.AddChild(_results);
        _tree = new Tree
        {
            SizeFlagsVertical = Control.SizeFlags.ExpandFill,
            HideRoot = true,
            Visible = false,
        };
        _tree.ItemSelected += OnTreeSelected;
        listArea.AddChild(_tree);

        var detail = new VBoxContainer
        {
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
            CustomMinimumSize = new Vector2(420, 0),
        };
        detail.AddThemeConstantOverride("separation", 6);
        split.AddChild(detail);

        var infoRow = new HBoxContainer();
        _info = new Label
        {
            AutowrapMode = TextServer.AutowrapMode.WordSmart,
            SizeFlagsHorizontal = Control.SizeFlags.ExpandFill,
        };
        infoRow.AddChild(_info);
        var copyToolId = new Button { Text = "도구 id 복사", TooltipText = "번역 도구의 id 열 값 (모드를 열고 검색)" };
        copyToolId.Pressed += () =>
        {
            if (_selected is { Info.Id: null })
            {
                ShowMessage("도구 id가 아직 없습니다. 프로그램에서 이 모드를 한 번 더 적용해 주세요.");
                return;
            }
            Copy(_selected?.Info.Id, "도구 id");
        };
        infoRow.AddChild(copyToolId);
        var copyKey = new Button { Text = "키 복사" };
        copyKey.Pressed += () => Copy(_selected?.Key, "키");
        infoRow.AddChild(copyKey);
        detail.AddChild(infoRow);

        var referenceRow = new HBoxContainer();
        referenceRow.AddChild(new Label { Text = "참고" });
        _referenceLanguage = new OptionButton { SizeFlagsHorizontal = Control.SizeFlags.ExpandFill };
        _referenceLanguage.ItemSelected += index =>
        {
            _preferredReference = _referenceLanguage.GetItemMetadata((int)index).AsString();
            ShowReference();
        };
        referenceRow.AddChild(_referenceLanguage);
        var copyReference = new Button { Text = "복사" };
        copyReference.Pressed += () => Copy(_reference.Text, "참고 문장");
        referenceRow.AddChild(copyReference);
        var useReference = new Button { Text = "번역 칸에 넣기", TooltipText = "참고 문장으로 번역 칸을 채웁니다 (저장 전까지 적용되지 않음)" };
        useReference.Pressed += () =>
        {
            if (_selected != null)
            {
                _value.Text = _reference.Text;
            }
        };
        referenceRow.AddChild(useReference);
        detail.AddChild(referenceRow);
        _reference = new TextEdit
        {
            Editable = false,
            WrapMode = TextEdit.LineWrappingMode.Boundary,
            CustomMinimumSize = new Vector2(0, 110),
        };
        detail.AddChild(_reference);

        var valueRow = new HBoxContainer();
        valueRow.AddChild(new Label { Text = "번역", SizeFlagsHorizontal = Control.SizeFlags.ExpandFill });
        var copyPrompt = new Button
        {
            Text = "검수 프롬프트 복사",
            TooltipText = "원문과 번역 칸의 내용으로 문법·형식 검수 프롬프트를 만들어 복사합니다. AI 채팅에 붙여 넣으세요",
        };
        copyPrompt.Pressed += CopyCheckPrompt;
        valueRow.AddChild(copyPrompt);
        var copyValue = new Button { Text = "복사" };
        copyValue.Pressed += () => Copy(_value.Text, "번역");
        valueRow.AddChild(copyValue);
        detail.AddChild(valueRow);
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

        UpdateHotkeyTexts();
        Select(null);
    }

    private Control BuildHotkeyArea()
    {
        _hotkeyArea = new VBoxContainer { Visible = false };
        _hotkeyArea.AddChild(new Label
        {
            Text = "바꿀 단축키의 버튼을 누른 뒤 새 키 조합을 누르세요 (Esc 취소). 패널 열기와 #번호 표시는 게임 어디서나 동작합니다.",
            AutowrapMode = TextServer.AutowrapMode.WordSmart,
        });
        var grid = new GridContainer { Columns = 4 };
        grid.AddThemeConstantOverride("h_separation", 12);
        foreach (var action in Hotkeys.Actions)
        {
            grid.AddChild(new Label { Text = Hotkeys.Label(action) });
            var button = new Button { CustomMinimumSize = new Vector2(150, 0) };
            button.Pressed += () => StartCapture(action);
            grid.AddChild(button);
            _hotkeyButtons[action] = button;
        }
        _hotkeyArea.AddChild(grid);
        var reset = new Button { Text = "기본값으로", SizeFlagsHorizontal = Control.SizeFlags.ShrinkBegin };
        reset.Pressed += () =>
        {
            StopCapture();
            Hotkeys.ResetAll();
            UpdateHotkeyTexts();
            ShowMessage("단축키를 기본값으로 되돌렸습니다.");
        };
        _hotkeyArea.AddChild(reset);
        _hotkeyArea.AddChild(new HSeparator());
        return _hotkeyArea;
    }

    private void StartCapture(HotkeyAction action)
    {
        _capturing = action;
        UpdateHotkeyTexts();
        ShowMessage($"'{Hotkeys.Label(action)}'에 쓸 키 조합을 누르세요. Esc를 누르면 취소합니다.");
    }

    private void StopCapture()
    {
        if (_capturing == null)
        {
            return;
        }
        _capturing = null;
        UpdateHotkeyTexts();
    }

    /// <summary>Takes the key pressed while a hotkey button waits for one.</summary>
    private void Capture(HotkeyAction action, InputEventKey key)
    {
        if (Hotkeys.IsModifierOnly(key))
        {
            return;
        }
        StopCapture();
        if (key.Keycode == Key.Escape)
        {
            ShowMessage("단축키 변경을 취소했습니다.");
            return;
        }
        var combo = Hotkeys.Combo(key);
        var error = Hotkeys.Set(action, combo);
        UpdateHotkeyTexts();
        ShowMessage(error == null
            ? $"'{Hotkeys.Label(action)}' 단축키를 {Hotkeys.Format(combo)}(으)로 바꿨습니다."
            : $"{Hotkeys.Format(combo)}은(는) 쓸 수 없습니다. {error}");
    }

    private void UpdateHotkeyTexts()
    {
        foreach (var (action, button) in _hotkeyButtons)
        {
            button.Text = _capturing == action ? "키를 누르세요…" : Hotkeys.Text(action);
        }
        _showKeys.TooltipText = $"번역된 문장 앞에 (#번호)를 붙여 게임 화면에서 본 문장을 찾기 쉽게 합니다 ({Hotkeys.Text(HotkeyAction.ToggleNumbers)})";
        UpdateTitle();
    }

    private void UpdateTitle() =>
        Title = $"번역 수정 · {Hotkeys.Text(HotkeyAction.TogglePanel)}/Esc 닫기 · {Hotkeys.Text(HotkeyAction.Save)} 저장 · {Hotkeys.Text(HotkeyAction.CopyCheckPrompt)} 검수 프롬프트 복사";

    /// <summary>Opens the panel with fresh translations, or closes it.</summary>
    public void Toggle()
    {
        if (Visible)
        {
            Hide();
            return;
        }
        StopCapture();
        var screen = GetParent<Node>().GetViewport().GetVisibleRect().Size;
        var size = new Vector2I(
            Math.Min(1200, (int)(screen.X * 0.9f)),
            Math.Min(760, (int)(screen.Y * 0.85f)));
        // Not transient, so it centres on its own screen: put it on the game's.
        if (GetTree().Root is { } root && !IsEmbedded())
        {
            CurrentScreen = root.CurrentScreen;
        }
        PopupCentered(size);
        _showKeys.SetPressedNoSignal(Injector.ShowKeys);
        ReloadAndShow("");
        _search.GrabFocus();
    }

    private void OnWindowInput(InputEvent @event)
    {
        if (@event is not InputEventKey { Pressed: true, Echo: false } key)
        {
            return;
        }
        if (_capturing is { } action)
        {
            Capture(action, key);
            SetInputAsHandled();
            return;
        }
        if (key.Keycode == Key.Escape || Hotkeys.Matches(key, HotkeyAction.TogglePanel))
        {
            Hide();
        }
        else if (Hotkeys.Matches(key, HotkeyAction.Save))
        {
            Save();
        }
        else if (Hotkeys.Matches(key, HotkeyAction.CopyCheckPrompt))
        {
            CopyCheckPrompt();
        }
        else if (Hotkeys.Matches(key, HotkeyAction.ToggleNumbers))
        {
            // Goes through the check box's Toggled handler.
            _showKeys.ButtonPressed = !_showKeys.ButtonPressed;
        }
        else
        {
            return;
        }
        SetInputAsHandled();
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
        // "(#123)" copied from the screen searches for the number inside it.
        var query = _search.Text.Replace('(', ' ').Replace(')', ' ');
        var tokens = Normalize(query).Split(' ', StringSplitOptions.RemoveEmptyEntries);
        // Number hits first (exact before prefix), then plain text hits, each in catalog order.
        var matches = Injector.Catalog
            .Select(entry => (Entry: entry, Rank: tokens.Length == 0 ? 0 : tokens.Max(token => Rank(entry, token))))
            .Where(match => match.Rank < NoMatch)
            .OrderBy(match => match.Rank)
            .ThenBy(match => match.Entry.Number)
            .Select(match => match.Entry)
            .ToList();
        var limit = _treeView ? MaxTreeResults : MaxListResults;
        _shown.Clear();
        _shown.AddRange(matches.Take(limit));
        _count.Text = matches.Count > limit ? $"{limit}+ / {Injector.Catalog.Count}" : $"{matches.Count} / {Injector.Catalog.Count}";
        _viewToggle.Text = _treeView ? "목록 보기" : "트리 보기";
        _results.Visible = !_treeView;
        _tree.Visible = _treeView;

        var reselect = previous == null ? -1 : _shown.FindIndex(entry => SameKey(entry, previous));
        if (_treeView)
        {
            FillTree(expand: tokens.Length > 0, reselect);
        }
        else
        {
            FillList(reselect);
        }
        if (reselect >= 0)
        {
            Select(_shown[reselect]);
        }
        else if (!keepSelection || previous != null)
        {
            Select(null);
        }
    }

    private void FillList(int reselect)
    {
        _results.Clear();
        foreach (var entry in _shown)
        {
            var index = _results.AddItem($"{(entry.Edited ? "✎ " : "")}{entry.Tag}  {OneLine(entry.Current)}");
            _results.SetItemTooltip(index, $"{entry.ModId} · {entry.Table} · {entry.Key}");
        }
        if (reselect >= 0)
        {
            _results.Select(reselect);
            _results.EnsureCurrentIsVisible();
        }
    }

    /// <summary>mod → table file → entries, folded unless a search narrows it down.</summary>
    private void FillTree(bool expand, int reselect)
    {
        _tree.Clear();
        var root = _tree.CreateItem();
        var folders = new Dictionary<string, TreeItem>(StringComparer.Ordinal);
        for (var index = 0; index < _shown.Count; index++)
        {
            var entry = _shown[index];
            var modPath = entry.ModId;
            if (!folders.TryGetValue(modPath, out var mod))
            {
                mod = _tree.CreateItem(root);
                mod.SetText(0, entry.ModId);
                mod.SetSelectable(0, false);
                folders[modPath] = mod;
            }
            var tablePath = $"{entry.ModId}/{entry.Table}";
            if (!folders.TryGetValue(tablePath, out var table))
            {
                table = _tree.CreateItem(mod);
                table.SetText(0, entry.IsDll ? $"DLL {entry.Assembly}" : $"{entry.Table}.json");
                table.SetSelectable(0, false);
                table.Collapsed = !expand;
                folders[tablePath] = table;
            }
            var item = _tree.CreateItem(table);
            item.SetText(0, $"{(entry.Edited ? "✎ " : "")}{entry.Tag}  {OneLine(entry.Current)}");
            item.SetTooltipText(0, entry.Key);
            item.SetMetadata(0, index);
            if (index == reselect)
            {
                table.Collapsed = false;
                item.Select(0);
                _tree.ScrollToItem(item);
            }
        }
        foreach (var (path, folder) in folders)
        {
            if (!path.Contains('/'))
            {
                folder.SetText(0, $"{path} ({_shown.Count(entry => entry.ModId == path)})");
            }
            else
            {
                folder.SetText(0, $"{folder.GetText(0)} ({folder.GetChildCount()})");
            }
        }
    }

    private void OnTreeSelected()
    {
        if (_tree.GetSelected() is { } item && item.GetMetadata(0).VariantType == Variant.Type.Int)
        {
            Select(_shown[item.GetMetadata(0).AsInt32()]);
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
        _referenceLanguage.Clear();
        if (entry == null)
        {
            _info.Text = "목록에서 항목을 고르세요.";
            _reference.Text = "";
            _value.Text = "";
            return;
        }
        _info.Text = (entry.IsDll
                ? $"{entry.Tag} · {entry.ModId} · DLL {entry.Assembly}\n도구 id {entry.Info.Id ?? "-"} · 코드에 들어 있는 문자열"
                : $"{entry.Tag} · {entry.ModId} · {entry.Table}.json\n도구 id {entry.Info.Id ?? "-"} · 키 {entry.Key}")
            + (entry.Edited ? $"\n게임 안에서 고친 값입니다. 프로그램 값: {OneLine(entry.SheetValue)}" : "")
            + (entry.IsDll && entry.SheetValue.Length == 0 && !entry.Edited ? "\n아직 번역하지 않은 문자열입니다." : "");
        _referenceLanguage.AddItem(entry.Info.SourceLang is { } sourceLang ? $"원문 ({sourceLang})" : "원문");
        _referenceLanguage.SetItemMetadata(0, "");
        foreach (var language in entry.Info.Langs.Keys.OrderBy(language => language, StringComparer.Ordinal))
        {
            _referenceLanguage.AddItem(language);
            _referenceLanguage.SetItemMetadata(_referenceLanguage.ItemCount - 1, language);
        }
        var preferred = Enumerable.Range(0, _referenceLanguage.ItemCount)
            .FirstOrDefault(index => _referenceLanguage.GetItemMetadata(index).AsString() == _preferredReference);
        _referenceLanguage.Select(preferred);
        ShowReference();
        _value.Text = entry.Current;
    }

    private void ShowReference()
    {
        if (_selected is not { } entry)
        {
            return;
        }
        var language = _referenceLanguage.Selected >= 0 ? _referenceLanguage.GetItemMetadata(_referenceLanguage.Selected).AsString() : "";
        _reference.Text = language.Length == 0
            ? entry.Info.Source ?? "(원문 정보 없음 - 프로그램에서 한 번 더 적용하면 표시됩니다)"
            : entry.Info.Langs.TryGetValue(language, out var text) ? text : "";
    }

    private void Copy(string? text, string what)
    {
        if (string.IsNullOrEmpty(text))
        {
            return;
        }
        DisplayServer.ClipboardSet(text);
        ShowMessage($"{what}을(를) 복사했습니다.");
    }

    private void CopyCheckPrompt()
    {
        if (_selected is not { } entry)
        {
            ShowMessage("목록에서 항목을 먼저 고르세요.");
            return;
        }
        if (_value.Text.Trim().Length == 0)
        {
            ShowMessage("번역 칸이 비어 있습니다.");
            return;
        }
        var reference = _referenceLanguage.Selected >= 0 ? _referenceLanguage.GetItemMetadata(_referenceLanguage.Selected).AsString() : "";
        DisplayServer.ClipboardSet(CheckPrompt.Build(entry, _value.Text, Injector.CatalogLanguage, reference));
        ShowMessage("검수 프롬프트를 복사했습니다. AI 채팅에 붙여 넣으세요 (번역 칸의 저장 전 내용 기준).");
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
        if (entry.IsDll ? !SamePlaceholders(entry.Key, after, out var error) : !LocValidator.ValidateFormatString(after, out error))
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
        ReloadAndShow(entry.IsDll ? ApplyDll(entry, kind == EditLog.Edit ? after : entry.SheetValue) : "저장하고 적용했습니다");
    }

    /// <summary>Patches the methods using a DLL string again; text a screen already built stays until rebuilt.</summary>
    private static string ApplyDll(CatalogEntry entry, string value)
    {
        var patched = StringReplacer.ApplyLive(entry.Assembly, entry.Key, value);
        return patched < 0
            ? "저장했습니다. 게임을 다시 시작하면 반영됩니다"
            : "저장했습니다. 새로 만들어지는 화면부터 바뀝니다(이미 열린 화면은 다시 열어야 함)";
    }

    /// <summary>
    /// A DLL string may be a <c>string.Format</c> pattern: the translation must
    /// keep its <c>{0}</c>-style slots, or the mod throws when it formats it.
    /// </summary>
    private static bool SamePlaceholders(string original, string translation, out string? error)
    {
        static List<string> Slots(string text) => System.Text.RegularExpressions.Regex
            .Matches(text, @"\{\d+(?:[,:][^}]*)?\}")
            .Select(match => match.Value)
            .OrderBy(slot => slot, StringComparer.Ordinal)
            .ToList();
        var expected = Slots(original);
        var actual = Slots(translation);
        error = expected.SequenceEqual(actual)
            ? null
            : $"원문의 자리표시자 {string.Join(" ", expected)}을(를) 그대로 둬야 합니다";
        return error == null;
    }

    private void Revert()
    {
        if (_selected is not { Edited: true } entry || !Write(entry, EditLog.Revert, null, ""))
        {
            return;
        }
        ReloadAndShow(entry.IsDll ? ApplyDll(entry, entry.SheetValue) : "프로그램 값으로 되돌렸습니다");
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
            var file = entry.IsDll
                ? EditLog.DllPathFor(SharedPaths.Edits, entry.ModId, Injector.CatalogLanguage)
                : EditLog.PathFor(SharedPaths.Edits, entry.ModId, Injector.CatalogLanguage);
            var before = kind == EditLog.Report ? entry.Current : entry.SheetValue;
            EditLog.Append(file, new EditRecord(
                EditLog.NewId(DateTimeOffset.Now), kind, entry.Table, entry.Key, entry.Info.Source, before, after, note));
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

    private const int NoMatch = 3;

    /// <summary>
    /// How well one search word fits: 0 = the exact <c>#number</c>, 1 = a
    /// number starting with the typed digits, 2 = text match, <see cref="NoMatch"/>.
    /// <c>#154</c> only looks at numbers; bare digits also search the text.
    /// </summary>
    private static int Rank(CatalogEntry entry, string token)
    {
        var numberOnly = token.Length > 1 && token[0] == '#';
        var digits = numberOnly ? token[1..] : token;
        if (digits.Length > 0 && digits.All(char.IsAsciiDigit))
        {
            var number = entry.Number.ToString();
            if (number == digits)
            {
                return 0;
            }
            if (number.StartsWith(digits, StringComparison.Ordinal))
            {
                return 1;
            }
            if (numberOnly)
            {
                return NoMatch;
            }
        }
        return Haystack(entry).Contains(token, StringComparison.Ordinal) ? 2 : NoMatch;
    }

    private static string Haystack(CatalogEntry entry) =>
        Normalize($"{entry.Info.Id} {entry.Key} {entry.Info.Source} {string.Join(' ', entry.Info.Langs.Values)} {entry.Current} {entry.SheetValue}");

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
