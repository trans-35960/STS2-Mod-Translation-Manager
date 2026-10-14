//! Term consistency checks for translation sheets.
//!
//! Two term lists are used:
//! - the game's official names (card, power, relic, keyword titles), read from
//!   the user's own `SlayTheSpire2.pck` so they always match the installed game;
//! - a user glossary per target language for mod-specific names.
//!
//! Official names are only checked inside `[gold]...[/gold]` highlights, which
//! is how game and mod text mark keywords; plain words like "Strike" or "Block"
//! elsewhere in a sentence are too ambiguous. User terms are checked anywhere
//! as whole words because the user chose them deliberately.

use crate::error::{AppError, AppResult};
use crate::json_translation::{JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus};
use crate::pck_reader::read_pck_files;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub const GLOSSARY_ISSUE_KIND: &str = "glossary";
pub const JOSA_ISSUE_KIND: &str = "josa";
const GAME_LOCALIZATION_PREFIX: &str = "localization/";
const DEFAULT_SOURCE_LANGUAGE: &str = "eng";
const TITLE_KEY_SUFFIX: &str = ".title";
const MAX_ISSUES_PER_ENTRY: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryTerm {
    pub source: String,
    pub targets: Vec<String>,
}

/// Official game names for one source → target language pair.
#[derive(Debug, Default)]
pub struct GameGlossary {
    pub source_language: String,
    pub target_language: String,
    terms: HashMap<String, GlossaryTerm>,
    /// Every official target name, for recognizing terms in translations.
    target_terms: HashSet<String>,
}

impl GameGlossary {
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Looks up a highlighted phrase, allowing simple English plurals and
    /// possessives ("Shivs", "Osty's").
    pub(crate) fn lookup(&self, phrase: &str) -> Option<&GlossaryTerm> {
        let lower = phrase.trim().to_lowercase();
        let candidates = [
            Some(lower.as_str()),
            lower.strip_suffix("'s"),
            lower.strip_suffix("es"),
            lower.strip_suffix('s'),
        ];
        candidates
            .into_iter()
            .flatten()
            .find_map(|candidate| self.terms.get(candidate))
    }
}

type GlossaryCacheKey = (PathBuf, String, String);
type GlossaryCacheValue = ((u64, Option<SystemTime>), Arc<GameGlossary>);

/// Reads (and memoizes per pack file version) the official term list.
/// Returns `None` when the pack cannot be read natively or lacks one of the
/// languages.
pub fn load_game_glossary(
    pck_path: &Path,
    source_language: &str,
    target_language: &str,
) -> AppResult<Option<Arc<GameGlossary>>> {
    static CACHE: OnceLock<Mutex<HashMap<GlossaryCacheKey, GlossaryCacheValue>>> = OnceLock::new();
    let metadata = fs::metadata(pck_path).map_err(|source| AppError::io(pck_path, source))?;
    let signature = (metadata.len(), metadata.modified().ok());
    let key = (
        pck_path.to_path_buf(),
        source_language.to_string(),
        target_language.to_string(),
    );
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((cached_signature, glossary)) =
        cache.lock().ok().and_then(|cache| cache.get(&key).cloned())
        && cached_signature == signature
    {
        return Ok(Some(glossary));
    }

    let source_prefix = format!("{GAME_LOCALIZATION_PREFIX}{source_language}/");
    let target_prefix = format!("{GAME_LOCALIZATION_PREFIX}{target_language}/");
    let Some(files) = read_pck_files(pck_path, |path| {
        is_top_level_json(path, &source_prefix) || is_top_level_json(path, &target_prefix)
    })?
    else {
        return Ok(None);
    };
    let mut source_files = HashMap::new();
    let mut target_files = HashMap::new();
    for (path, data) in files {
        if let Some(name) = path.strip_prefix(&source_prefix) {
            source_files.insert(name.to_string(), data);
        } else if let Some(name) = path.strip_prefix(&target_prefix) {
            target_files.insert(name.to_string(), data);
        }
    }
    if source_files.is_empty() || target_files.is_empty() {
        return Ok(None);
    }
    let glossary = Arc::new(build_game_glossary(
        source_language,
        target_language,
        &source_files,
        &target_files,
    ));
    if let Ok(mut cache) = cache.lock() {
        cache.insert(key, (signature, glossary.clone()));
    }
    Ok(Some(glossary))
}

fn is_top_level_json(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|name| !name.contains('/') && name.ends_with(".json"))
}

fn build_game_glossary(
    source_language: &str,
    target_language: &str,
    source_files: &HashMap<String, Vec<u8>>,
    target_files: &HashMap<String, Vec<u8>>,
) -> GameGlossary {
    let mut terms: HashMap<String, GlossaryTerm> = HashMap::new();
    for (name, source_data) in source_files {
        let Some(target_data) = target_files.get(name) else {
            continue;
        };
        let (Some(source_map), Some(target_map)) =
            (string_map(source_data), string_map(target_data))
        else {
            continue;
        };
        for (key, source_value) in &source_map {
            if !key.ends_with(TITLE_KEY_SUFFIX) {
                continue;
            }
            let Some(target_value) = target_map.get(key) else {
                continue;
            };
            let (source_value, target_value) = (source_value.trim(), target_value.trim());
            if source_value.is_empty() || target_value.is_empty() || has_markup(source_value) {
                continue;
            }
            let term = terms
                .entry(source_value.to_lowercase())
                .or_insert_with(|| GlossaryTerm {
                    source: source_value.to_string(),
                    targets: Vec::new(),
                });
            if !term.targets.iter().any(|existing| existing == target_value) {
                term.targets.push(target_value.to_string());
            }
        }
    }
    let target_terms = terms
        .values()
        .flat_map(|term| term.targets.iter().cloned())
        .collect();
    GameGlossary {
        source_language: source_language.to_string(),
        target_language: target_language.to_string(),
        terms,
        target_terms,
    }
}

