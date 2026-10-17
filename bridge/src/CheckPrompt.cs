using System.Text;
using System.Text.RegularExpressions;

namespace Sts2ModManagerBridge;

/// <summary>
/// Builds a prompt asking an AI chat to check one translation against its
/// source for broken markup and grammar. The panel copies it to the
/// clipboard; nothing is sent anywhere from the game.
/// </summary>
public static class CheckPrompt
{
    private static readonly Regex Placeholder = new(@"\{[^{}]*\}", RegexOptions.Compiled);
    private static readonly Regex Tag = new(@"\[/?[A-Za-z_][^\[\]]*\]", RegexOptions.Compiled);

    /// <param name="entry">The entry being edited.</param>
    /// <param name="translation">The translation box as typed, saved or not.</param>
    /// <param name="language">The game language code of the translation (e.g. <c>kor</c>).</param>
    /// <param name="reference">The reference language picked in the panel ("" = source).</param>
    public static string Build(CatalogEntry entry, string translation, string language, string reference)
    {
        var source = entry.Info.Source ?? (entry.IsDll ? entry.Key : null);
        var sourceLang = entry.Info.SourceLang ?? "원문";
        var targetName = LanguageName(language);
        var text = new StringBuilder();

        text.AppendLine($"다음은 게임 Slay the Spire 2의 모드 \"{entry.ModId}\"에 들어가는 문자열의 {targetName} 번역입니다.");
        text.AppendLine("번역문이 원문과 비교해 형식이나 문법이 깨지지 않았는지 검수해 주세요.");
        text.AppendLine();
        text.AppendLine("[정보]");
        text.AppendLine(entry.IsDll
            ? $"- 위치: DLL {entry.Assembly} (C# 코드 안 문자열)"
            : $"- 위치: {entry.Table}.json / 키 {entry.Key}");
        text.AppendLine($"- 번역 언어: {targetName} ({language})");
        text.AppendLine();

        if (source != null)
        {
            AppendBlock(text, $"원문 ({sourceLang})", source);
        }
        else
        {
            text.AppendLine("[원문]");
            text.AppendLine("(원문 정보 없음)");
            text.AppendLine();
        }
        if (reference.Length > 0 && reference != language
            && entry.Info.Langs.TryGetValue(reference, out var referenceText) && referenceText.Trim().Length > 0)
        {
            AppendBlock(text, $"참고 번역 ({reference})", referenceText);
        }
        AppendBlock(text, $"검수할 번역 ({language})", translation);

        if (source != null)
        {
            var notes = Differences(source, translation);
            if (notes.Count > 0)
            {
                text.AppendLine("[자동 비교에서 눈에 띈 차이 (참고용)]");
                foreach (var note in notes)
                {
                    text.AppendLine($"- {note}");
                }
                text.AppendLine();
            }
        }

        text.AppendLine("[확인할 것]");
        text.AppendLine("1. 자리표시자: `{Damage}`, `{Neko:diff()}`, `{0}`처럼 중괄호로 감싼 부분은 게임이 값을 넣는 코드입니다. 원문과 글자 하나까지 같아야 합니다(대소문자, 콜론 뒤 수식 포함). 빠지거나, 더해지거나, 바뀐 것이 있는지 봐 주세요. 순서는 문법상 바뀌어도 됩니다.");
        text.AppendLine("2. 태그: `[gold]…[/gold]`처럼 대괄호로 감싼 서식 태그가 원문과 같은 종류인지, 여는 태그와 닫는 태그의 짝과 이름이 맞는지 봐 주세요. 태그가 감싸는 범위가 원문의 의미와 맞는지도 봐 주세요.");
        text.AppendLine("3. 줄바꿈과 문장 구성이 원문과 크게 어긋나지 않는지 봐 주세요.");
        text.AppendLine("4. 의미: 원문에 있는 숫자, 대상, 조건, 효과가 빠지거나 덧붙거나 잘못 옮겨진 곳이 있는지 봐 주세요.");
        text.AppendLine(language == "kor"
            ? "5. 한국어 문법: 조사(을/를, 이/가, 은/는, (으)로 — 자리표시자나 태그 바로 뒤 포함), 띄어쓰기, 맞춤법, 어색한 어순, 카드 게임 용어의 일관성을 봐 주세요. 자리표시자 뒤 조사는 들어갈 값을 생각해 자연스럽게 골라 주세요."
            : $"5. {targetName} 문법: 맞춤법, 문법, 어색한 표현, 카드 게임 용어의 일관성을 봐 주세요.");
        text.AppendLine();
        text.AppendLine("[답변 형식]");
        text.AppendLine("- 문제가 없으면 \"문제 없음\"이라고만 답해 주세요.");
        text.AppendLine("- 문제가 있으면 문제마다 무엇이 왜 문제인지 한 줄로 적고, 마지막에 고친 번역 전체를 코드 블록 하나에 담아 주세요.");
        text.AppendLine("- 고친 번역에서도 자리표시자와 태그는 원문 그대로 두세요. 게임에 바로 붙여 넣을 수 있도록 설명이나 따옴표를 코드 블록 안에 넣지 마세요.");
        return text.ToString();
    }

