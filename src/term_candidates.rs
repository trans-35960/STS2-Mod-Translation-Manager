//! Mechanical glossary candidates for a mod, without AI.
//!
//! Terms come from three signals: words the mod highlights (`[gold]Ammo[/gold]`),
//! name entries (`*.title` / `*.name`), and, for English sources, capitalized
//! words in mid-sentence. Where the sheet already has translations, the
//! highlighted words are paired with their translations by tag order to
//! suggest a translation and to expose inconsistent or colliding ones.

use crate::glossary::{
    GameGlossary, GlossaryTerm, KOREAN_JOSA, has_markup, highlight_spans, is_korean_language,
};
use crate::json_translation::{JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus};
use std::collections::{BTreeMap, HashMap, HashSet};

const MAX_TERM_CHARS: usize = 40;
const MAX_TERM_WORDS: usize = 5;
const MAX_CANDIDATES: usize = 300;
const MAX_EXAMPLE_CHARS: usize = 120;
const MIN_CAPITALIZED_COUNT: usize = 3;
const MIN_HIGHLIGHT_COUNT: usize = 2;

/// Sentence-initial or rules words that are capitalized without being names.
const CAPITALIZED_STOPWORDS: &[&str] = &[
    "a", "add", "all", "an", "and", "apply", "at", "choose", "deal", "draw", "each", "for", "gain",
    "i", "if", "in", "is", "it", "lose", "of", "on", "or", "the", "then", "this", "to", "upon",
    "when", "whenever", "with", "you", "your", "hp",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermCandidate {
    pub source: String,
    /// Entries whose source mentions the term.
    pub count: usize,
    /// Which signals found it: `highlight`, `title`, `capitalized`.
    pub kinds: Vec<&'static str>,
    /// Translations seen in the sheet, most used first.
    pub suggestions: Vec<(String, usize)>,
    /// Other candidates whose main translation is the same word.
    pub shares_translation_with: Vec<String>,
    /// A source line using the term, for context.
    pub example: String,
}

#[derive(Default)]
struct Builder {
    kinds: Vec<&'static str>,
    suggestions: HashMap<String, usize>,
    display: String,
}

pub fn extract_term_candidates(
    sheet: &JsonTranslationSheet,
    source_language: &str,
    game: Option<&GameGlossary>,
    known_terms: &[GlossaryTerm],
) -> Vec<TermCandidate> {
    let entries = sheet
        .entries
        .iter()
        .filter(|entry| entry.status != JsonTranslationStatus::Removed)
        .collect::<Vec<_>>();
    let korean = is_korean_language(&sheet.target_language);
    let known = known_terms
        .iter()
        .map(|term| term.source.to_lowercase())
        .collect::<HashSet<_>>();
    let mut builders: BTreeMap<String, Builder> = BTreeMap::new();
    let note = |builders: &mut BTreeMap<String, Builder>, term: &str, kind: &'static str| {
        let builder = builders.entry(term.to_lowercase()).or_default();
        if builder.display.is_empty() {
            builder.display = term.to_string();
        }
        if !builder.kinds.contains(&kind) {
            builder.kinds.push(kind);
        }
    };

    // Highlighted words, paired with the translation's highlights when the
    // tag sequence matches.
    let mut highlight_counts: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        let source_spans = highlight_spans(&entry.source_value);
        let mut seen = HashSet::new();
        for span in &source_spans {
            if let Some(term) = clean_term(span.content)
                && seen.insert(term.to_lowercase())
            {
                *highlight_counts.entry(term.to_lowercase()).or_default() += 1;
                note(&mut builders, &term, "highlight");
            }
        }
        if entry.translated_value.trim().is_empty() || entry.translated_value == entry.source_value
        {
            continue;
        }
        let target_spans = highlight_spans(&entry.translated_value);
        let same_shape = source_spans.len() == target_spans.len()
            && source_spans
                .iter()
                .zip(&target_spans)
                .all(|(left, right)| left.tag == right.tag);
        if !same_shape {
            continue;
        }
        for (source, target) in source_spans.iter().zip(&target_spans) {
            let (Some(term), target) = (clean_term(source.content), target.content.trim()) else {
                continue;
            };
            if target.is_empty() || has_markup(target) {
                continue;
            }
            let target = if korean {
                strip_trailing_josa(target)
            } else {
                target
            };
            *builders
                .entry(term.to_lowercase())
                .or_default()
                .suggestions
                .entry(target.to_string())
                .or_default() += 1;
        }
    }

    // Name entries are already source → translation pairs.
    for entry in &entries {
        if !is_name_entry(entry) {
            continue;
        }
        let Some(term) = clean_term(&entry.source_value) else {
            continue;
        };
        note(&mut builders, &term, "title");
        let target = entry.translated_value.trim();
        if !target.is_empty() && target != entry.source_value.trim() && !has_markup(target) {
            *builders
                .entry(term.to_lowercase())
                .or_default()
                .suggestions
                .entry(target.to_string())
                .or_default() += 1;
        }
    }

    if source_language.eq_ignore_ascii_case("eng") {
        let mut capitalized: HashMap<String, (String, usize)> = HashMap::new();
        for entry in &entries {
            let mut seen = HashSet::new();
            for term in capitalized_phrases(&entry.source_value) {
                if seen.insert(term.to_lowercase()) {
                    capitalized
                        .entry(term.to_lowercase())
                        .or_insert((term, 0))
                        .1 += 1;
                }
            }
        }
        for (_, (term, count)) in capitalized {
            if count >= MIN_CAPITALIZED_COUNT {
                note(&mut builders, &term, "capitalized");
            }
        }
    }

    // Keep terms that are not official game names or already registered,
    // and that are worth fixing (repeated highlights, names, frequent caps).
    let lowered_sources = entries
        .iter()
        .map(|entry| entry.source_value.to_lowercase())
        .collect::<Vec<_>>();
    let mut candidates = builders
        .into_iter()
        .filter(|(lower, builder)| {
            !known.contains(lower)
                && !game.is_some_and(|game| game.lookup(&builder.display).is_some())
                && (builder.kinds.contains(&"title")
                    || builder.kinds.contains(&"capitalized")
                    || highlight_counts.get(lower).copied().unwrap_or(0) >= MIN_HIGHLIGHT_COUNT)
        })
        .map(|(lower, builder)| {
            let mut suggestions = builder.suggestions.into_iter().collect::<Vec<_>>();
            suggestions
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            let mention_count = lowered_sources
                .iter()
                .filter(|source| mentions(source, &lower))
                .count();
            let example = entries
                .iter()
                .find(|entry| {
                    !is_name_entry(entry) && mentions(&entry.source_value.to_lowercase(), &lower)
                })
                .map(|entry| example_line(&entry.source_value))
                .unwrap_or_default();
            TermCandidate {
                source: builder.display,
                count: mention_count.max(1),
                kinds: builder.kinds,
                suggestions,
                shares_translation_with: Vec::new(),
                example,
            }
        })
        .collect::<Vec<_>>();

    let mut by_translation: HashMap<String, Vec<String>> = HashMap::new();
    for candidate in &candidates {
        if let Some((top, _)) = candidate.suggestions.first() {
            by_translation
                .entry(top.clone())
                .or_default()
                .push(candidate.source.clone());
        }
    }
    for candidate in &mut candidates {
        if let Some((top, _)) = candidate.suggestions.first() {
            candidate.shares_translation_with = by_translation[top]
                .iter()
                .filter(|other| **other != candidate.source)
                .cloned()
                .collect();
        }
    }

    candidates.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.source.to_lowercase().cmp(&right.source.to_lowercase()))
    });
    candidates.truncate(MAX_CANDIDATES);
    candidates
}