fn string_map(data: &[u8]) -> Option<HashMap<String, String>> {
    let text = std::str::from_utf8(data).ok()?;
    let value: serde_json::Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    Some(
        value
            .as_object()?
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
            .collect(),
    )
}

pub(crate) fn has_markup(value: &str) -> bool {
    value.contains(['[', ']', '{', '}', '<', '>'])
}

/// Picks the game language folder named in the sheet's source path
/// (`.../localization/eng`) or, for sheets built from a copied folder, in the
/// entry keys (`file://Mod/localization/jpn/cards.json#/...`). Sheets without
/// one (DLL strings) are judged by the script of their text; English is the
/// default.
pub fn sheet_source_language(sheet: &JsonTranslationSheet) -> String {
    named_source_language(
        &sheet.source_path,
        sheet
            .entries
            .iter()
            .take(32)
            .map(|entry| entry.key.as_str()),
    )
    .or_else(|| {
        script_language(
            sheet
                .entries
                .iter()
                .map(|entry| entry.source_value.as_str()),
        )
    })
    .unwrap_or_else(|| DEFAULT_SOURCE_LANGUAGE.to_string())
}

/// The language whose script most of `texts` is written in. Latin text says
/// nothing (identifiers look the same in every language); any kana means
/// Japanese, since Japanese text also uses Han.
fn script_language<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let (mut han, mut kana, mut hangul, mut cyrillic) = (0usize, 0usize, 0usize, 0usize);
    for character in texts.into_iter().flat_map(str::chars) {
        match character {
            '\u{3040}'..='\u{30ff}' => kana += 1,
            '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' => han += 1,
            '\u{ac00}'..='\u{d7af}' => hangul += 1,
            '\u{0400}'..='\u{04ff}' => cyrillic += 1,
            _ => {}
        }
    }
    if kana > 0 {
        return Some("jpn".to_string());
    }
    [(han, "zhs"), (hangul, "kor"), (cyrillic, "rus")]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .max_by_key(|(count, _)| *count)
        .map(|(_, language)| language.to_string())
}

/// The source language from a sheet's source folder, or else from its entry
/// keys (`file://Mod/localization/<lang>/...`), which name it even when the
/// folder is a cache directory.
pub fn source_language_of<'a>(
    source_path: &str,
    entry_keys: impl IntoIterator<Item = &'a str>,
) -> String {
    named_source_language(source_path, entry_keys)
        .unwrap_or_else(|| DEFAULT_SOURCE_LANGUAGE.to_string())
}

fn named_source_language<'a>(
    source_path: &str,
    entry_keys: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    language_component(source_path.split(['/', '\\']).rev()).or_else(|| {
        entry_keys.into_iter().find_map(|key| {
            let file = key.strip_prefix("file://")?.split('#').next()?;
            language_component(file.split('/').rev())
        })
    })
}

fn language_component<'a>(parts: impl Iterator<Item = &'a str>) -> Option<String> {
    parts
        .map(str::to_lowercase)
        .find(|name| is_game_language_code(name))
}

fn is_game_language_code(name: &str) -> bool {
    matches!(
        name,
        "deu"
            | "eng"
            | "esp"
            | "fra"
            | "ind"
            | "ita"
            | "jpn"
            | "kor"
            | "pol"
            | "ptb"
            | "rus"
            | "spa"
            | "tha"
            | "tur"
            | "zhs"
            | "zht"
    )
}

pub fn user_glossary_path(state_dir: &Path, target_language: &str) -> PathBuf {
    let safe = target_language
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>();
    state_dir.join("glossary").join(format!(
        "{}.tsv",
        if safe.is_empty() { "default" } else { &safe }
    ))
}

/// Reads `source<TAB>target` lines; blank lines and `#` comments are ignored.
pub fn read_user_glossary(state_dir: &Path, target_language: &str) -> AppResult<Vec<GlossaryTerm>> {
    let path = user_glossary_path(state_dir, target_language);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?;
    let mut terms: Vec<GlossaryTerm> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((source, target)) = line.split_once('\t') else {
            continue;
        };
        let (source, target) = (source.trim(), target.trim());
        if source.is_empty() || target.is_empty() {
            continue;
        }
        match terms
            .iter_mut()
            .find(|term| term.source.eq_ignore_ascii_case(source))
        {
            Some(term) => {
                if !term.targets.iter().any(|existing| existing == target) {
                    term.targets.push(target.to_string());
                }
            }
            None => terms.push(GlossaryTerm {
                source: source.to_string(),
                targets: vec![target.to_string()],
            }),
        }
    }
    Ok(terms)
}

