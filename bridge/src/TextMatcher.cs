namespace Sts2ModManagerBridge;

/// <summary>
/// Finds which of many short texts occur in a longer one, indexed by their
/// first two characters so a label text costs one lookup per position.
/// Latin texts only match whole words (<c>Hit</c> is not in <c>Hits</c>).
/// Single characters (<c>。</c>) have their own index.
/// </summary>
public sealed class TextMatcher
{
    private readonly Dictionary<int, Candidate[]> _byStart;
    private readonly Dictionary<char, Candidate[]> _single;

    public TextMatcher(IEnumerable<Candidate> candidates)
    {
        var all = candidates.ToList();
        _byStart = all
            .Where(candidate => candidate.Text.Length >= 2)
            .GroupBy(candidate => StartKey(candidate.Text, 0))
            .ToDictionary(group => group.Key, group => group.ToArray());
        _single = all
            .Where(candidate => candidate.Text.Length == 1)
            .GroupBy(candidate => candidate.Text[0])
            .ToDictionary(group => group.Key, group => group.ToArray());
    }

    public bool IsEmpty => _byStart.Count == 0 && _single.Count == 0;

    public IEnumerable<Candidate> Find(string text)
    {
        if (_single.Count > 0)
        {
            foreach (var character in text.Distinct())
            {
                if (_single.TryGetValue(character, out var singles))
                {
                    foreach (var candidate in singles)
                    {
                        yield return candidate;
                    }
                }
            }
        }
        for (var index = 0; index + 1 < text.Length; index++)
        {
            if (!_byStart.TryGetValue(StartKey(text, index), out var candidates))
            {
                continue;
            }
            foreach (var candidate in candidates)
            {
                var end = index + candidate.Text.Length;
                if (end <= text.Length
                    && string.CompareOrdinal(text, index, candidate.Text, 0, candidate.Text.Length) == 0
                    && (!candidate.WholeWord || (IsBoundary(text, index - 1) && IsBoundary(text, end))))
                {
                    yield return candidate;
                }
            }
        }
    }

    /// <summary>Text without BBCode tags, so <c>[purple]</c> is not a word.</summary>
    public static string WithoutMarkup(string text) =>
        text.Contains('[') ? System.Text.RegularExpressions.Regex.Replace(text, @"\[[^\]\n]*\]", "") : text;

    private static int StartKey(string text, int index) => (text[index] << 16) | text[index + 1];

    private static bool IsBoundary(string text, int index) =>
        index < 0 || index >= text.Length || !char.IsLetterOrDigit(text[index]);

    /// <summary>One text to look for and the literal of the DLL it stands for.</summary>
    public sealed record Candidate(string Text, string Owner, string Literal, bool WholeWord)
    {
        private static readonly string[] CodeMarkers = { "(?:", "\\d", "\\s", "\\w", "[^", "(\\", ")+", ")*", ".*", "+?", "*?" };

        /// <summary>A candidate for <paramref name="text"/>, or null when it is too short or not words.</summary>
        public static Candidate? For(string text, string owner, string literal)
        {
            var trimmed = WithoutMarkup(text).Trim();
            // A lone CJK character or mark (`。`, `攻`); Latin ones are code.
            if (trimmed.Length == 1)
            {
                return trimmed[0] >= '\u3000' ? new Candidate(trimmed, owner, literal, false) : null;
            }
            if (trimmed.Length < 2 || !trimmed.Any(char.IsLetter) || CodeMarkers.Any(trimmed.Contains))
            {
                return null;
            }
            var latin = trimmed.All(character => character < 0x250);
            if (latin
                && !trimmed.Contains(' ')
                && (trimmed.Count(char.IsLetter) < 6 || !char.IsUpper(trimmed[0]) || LooksLikeName(trimmed)))
            {
                // A short Latin word alone turns up everywhere, a lowercase
                // one (`description`) is a key, and names (`DrawInternal`,
                // `cards.json`, `res://…`) are not text.
                return null;
            }
            return new Candidate(trimmed, owner, literal, latin);
        }

        private static bool LooksLikeName(string word)
        {
            if (word.IndexOfAny(new[] { '.', '/', '\\', '_', ':', '<', '>', '#', '=' }) >= 0)
            {
                return true;
            }
            // A case change inside the word: `AcceleratedMotion`, `cardPool`.
            for (var index = 1; index < word.Length; index++)
            {
                if (char.IsUpper(word[index]) && char.IsLower(word[index - 1]))
                {
                    return true;
                }
            }
            return false;
        }
    }
}
