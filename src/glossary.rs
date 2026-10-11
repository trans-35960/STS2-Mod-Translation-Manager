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
use crate::json_translation::{
    JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus, JsonValidationIssue,
};
use crate::pck_reader::read_pck_files;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub const GLOSSARY_ISSUE_KIND: &str = "glossary";
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
    fn lookup(&self, phrase: &str) -> Option<&GlossaryTerm> {
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
    {
        if cached_signature == signature {
            return Ok(Some(glossary));
        }
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
    GameGlossary {
        source_language: source_language.to_string(),
        target_language: target_language.to_string(),
        terms,
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

fn has_markup(value: &str) -> bool {
    value.contains(['[', ']', '{', '}', '<', '>'])
}

/// Picks the game language folder named in the sheet's source path
/// (`.../localization/eng`) or, for sheets built from a copied folder, in the
/// entry keys (`file://Mod/localization/jpn/cards.json#/...`). Defaults to
/// English.
pub fn sheet_source_language(sheet: &JsonTranslationSheet) -> String {
    let from_path = language_component(sheet.source_path.split(['/', '\\']).rev());
    from_path
        .or_else(|| {
            sheet.entries.iter().take(32).find_map(|entry| {
                let file = entry.key.strip_prefix("file://")?.split('#').next()?;
                language_component(file.split('/').rev())
            })
        })
        .unwrap_or_else(|| DEFAULT_SOURCE_LANGUAGE.to_string())
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
pub fn glossary_issues(
    sheet: &JsonTranslationSheet,
    game: Option<&GameGlossary>,
    user_terms: &[GlossaryTerm],
) -> Vec<JsonValidationIssue> {
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

        for (lower, term) in &user_terms {
            if contains_word(&source_lower, lower) && seen.insert(lower.clone()) {
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
        for term in expected
            .into_iter()
            .filter(|term| !uses_any_target(&translated_lower, term))
            .take(MAX_ISSUES_PER_ENTRY)
        {
            issues.push(JsonValidationIssue {
                key: entry.key.clone(),
                kind: GLOSSARY_ISSUE_KIND.to_string(),
                message: format!(
                    "용어집: '{}' → '{}'",
                    term.source,
                    term.targets.join("' / '")
                ),
            });
        }
    }
    issues
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

fn contains_word(haystack: &str, needle: &str) -> bool {
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
        assert_eq!(issues[0].kind, GLOSSARY_ISSUE_KIND);
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
}