pub fn write_user_glossary(
    state_dir: &Path,
    target_language: &str,
    terms: &[GlossaryTerm],
) -> AppResult<()> {
    let path = user_glossary_path(state_dir, target_language);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let mut content = String::from("# source\ttarget\n");
    for term in terms {
        let source = clean_cell(&term.source);
        for target in &term.targets {
            let target = clean_cell(target);
            if !source.is_empty() && !target.is_empty() {
                content.push_str(&format!("{source}\t{target}\n"));
            }
        }
    }
    fs::write(&path, content).map_err(|source| AppError::io(&path, source))
}

fn clean_cell(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

/// Reports translations that do not use the expected name for a term found
/// in the source text.
/// A glossary mismatch, with a corrected translation when the term's
/// highlighted spot in the translation could be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryIssue {
    pub key: String,
    pub message: String,
    pub original: String,
    pub suggestion: Option<String>,
}

pub fn glossary_issues(
    sheet: &JsonTranslationSheet,
    game: Option<&GameGlossary>,
    user_terms: &[GlossaryTerm],
) -> Vec<GlossaryIssue> {
    let korean = is_korean_language(&sheet.target_language);
    let user_terms = user_terms
        .iter()
        .map(|term| (term.source.to_lowercase(), term))
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    for entry in &sheet.entries {
        if !should_check(entry) {
            continue;
        }
        let source_lower = entry.source_value.to_lowercase();
        let mut expected: Vec<&GlossaryTerm> = Vec::new();
        let mut seen = BTreeSet::new();

        // Highlighted words may be plural ("[gold]Shivs[/gold]"); plain text
        // must match the term exactly so other names ("Artorias") stay out.
        let highlighted = highlight_spans(&entry.source_value);
        for (lower, term) in &user_terms {
            let named = mentions_term(&source_lower, lower)
                || highlighted
                    .iter()
                    .any(|span| phrase_matches(span.content, &term.source));
            if named && seen.insert(lower.clone()) {
                expected.push(term);
            }
        }
        if let Some(game) = game {
            for phrase in gold_spans(&entry.source_value) {
                if has_markup(phrase) {
                    continue;
                }
                if let Some(term) = game.lookup(phrase) {
                    let lower = term.source.to_lowercase();
                    // A user entry for the same word overrides the game name.
                    if seen.insert(lower) {
                        expected.push(term);
                    }
                }
            }
        }

        let translated_lower = entry.translated_value.to_lowercase();
        let missing = expected
            .into_iter()
            .filter(|term| !uses_any_target(&translated_lower, term))
            .take(MAX_ISSUES_PER_ENTRY)
            .collect::<Vec<_>>();
        if missing.is_empty() {
            continue;
        }
        let fix = replace_term_variants(entry, &missing, korean);
        for (index, term) in missing.iter().enumerate() {
            let replaced = fix
                .as_ref()
                .and_then(|fix| {
                    fix.replaced
                        .iter()
                        .find(|(source, _)| source == &term.source)
                })
                .map(|(_, variant)| format!(" · 치환: {variant} → {}", term.targets[0]))
                .unwrap_or_default();
            issues.push(GlossaryIssue {
                key: entry.key.clone(),
                message: format!(
                    "용어집: '{}' → '{}'{replaced}",
                    term.source,
                    term.targets.join("' / '")
                ),
                original: entry.translated_value.clone(),
                // One combined fix per entry, on its first issue.
                suggestion: (index == 0)
                    .then(|| fix.as_ref().map(|fix| fix.value.clone()))
                    .flatten(),
            });
        }
    }
    issues
}

struct TermFix {
    value: String,
    /// `(term source, variant that was replaced)`
    replaced: Vec<(String, String)>,
}

/// Rewrites the translation's highlighted spans that correspond (by tag
/// order) to a missing term's highlight in the source, so a split
/// translation ("위압 오라") becomes the glossary one ("위압감"). For Korean,
/// a particle right after the term is re-chosen for the new final consonant.
fn replace_term_variants(
    entry: &JsonTranslationEntry,
    missing: &[&GlossaryTerm],
    korean: bool,
) -> Option<TermFix> {
    let source_spans = highlight_spans(&entry.source_value);
    let target_spans = highlight_spans(&entry.translated_value);
    let same_shape = !source_spans.is_empty()
        && source_spans.len() == target_spans.len()
        && source_spans
            .iter()
            .zip(&target_spans)
            .all(|(left, right)| left.tag == right.tag);
    if !same_shape {
        return None;
    }
    let value = &entry.translated_value;
    let mut output = String::with_capacity(value.len() + 8);
    let mut replaced = Vec::new();
    let mut cursor = 0;
    for (source, target) in source_spans.iter().zip(&target_spans) {
        let Some(term) = missing
            .iter()
            .find(|term| phrase_matches(source.content, &term.source))
        else {
            continue;
        };
        let canonical = term.targets[0].as_str();
        let content = target.content.trim();
        let (stem, inner_josa) = if korean {
            split_trailing_josa(content)
        } else {
            (content, "")
        };
        if stem.is_empty() || stem == canonical || has_markup(stem) {
            continue;
        }
        output.push_str(&value[cursor..target.start]);
        let inner = if inner_josa.is_empty() {
            String::new()
        } else {
            adjust_josa(canonical, inner_josa)
        };
        output.push_str(&format!("[{0}]{canonical}{inner}[/{0}]", target.tag));
        cursor = target.end;
        if korean
            && inner_josa.is_empty()
            && let Some(josa) = leading_josa(&value[cursor..])
        {
            output.push_str(&adjust_josa(canonical, josa));
            cursor += josa.len();
        }
        replaced.push((term.source.clone(), stem.to_string()));
    }
    if replaced.is_empty() {
        return None;
    }
    output.push_str(&value[cursor..]);
    Some(TermFix {
        value: output,
        replaced,
    })
}