/// Whole-word match for spaced scripts; plain substring for CJK text,
/// which has no spaces between words.
fn mentions(source_lower: &str, term_lower: &str) -> bool {
    if term_lower.chars().any(|ch| !ch.is_ascii()) {
        source_lower.contains(term_lower)
    } else {
        crate::glossary::contains_word(source_lower, term_lower)
    }
}

/// A short phrase worth treating as a term, or `None` for sentences,
/// numbers and markup.
fn clean_term(value: &str) -> Option<String> {
    let term = value
        .trim()
        .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '“' | '”'));
    let words = term.split_whitespace().count();
    let has_letter = term.chars().any(char::is_alphabetic);
    let sentence = term.ends_with(['.', '?', '!', '。', '？', '！']);
    let token = term.starts_with('!') && term.ends_with('!');
    (has_letter
        && !sentence
        && !token
        && !has_markup(term)
        && term.chars().count() <= MAX_TERM_CHARS
        && (1..=MAX_TERM_WORDS).contains(&words))
    .then(|| term.to_string())
}

fn is_name_entry(entry: &JsonTranslationEntry) -> bool {
    let pointer = entry.key.rsplit(['#', '/']).next().unwrap_or_default();
    let last = pointer
        .rsplit('.')
        .next()
        .unwrap_or(pointer)
        .to_ascii_lowercase();
    matches!(last.as_str(), "title" | "name")
}

fn strip_trailing_josa(value: &str) -> &str {
    KOREAN_JOSA
        .iter()
        .find(|josa| value.len() > josa.len() + 3 && value.ends_with(*josa))
        .map(|josa| value[..value.len() - josa.len()].trim_end())
        .unwrap_or(value)
}

