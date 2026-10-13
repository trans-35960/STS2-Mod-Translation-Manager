//! Which string literals of a mod DLL are text worth translating.
//!
//! A DLL sheet lists every literal: type and node names, loc keys, log lines,
//! regexes, and somewhere among them the text players read. The bridge reads
//! each mod DLL in game and records how its code uses every literal
//! (`analysis/{dll}.json`); uses that break when translated, or that nobody
//! reads, rule a literal out. What stays unknown is judged by its shape.

use crate::leftover_text::{contains_foreign_script, is_code_like_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// `analysis/{dll file stem}.json`, written by the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DllAnalysis {
    pub assembly: String,
    /// Size of the DLL that was read; a different size means another build.
    pub size: u64,
    #[serde(default)]
    pub bridge_version: String,
    #[serde(default)]
    pub analyzed_at: String,
    /// Literal → use (`key`, `log`, `shown`, …). Unknown uses are left out.
    #[serde(default)]
    pub uses: BTreeMap<String, String>,
}

impl DllAnalysis {
    pub fn use_of(&self, literal: &str) -> Option<&str> {
        self.uses.get(literal).map(String::as_str)
    }
}

pub fn analysis_path(shared: &Path, dll_file_stem: &str) -> PathBuf {
    shared
        .join("analysis")
        .join(format!("{dll_file_stem}.json"))
}

/// The bridge's analysis of `dll`, if it read this same build.
pub fn analysis_for_dll(shared: &Path, dll: &Path) -> Option<DllAnalysis> {
    let stem = dll.file_stem()?.to_string_lossy();
    let analysis: DllAnalysis =
        serde_json::from_str(&fs::read_to_string(analysis_path(shared, &stem)).ok()?).ok()?;
    let size = fs::metadata(dll).ok()?.len();
    (analysis.size == size).then_some(analysis)
}

/// Uses where the text is never shown, or must not change.
fn is_excluded_use(use_: &str) -> bool {
    matches!(
        use_,
        "key"
            | "searched_for"
            | "contains_key"
            | "reflection"
            | "node"
            | "loc"
            | "exception"
            | "log"
    )
}

/// Foreign-script texts from which a DLL counts as written in that script.
const FOREIGN_TEXT_MAJORITY: usize = 30;

/// Whether Latin sentences in a DLL are likely its text. A mod whose text is
/// in another script (dozens of Chinese strings) uses English only for logs
/// and internal names, so its Latin strings only count once seen on screen.
pub fn latin_is_text<'a>(
    literals: impl IntoIterator<Item = &'a str>,
    target_language: &str,
) -> bool {
    literals
        .into_iter()
        .filter(|literal| {
            contains_foreign_script(literal, target_language) && !is_code_like_text(literal)
        })
        .take(FOREIGN_TEXT_MAJORITY)
        .count()
        < FOREIGN_TEXT_MAJORITY
}

/// Whether a DLL literal is text worth translating. `latin_is_text` is
/// [`latin_is_text`] over the whole DLL.
pub fn is_dll_text(
    literal: &str,
    target_language: &str,
    use_: Option<&str>,
    latin_is_text: bool,
) -> bool {
    match use_ {
        Some("shown") => return true,
        Some(other) if is_excluded_use(other) => return false,
        _ => {}
    }
    if is_code_like_text(literal) {
        return false;
    }
    contains_foreign_script(literal, target_language)
        || (latin_is_text && looks_like_sentence(literal))
}

/// Latin text that reads like words for people: two or more words, no
/// `name=value`, paths, dotted keys, `snake_case` or `[LogTag]` prefixes.
pub fn looks_like_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.contains('=') || trimmed.contains("://") || has_log_tag(trimmed) {
        return false;
    }
    let words = trimmed
        .split_whitespace()
        .map(strip_markup)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.iter().any(|word| is_code_word(word)) {
        return false;
    }
    let real_words = words
        .iter()
        .filter(|word| word.chars().filter(|character| character.is_alphabetic()).count() >= 2)
        .count();
    real_words >= 2
        && trimmed
            .chars()
            .any(|character| character.is_lowercase())
}