/// Highlighted source text naming the term, allowing English plurals and
/// possessives like the game glossary lookup does.
fn phrase_matches(phrase: &str, term: &str) -> bool {
    let phrase = phrase.trim().to_lowercase();
    let term = term.trim().to_lowercase();
    phrase == term
        || [
            phrase.strip_suffix("'s"),
            phrase.strip_suffix("es"),
            phrase.strip_suffix('s'),
        ]
        .into_iter()
        .flatten()
        .any(|candidate| candidate == term)
}

/// Whole-word match for spaced scripts; plain substring for CJK text, which
/// has no spaces between words.
pub(crate) fn mentions_term(source_lower: &str, term_lower: &str) -> bool {
    if !term_lower.is_ascii() {
        !term_lower.is_empty() && source_lower.contains(term_lower)
    } else {
        contains_word(source_lower, term_lower)
    }
}

/// Particle pairs chosen by the preceding final consonant:
/// (after a consonant, after a vowel).
const JOSA_PAIRS: &[(&str, &str)] = &[
    ("으로서", "로서"),
    ("으로써", "로써"),
    ("으로는", "로는"),
    ("으로", "로"),
    ("이랑", "랑"),
    ("이나", "나"),
    ("을", "를"),
    ("이", "가"),
    ("은", "는"),
    ("과", "와"),
];

fn split_trailing_josa(content: &str) -> (&str, &str) {
    KOREAN_JOSA
        .iter()
        .find(|josa| content.len() > josa.len() + 3 && content.ends_with(*josa))
        .map(|josa| {
            (
                content[..content.len() - josa.len()].trim_end(),
                &content[content.len() - josa.len()..],
            )
        })
        .unwrap_or((content, ""))
}

fn leading_josa(rest: &str) -> Option<&'static str> {
    JOSA_PAIRS
        .iter()
        .flat_map(|(with, without)| [*with, *without])
        .filter(|josa| rest.starts_with(josa))
        .max_by_key(|josa| josa.len())
}

/// Re-chooses a particle for `word` (을/를, 이/가, 으로/로 ...). Unknown
/// particles, or words not ending in a Hangul syllable, keep the particle.
fn adjust_josa(word: &str, josa: &str) -> String {
    let Some(&(with, without)) = JOSA_PAIRS
        .iter()
        .find(|(with, without)| *with == josa || *without == josa)
    else {
        return josa.to_string();
    };
    let Some(last) = word
        .chars()
        .last()
        .filter(|ch| ('\u{AC00}'..='\u{D7A3}').contains(ch))
    else {
        return josa.to_string();
    };
    let final_consonant = (last as u32 - 0xAC00) % 28;
    let uses_consonant_form = if with.starts_with('으') {
        // ㄹ (index 8) takes 로 like a vowel does.
        final_consonant != 0 && final_consonant != 8
    } else {
        final_consonant != 0
    };
    (if uses_consonant_form { with } else { without }).to_string()
}

/// Korean particles, longest first so `으로` wins over `로`.
pub(crate) const KOREAN_JOSA: &[&str] = &[
    "으로부터",
    "에서부터",
    "으로서",
    "으로써",
    "에게서",
    "에서는",
    "에게는",
    "으로는",
    "에서",
    "에게",
    "한테",
    "으로",
    "께서",
    "이나",
    "이랑",
    "까지",
    "부터",
    "보다",
    "처럼",
    "마다",
    "이며",
    "이고",
    "과",
    "와",
    "을",
    "를",
    "이",
    "가",
    "은",
    "는",
    "의",
    "에",
    "로",
    "도",
    "만",
];

/// Single-syllable particles that rarely end a noun, so they are trusted
/// without the source text confirming the term ("방어도" is a word, not
/// "방어" + "도").
const UNAMBIGUOUS_JOSA: &[&str] = &["을", "를", "은", "는", "과", "와", "에"];

/// A translation whose highlight tags swallowed a Korean particle, such as
/// `[gold]취약을[/gold]`, with the corrected value `[gold]취약[/gold]을`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JosaIssue {
    pub key: String,
    pub message: String,
    pub original: String,
    pub suggestion: String,
}

pub fn is_korean_language(language: &str) -> bool {
    let language = language.trim().to_ascii_lowercase();
    language == "kor"
        || language == "ko"
        || language.starts_with("ko-")
        || language.starts_with("ko_")
}