/// Runs of capitalized words that do not start a sentence ("Koyuki",
/// "Mod Bosses"), ignoring markup and rules words.
fn capitalized_phrases(value: &str) -> Vec<String> {
    let plain = strip_for_words(value);
    let mut phrases = Vec::new();
    for sentence in plain.split(['.', '!', '?', ':', '\n', ';']) {
        let words = sentence
            .split(|ch: char| !(ch.is_ascii_alphabetic() || ch == '\'' || ch == '-'))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let mut index = 1;
        while index < words.len() {
            let is_name = |word: &str| {
                word.chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
                    && !CAPITALIZED_STOPWORDS.contains(&word.to_ascii_lowercase().as_str())
            };
            if !is_name(words[index]) {
                index += 1;
                continue;
            }
            let start = index;
            while index + 1 < words.len() && is_name(words[index + 1]) {
                index += 1;
            }
            phrases.push(words[start..=index].join(" "));
            index += 1;
        }
    }
    phrases
}

/// Drops `{...}` placeholders and highlighted spans so only running text is
/// scanned for capitalized names.
fn strip_for_words(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    for span in highlight_spans(value) {
        output.push_str(&value[cursor..span.start]);
        output.push(' ');
        cursor = span.end;
    }
    output.push_str(&value[cursor..]);
    let mut result = String::with_capacity(output.len());
    let mut depth = 0usize;
    for ch in output.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

fn example_line(value: &str) -> String {
    let line = value.replace(['\n', '\r'], " ");
    let line = line.trim();
    if line.chars().count() <= MAX_EXAMPLE_CHARS {
        line.to_string()
    } else {
        format!(
            "{}…",
            line.chars().take(MAX_EXAMPLE_CHARS).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, source: &str, translated: &str) -> JsonTranslationEntry {
        JsonTranslationEntry {
            key: format!("file://cards.json#/{key}"),
            slot_id: None,
            previous_source_value: None,
            source_value: source.to_string(),
            translated_value: translated.to_string(),
            status: JsonTranslationStatus::Ready,
        }
    }

    fn sheet(entries: Vec<JsonTranslationEntry>) -> JsonTranslationSheet {
        JsonTranslationSheet {
            source_path: "Z:/mods/sample/localization/eng".to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries,
        }
    }

    #[test]
    fn finds_highlights_names_and_capitalized_words_with_suggestions() {
        let sheet = sheet(vec![
            entry(
                "A.description",
                "Gain 2 [gold]Ammo[/gold] while Koyuki laughs.",
                "[gold]탄약[/gold]을 2 얻습니다.",
            ),
            entry(
                "B.description",
                "Spend [gold]Ammo[/gold] and Koyuki cheers.",
                "[gold]탄약[/gold]을 소모합니다.",
            ),
            entry(
                "C.description",
                "Lose [gold]Ammo[/gold] if Koyuki is here.",
                "",
            ),
            entry(
                "D.description",
                "Gain [gold]Stealth[/gold] and [blue]Concealment[/blue].",
                "[gold]은신[/gold]과 [blue]은신[/blue]을 얻습니다.",
            ),
            entry(
                "E.description",
                "Gain [gold]Stealth[/gold] and [blue]Concealment[/blue].",
                "",
            ),
            entry("WAKAMO.title", "Wakamo's Mark", "와카모의 표식"),
            entry("F.description", "Apply [gold]Once[/gold]. What now?", ""),
            entry(
                "G.description",
                "[gold]How will you deal with this?[/gold]",
                "",
            ),
            entry(
                "H.description",
                "[gold]How will you deal with this?[/gold]",
                "",
            ),
        ]);
        let candidates = extract_term_candidates(&sheet, "eng", None, &[]);
        let find = |name: &str| candidates.iter().find(|candidate| candidate.source == name);

        let ammo = find("Ammo").expect("ammo");
        assert_eq!(ammo.count, 3);
        assert_eq!(ammo.suggestions, vec![("탄약".to_string(), 2)]);
        assert!(ammo.example.contains("Ammo"));

        let koyuki = find("Koyuki").expect("koyuki");
        assert_eq!(koyuki.kinds, vec!["capitalized"]);

        assert_eq!(
            find("Wakamo's Mark").expect("title").suggestions[0].0,
            "와카모의 표식"
        );
        assert_eq!(
            find("Stealth").expect("stealth").shares_translation_with,
            vec!["Concealment"]
        );
        assert!(find("Once").is_none(), "single highlight is skipped");
        assert!(
            find("How will you deal with this?").is_none(),
            "sentences are not terms"
        );
    }

    #[test]
    fn skips_known_terms() {
        let sheet = sheet(vec![
            entry("A.description", "Gain [gold]Ammo[/gold].", ""),
            entry("B.description", "Gain [gold]Ammo[/gold].", ""),
        ]);
        let known = vec![GlossaryTerm {
            source: "ammo".to_string(),
            targets: vec!["탄약".to_string()],
        }];
        assert!(extract_term_candidates(&sheet, "eng", None, &known).is_empty());
    }
}