/// `[ControlProjection] refresh complete`: a log prefix, unlike `[gold]`.
fn has_log_tag(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('[') else {
        return false;
    };
    let Some((tag, _)) = rest.split_once(']') else {
        return false;
    };
    !tag.is_empty()
        && tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && tag.chars().any(|character| character.is_ascii_uppercase())
}

/// Drops BBCode tags and surrounding punctuation from a word.
fn strip_markup(word: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for character in word.chars() {
        match character {
            '[' => in_tag = true,
            ']' => in_tag = false,
            _ if !in_tag => result.push(character),
            _ => {}
        }
    }
    result
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_string()
}

/// `snake_case`, `dotted.key`, `a/path`, `CamelCaseName`.
fn is_code_word(word: &str) -> bool {
    if word.contains('_') || word.contains('/') || word.contains('\\') {
        return true;
    }
    if let Some((left, right)) = word.split_once('.')
        && left.chars().any(char::is_alphabetic)
        && right.chars().any(char::is_alphabetic)
    {
        return true;
    }
    // A case change inside a word: `GetNodeOrNull`, `cardModifier`.
    let mut humps = 0;
    let mut previous_lower = false;
    for character in word.chars() {
        if character.is_uppercase() && previous_lower {
            humps += 1;
        }
        previous_lower = character.is_lowercase();
    }
    humps >= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_are_text_and_code_is_not() {
        for text in [
            "No items yet. Add one to start editing.",
            "Open Logs Folder",
            "[gold]Retain[/gold] your hand this turn.",
            "Skip 2 card rewards.",
        ] {
            assert!(looks_like_sentence(text), "{text}");
        }
        for text in [
            "CURSE_INFECTION",
            "Refund",
            ", Id = ",
            "page.failed.body",
            "[ControlProjection] projection refresh complete",
            "res://ui/panel.tscn",
            "GetNodeOrNull failed for",
            "event:/sfx/enemy/attack",
        ] {
            assert!(!looks_like_sentence(text), "{text}");
        }
    }

    #[test]
    fn analysis_decides_before_shape() {
        assert!(is_dll_text("撕裂衣服", "kor", None, true));
        assert!(!is_dll_text("撕裂衣服", "kor", Some("searched_for"), true));
        assert!(!is_dll_text("Saved config to disk", "kor", Some("log"), true));
        assert!(is_dll_text("Refund", "kor", Some("shown"), true));
        assert!(!is_dll_text("Refund", "kor", None, true));
        assert!(!is_dll_text("^(?:进行)\\d+$", "kor", None, true));
    }

    #[test]
    fn latin_sentences_only_count_in_latin_mods() {
        let chinese = vec!["造成伤害"; FOREIGN_TEXT_MAJORITY];

        assert!(!latin_is_text(chinese.iter().copied(), "kor"));
        assert!(latin_is_text(["Open Logs Folder", "造成伤害"], "kor"));
        assert!(!is_dll_text("fourth act holy route", "kor", None, false));
        assert!(is_dll_text("fourth act holy route", "kor", Some("shown"), false));
    }

    #[test]
    fn analysis_of_another_build_is_ignored() {
        let dir = std::env::temp_dir().join(format!("sts2-dll-text-{}", std::process::id()));
        let dll = dir.join("Mod.dll");
        fs::create_dir_all(dir.join("analysis")).expect("dirs");
        fs::write(&dll, b"0123456789").expect("dll");
        let write = |size: u64| {
            let analysis = DllAnalysis {
                assembly: "Mod".to_string(),
                size,
                bridge_version: String::new(),
                analyzed_at: String::new(),
                uses: BTreeMap::from([("x".to_string(), "key".to_string())]),
            };
            fs::write(
                analysis_path(&dir, "Mod"),
                serde_json::to_string(&analysis).expect("json"),
            )
            .expect("write");
        };

        write(10);
        let current = analysis_for_dll(&dir, &dll);
        write(11);
        let stale = analysis_for_dll(&dir, &dll);
        fs::remove_dir_all(&dir).ok();

        assert_eq!(current.and_then(|a| a.use_of("x").map(str::to_string)), Some("key".to_string()));
        assert!(stale.is_none());
    }
}