/// Finds particles inside highlight tags. A highlighted word only counts as
/// "term + particle" when the bare term is known: the official name of a term
/// highlighted in this entry's source, any official or user glossary name
/// (with an unambiguous particle), or a word this sheet highlights on its own
/// elsewhere (with an unambiguous particle).
pub fn korean_josa_issues(
    sheet: &JsonTranslationSheet,
    game: Option<&GameGlossary>,
    user_terms: &[GlossaryTerm],
) -> Vec<JosaIssue> {
    if !is_korean_language(&sheet.target_language) {
        return Vec::new();
    }
    let user_targets = user_terms
        .iter()
        .flat_map(|term| term.targets.iter().map(String::as_str))
        .collect::<HashSet<_>>();
    let standalone = sheet
        .entries
        .iter()
        .flat_map(|entry| highlight_spans(&entry.translated_value))
        .map(|span| span.content)
        .collect::<HashSet<_>>();
    let is_known_word = |word: &str| {
        user_targets.contains(word) || game.is_some_and(|game| game.target_terms.contains(word))
    };

    let mut issues = Vec::new();
    for entry in sheet.entries.iter().filter(|entry| should_check(entry)) {
        let expected = game
            .map(|game| {
                highlight_spans(&entry.source_value)
                    .into_iter()
                    .filter(|span| !has_markup(span.content))
                    .filter_map(|span| game.lookup(span.content))
                    .flat_map(|term| term.targets.iter().map(String::as_str))
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();

        let value = &entry.translated_value;
        let mut suggestion = String::with_capacity(value.len() + 8);
        let mut moved = Vec::new();
        let mut cursor = 0;
        for span in highlight_spans(value) {
            let content = span.content;
            if has_markup(content) || is_known_word(content) {
                continue;
            }
            let Some(josa) = KOREAN_JOSA
                .iter()
                .find(|josa| content.len() > josa.len() && content.ends_with(*josa))
            else {
                continue;
            };
            let stem = &content[..content.len() - josa.len()];
            let trusted = josa.chars().count() >= 2 || UNAMBIGUOUS_JOSA.contains(josa);
            if !(expected.contains(stem)
                || (trusted && (is_known_word(stem) || standalone.contains(stem))))
            {
                continue;
            }
            suggestion.push_str(&value[cursor..span.start]);
            suggestion.push_str(&format!("[{0}]{stem}[/{0}]{josa}", span.tag));
            cursor = span.end;
            moved.push(format!(
                "[{0}]{content}[/{0}] → [{0}]{stem}[/{0}]{josa}",
                span.tag
            ));
        }
        if moved.is_empty() {
            continue;
        }
        suggestion.push_str(&value[cursor..]);
        issues.push(JosaIssue {
            key: entry.key.clone(),
            message: format!("조사가 강조 태그 안에 있습니다: {}", moved.join(", ")),
            original: value.clone(),
            suggestion,
        });
    }
    issues
}

pub(crate) struct HighlightSpan<'a> {
    pub(crate) tag: &'a str,
    pub(crate) content: &'a str,
    /// Byte range of the whole `[tag]content[/tag]`.
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Non-nested `[tag]...[/tag]` pairs with a lowercase ASCII tag name, the
/// game's color/style markup (`gold`, `blue`, `green`, ...).
pub(crate) fn highlight_spans(value: &str) -> Vec<HighlightSpan<'_>> {
    let bytes = value.as_bytes();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while let Some(open) = value[cursor..].find('[').map(|index| cursor + index) {
        let name_end = bytes[open + 1..]
            .iter()
            .position(|byte| !byte.is_ascii_lowercase())
            .map(|offset| open + 1 + offset);
        let Some(name_end) = name_end.filter(|end| *end > open + 1 && bytes[*end] == b']') else {
            cursor = open + 1;
            continue;
        };
        let tag = &value[open + 1..name_end];
        let content_start = name_end + 1;
        let close = format!("[/{tag}]");
        let Some(content_end) = value[content_start..]
            .find(&close)
            .map(|index| content_start + index)
        else {
            cursor = content_start;
            continue;
        };
        let content = &value[content_start..content_end];
        if content.contains('[') {
            cursor = content_start;
            continue;
        }
        let end = content_end + close.len();
        spans.push(HighlightSpan {
            tag,
            content,
            start: open,
            end,
        });
        cursor = end;
    }
    spans
}

/// Terms worth handing to a translator for these entries: every user term,
/// plus the official names of terms highlighted in the included sources.
/// User terms come first and win over a game name for the same word.
pub fn sheet_glossary_terms(
    sheet: &JsonTranslationSheet,
    game: Option<&GameGlossary>,
    user_terms: &[GlossaryTerm],
    include_keys: Option<&BTreeSet<String>>,
) -> Vec<GlossaryTerm> {
    let mut seen = user_terms
        .iter()
        .map(|term| term.source.to_lowercase())
        .collect::<HashSet<_>>();
    let mut terms = user_terms.to_vec();
    let Some(game) = game else {
        return terms;
    };
    let mut game_terms = Vec::new();
    for entry in &sheet.entries {
        if include_keys.is_some_and(|keys| !keys.contains(&entry.key)) {
            continue;
        }
        for span in highlight_spans(&entry.source_value) {
            if has_markup(span.content) {
                continue;
            }
            if let Some(term) = game.lookup(span.content)
                && seen.insert(term.source.to_lowercase())
            {
                game_terms.push(term.clone());
            }
        }
    }
    game_terms.sort_by(|left, right| left.source.to_lowercase().cmp(&right.source.to_lowercase()));
    terms.extend(game_terms);
    terms
}

/// Glossary terms that apply to one source text: user terms found as whole
/// words, then official names of highlighted terms. A user term overrides the
/// game name for the same word.
pub fn entry_glossary_terms<'a>(
    source: &str,
    game: Option<&'a GameGlossary>,
    user_terms: &'a [GlossaryTerm],
) -> Vec<&'a GlossaryTerm> {
    let source_lower = source.to_lowercase();
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for term in user_terms {
        let lower = term.source.to_lowercase();
        if mentions_term(&source_lower, &lower) && seen.insert(lower) {
            terms.push(term);
        }
    }
    if let Some(game) = game {
        for span in highlight_spans(source) {
            if has_markup(span.content) {
                continue;
            }
            if let Some(term) = game.lookup(span.content)
                && seen.insert(term.source.to_lowercase())
            {
                terms.push(term);
            }
        }
    }
    terms
}