    private static void AppendBlock(StringBuilder text, string title, string body)
    {
        // A longer fence than any backtick run inside keeps the block intact.
        var fence = new string('`', Math.Max(3, LongestBacktickRun(body) + 1));
        text.AppendLine($"[{title}]");
        text.AppendLine(fence);
        text.AppendLine(body.TrimEnd('\r', '\n'));
        text.AppendLine(fence);
        text.AppendLine();
    }

    /// <summary>Mechanical placeholder/tag/line differences, as hints for the reviewer.</summary>
    private static List<string> Differences(string source, string translation)
    {
        var notes = new List<string>();
        Compare(Placeholder, "자리표시자", source, translation, notes);
        Compare(Tag, "태그", source, translation, notes);
        var sourceLines = source.Split('\n').Length;
        var translationLines = translation.Split('\n').Length;
        if (sourceLines != translationLines)
        {
            notes.Add($"줄 수가 다릅니다 (원문 {sourceLines}줄, 번역 {translationLines}줄).");
        }
        return notes;
    }

    private static void Compare(Regex pattern, string what, string source, string translation, List<string> notes)
    {
        var expected = Counts(pattern, source);
        var actual = Counts(pattern, translation);
        var missing = expected.Where(pair => actual.GetValueOrDefault(pair.Key) < pair.Value).Select(pair => pair.Key).ToList();
        var extra = actual.Where(pair => expected.GetValueOrDefault(pair.Key) < pair.Value).Select(pair => pair.Key).ToList();
        if (missing.Count > 0)
        {
            notes.Add($"원문에 있는 {what}가 번역에서 빠졌거나 적습니다: {string.Join(" ", missing)}");
        }
        if (extra.Count > 0)
        {
            notes.Add($"원문에 없는 {what}가 번역에 있거나 더 많습니다: {string.Join(" ", extra)}");
        }
    }

    private static Dictionary<string, int> Counts(Regex pattern, string text) => pattern.Matches(text)
        .GroupBy(match => match.Value, StringComparer.Ordinal)
        .ToDictionary(group => group.Key, group => group.Count(), StringComparer.Ordinal);

    private static int LongestBacktickRun(string text)
    {
        int longest = 0, run = 0;
        foreach (var character in text)
        {
            run = character == '`' ? run + 1 : 0;
            longest = Math.Max(longest, run);
        }
        return longest;
    }

    private static string LanguageName(string language) => language switch
    {
        "kor" => "한국어",
        "eng" => "영어",
        "jpn" => "일본어",
        "zhs" => "중국어(간체)",
        "zht" => "중국어(번체)",
        "deu" => "독일어",
        "fra" => "프랑스어",
        "spa" or "esp" => "스페인어",
        "ita" => "이탈리아어",
        "pol" => "폴란드어",
        "ptb" => "포르투갈어(브라질)",
        "rus" => "러시아어",
        "tha" => "태국어",
        "tur" => "튀르키예어",
        _ => language.Length > 0 ? language : "대상 언어",
    };
}