/// Project terms first so they win over language-wide terms for the same word.
pub fn merge_glossary_terms(
    primary: &[GlossaryTerm],
    secondary: &[GlossaryTerm],
) -> Vec<GlossaryTerm> {
    let mut seen = HashSet::new();
    primary
        .iter()
        .chain(secondary)
        .filter(|term| seen.insert(term.source.to_lowercase()))
        .cloned()
        .collect()
}

fn should_check(entry: &JsonTranslationEntry) -> bool {
    entry.status != JsonTranslationStatus::Removed
        && !entry.translated_value.trim().is_empty()
        // An untranslated copy of the source is reported elsewhere.
        && entry.translated_value != entry.source_value
}

fn uses_any_target(translated_lower: &str, term: &GlossaryTerm) -> bool {
    term.targets.iter().any(|target| {
        let target = target.to_lowercase();
        // Names kept as-is in the translation are not a mismatch.
        translated_lower.contains(&target) || target == term.source.to_lowercase()
    })
}

fn gold_spans(value: &str) -> Vec<&str> {
    const OPEN: &str = "[gold]";
    const CLOSE: &str = "[/gold]";
    let lower = value.to_ascii_lowercase();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..]
        .find(OPEN)
        .map(|index| cursor + index + OPEN.len())
    {
        let Some(end) = lower[start..].find(CLOSE).map(|index| start + index) else {
            break;
        };
        spans.push(&value[start..end]);
        cursor = end + CLOSE.len();
    }
    spans
}

pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(index) = haystack[from..].find(needle).map(|index| from + index) {
        let end = index + needle.len();
        let before = haystack[..index].chars().next_back();
        let after = haystack[end..].chars().next();
        if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
            return true;
        }
        from = index + haystack[index..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, translated: &str) -> JsonTranslationEntry {
        JsonTranslationEntry {
            key: format!("file://cards.json#/{source}"),
            slot_id: None,
            previous_source_value: None,
            source_value: source.to_string(),
            translated_value: translated.to_string(),
            status: JsonTranslationStatus::Ready,
            acknowledged_issues: Vec::new(),
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

    fn game() -> GameGlossary {
        let source = HashMap::from([(
            "powers.json".to_string(),
            br#"{"VULNERABLE_POWER.title":"Vulnerable","SHIV.title":"Shiv","X.description":"Ignored","Y.title":"[gold]Skip[/gold]"}"#.to_vec(),
        )]);
        let target = HashMap::from([(
            "powers.json".to_string(),
            r#"{"VULNERABLE_POWER.title":"취약","SHIV.title":"단도","X.description":"무시","Y.title":"건너뜀"}"#
                .as_bytes()
                .to_vec(),
        )]);
        build_game_glossary("eng", "kor", &source, &target)
    }

    #[test]
    fn builds_terms_from_title_keys_only() {
        let game = game();
        assert_eq!(game.len(), 2);
        assert_eq!(
            game.lookup("Vulnerable").map(|term| term.targets.clone()),
            Some(vec!["취약".to_string()])
        );
        assert!(game.lookup("Shivs").is_some(), "plural lookup");
    }

    #[test]
    fn flags_highlighted_game_terms_missing_from_translation() {
        let game = game();
        let sheet = sheet(vec![
            entry(
                "Apply 2 [gold]Vulnerable[/gold].",
                "[gold]취약[/gold]을 2 부여합니다.",
            ),
            entry(
                "Apply 2 [gold]Vulnerable[/gold].",
                "[gold]약화[/gold]를 2 부여합니다.",
            ),
            entry(
                "Add 2 [gold]Shivs[/gold].",
                "[gold]단도[/gold] 2장을 추가합니다.",
            ),
            entry("You feel vulnerable.", "약해진 느낌이 듭니다."),
            entry("Apply [gold]Vulnerable[/gold].", ""),
        ]);
        let issues = glossary_issues(&sheet, Some(&game), &[]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, sheet.entries[1].key);
        assert!(issues[0].message.contains("취약"));
    }

    #[test]
    fn user_terms_match_whole_words_and_override_game_names() {
        let game = game();
        let user = vec![
            GlossaryTerm {
                source: "Artoria".to_string(),
                targets: vec!["아르토리아".to_string()],
            },
            GlossaryTerm {
                source: "Vulnerable".to_string(),
                targets: vec!["취약함".to_string()],
            },
        ];
        let sheet = sheet(vec![
            entry("Artoria attacks.", "아르토리아가 공격합니다."),
            entry("Artoria's sword.", "알토리아의 검."),
            entry("Artorias the Abysswalker.", "심연의 감시자."),
            entry(
                "Apply [gold]Vulnerable[/gold].",
                "[gold]취약[/gold]을 부여합니다.",
            ),
        ]);
        let issues = glossary_issues(&sheet, Some(&game), &user);
        let keys = issues
            .iter()
            .map(|issue| issue.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![sheet.entries[1].key.as_str(), sheet.entries[3].key.as_str()]
        );
        assert!(issues[1].message.contains("취약함"));
    }

    #[test]
    fn user_glossary_round_trips() {
        let dir = std::env::temp_dir().join(format!(
            "sts2-glossary-{}",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let terms = vec![GlossaryTerm {
            source: "Artoria".to_string(),
            targets: vec!["아르토리아".to_string(), "알트리아".to_string()],
        }];
        write_user_glossary(&dir, "kor", &terms).expect("write");
        assert_eq!(read_user_glossary(&dir, "kor").expect("read"), terms);
        assert!(read_user_glossary(&dir, "jpn").expect("read").is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn source_language_comes_from_the_sheet_path() {
        let mut value = sheet(Vec::new());
        assert_eq!(sheet_source_language(&value), "eng");
        value.source_path = "Z:/mods/sample/localization/jpn/cards.json".to_string();
        assert_eq!(sheet_source_language(&value), "jpn");
        value.source_path = "Z:/mods/sample/text".to_string();
        assert_eq!(sheet_source_language(&value), "eng");
        value.entries = vec![entry("x", "y")];
        value.entries[0].key = "file://Mod/localization/zhs/cards.json#/A".to_string();
        assert_eq!(sheet_source_language(&value), "zhs");
    }

    #[test]
    fn dll_sheet_source_language_comes_from_its_script() {
        let mut value = sheet(vec![entry("CURSE_INFECTION", ""), entry("灵体嫁接", "")]);
        value.source_path = "Z:/work/source/Mod.dll".to_string();
        value.entries[0].key = "dll://10:4".to_string();
        value.entries[1].key = "dll://20:8".to_string();
        assert_eq!(sheet_source_language(&value), "zhs");
        value.entries[1].source_value = "スキップ".to_string();
        assert_eq!(sheet_source_language(&value), "jpn");
        value.entries[1].source_value = "Refund".to_string();
        assert_eq!(sheet_source_language(&value), "eng");
    }

    #[test]
    fn moves_particles_out_of_highlight_tags() {
        let game = game();
        let sheet = sheet(vec![
            // Source confirms the term, so even an ambiguous particle counts.
            entry(
                "Apply [gold]Vulnerable[/gold].",
                "[gold]취약이[/gold] 부여되고 [gold]단도를[/gold] 얻습니다.",
            ),
            // Already correct.
            entry(
                "Apply [gold]Vulnerable[/gold].",
                "[gold]취약[/gold]을 부여합니다.",
            ),
            // Particle-like ending without a known stem is left alone.
            entry(
                "Gain [gold]Block[/gold].",
                "[gold]방어도[/gold]를 얻습니다.",
            ),
            // Known only from this sheet, with an unambiguous particle.
            entry(
                "[gold]Ammo[/gold] and [gold]Ammo[/gold].",
                "[gold]탄약[/gold], [blue]탄약은[/blue] 소모됩니다.",
            ),
            // Ambiguous particle without confirmation from the source.
            entry("[gold]Randomly[/gold] add.", "[gold]탄약도[/gold] 추가."),
        ]);
        let issues = korean_josa_issues(&sheet, Some(&game), &[]);
        assert_eq!(issues.len(), 2, "{issues:#?}");
        assert_eq!(issues[0].key, sheet.entries[0].key);
        assert_eq!(
            issues[0].suggestion,
            "[gold]취약[/gold]이 부여되고 [gold]단도[/gold]를 얻습니다."
        );
        assert_eq!(
            issues[1].suggestion,
            "[gold]탄약[/gold], [blue]탄약[/blue]은 소모됩니다."
        );

        let mut japanese = sheet.clone();
        japanese.target_language = "jpn".to_string();
        assert!(korean_josa_issues(&japanese, Some(&game), &[]).is_empty());
    }

    #[test]
    fn user_terms_mark_stems_for_particle_checks() {
        let user = vec![GlossaryTerm {
            source: "Artoria".to_string(),
            targets: vec!["아르토리아".to_string()],
        }];
        let ambiguous = sheet(vec![entry(
            "[gold]Artoria[/gold] attacks.",
            "[gold]아르토리아가[/gold] 공격합니다.",
        )]);
        assert!(korean_josa_issues(&ambiguous, None, &user).is_empty());
        let clear = sheet(vec![entry(
            "[gold]Artoria[/gold] attacks.",
            "[gold]아르토리아는[/gold] 공격합니다.",
        )]);
        let issues = korean_josa_issues(&clear, None, &user);
        assert_eq!(
            issues[0].suggestion,
            "[gold]아르토리아[/gold]는 공격합니다."
        );
    }

    #[test]
    fn collects_terms_used_by_the_sheet() {
        let game = game();
        let user = vec![GlossaryTerm {
            source: "Artoria".to_string(),
            targets: vec!["아르토리아".to_string()],
        }];
        let sheet = sheet(vec![
            entry("Add [gold]Shivs[/gold].", ""),
            entry("Apply [blue]Vulnerable[/blue].", ""),
            entry("Plain Vulnerable text.", ""),
        ]);
        let terms = sheet_glossary_terms(&sheet, Some(&game), &user, None);
        let sources = terms
            .iter()
            .map(|term| term.source.as_str())
            .collect::<Vec<_>>();
        assert_eq!(sources, vec!["Artoria", "Shiv", "Vulnerable"]);

        let only_first = BTreeSet::from([sheet.entries[0].key.clone()]);
        let terms = sheet_glossary_terms(&sheet, Some(&game), &[], Some(&only_first));
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].targets, vec!["단도".to_string()]);
    }

    #[test]
    fn entry_terms_prefer_user_terms_and_merge_by_source() {
        let game = game();
        let user = vec![GlossaryTerm {
            source: "Vulnerable".to_string(),
            targets: vec!["취약함".to_string()],
        }];
        let terms = entry_glossary_terms(
            "Apply [gold]Vulnerable[/gold] and [gold]Shiv[/gold].",
            Some(&game),
            &user,
        );
        let pairs = terms
            .iter()
            .map(|term| (term.source.as_str(), term.targets[0].as_str()))
            .collect::<Vec<_>>();
        assert_eq!(pairs, vec![("Vulnerable", "취약함"), ("Shiv", "단도")]);

        let project = vec![GlossaryTerm {
            source: "artoria".to_string(),
            targets: vec!["알트리아".to_string()],
        }];
        let language = vec![
            GlossaryTerm {
                source: "Artoria".to_string(),
                targets: vec!["아르토리아".to_string()],
            },
            user[0].clone(),
        ];
        let merged = merge_glossary_terms(&project, &language);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].targets, vec!["알트리아".to_string()]);
    }

    #[test]
    fn suggests_replacing_split_translations_with_the_glossary_term() {
        let user = vec![
            GlossaryTerm {
                source: "Intimidation Aura".to_string(),
                targets: vec!["위압감".to_string()],
            },
            GlossaryTerm {
                source: "Shiv".to_string(),
                targets: vec!["단검".to_string()],
            },
        ];
        let sheet = sheet(vec![
            // Particle after the tag is re-chosen: 오라를 -> 위압감을.
            entry(
                "Gain [gold]Intimidation Aura[/gold].",
                "[gold]위압 오라[/gold]를 얻습니다.",
            ),
            // Particle inside the tag, plural source, 으로 -> 로 after a vowel.
            entry(
                "Turn into [blue]Shivs[/blue] and [gold]Intimidation Aura[/gold].",
                "[blue]단도로[/blue] 바꾸고 [gold]위압감[/gold]을 얻습니다.",
            ),
            // Tags do not line up: report only.
            entry(
                "Gain [gold]Intimidation Aura[/gold].",
                "위압 오라를 얻습니다.",
            ),
        ]);
        let issues = glossary_issues(&sheet, None, &user);
        assert_eq!(issues.len(), 3, "{issues:#?}");
        assert_eq!(
            issues[0].suggestion.as_deref(),
            Some("[gold]위압감[/gold]을 얻습니다.")
        );
        assert!(issues[0].message.contains("치환: 위압 오라 → 위압감"));
        assert_eq!(
            issues[1].suggestion.as_deref(),
            Some("[blue]단검으로[/blue] 바꾸고 [gold]위압감[/gold]을 얻습니다.")
        );
        assert_eq!(issues[2].suggestion, None);
    }

    #[test]
    fn particles_follow_the_final_consonant() {
        assert_eq!(adjust_josa("위압감", "를"), "을");
        assert_eq!(adjust_josa("단도", "을"), "를");
        assert_eq!(adjust_josa("단검", "로"), "으로");
        assert_eq!(adjust_josa("마을", "으로"), "로");
        assert_eq!(adjust_josa("오라", "이"), "가");
        assert_eq!(adjust_josa("Artoria", "을"), "을");
        assert_eq!(adjust_josa("위압감", "의"), "의");
    }

    #[test]
    fn user_terms_match_inside_cjk_sentences() {
        let user = vec![GlossaryTerm {
            source: "谦逊".to_string(),
            targets: vec!["겸손".to_string()],
        }];
        let mut sheet = sheet(vec![entry(
            "这张牌的谦逊改写尚未实现。",
            "이 카드의 겸허 효과는 아직 없습니다.",
        )]);
        sheet.source_path = "Z:/mods/sample/localization/zhs".to_string();
        let issues = glossary_issues(&sheet, None, &user);
        assert_eq!(issues.len(), 1);
    }
}
