//! Finds text a translation did not reach.
//!
//! A translation sheet only covers the files it was built from, usually
//! `localization/<lang>/*.json`. Mods also show text from C# string literals
//! (the DLL `#US` heap) and from scenes or data files elsewhere in the pack,
//! and a patch can ship values that were never translated. This scans those
//! places for characters of a script the target language does not use, so the
//! user can see where leftover source text (e.g. Chinese in a Korean play
//! through) actually comes from.

use crate::error::{AppError, AppResult};
use crate::json_translation::{flatten_hardcoded_values, is_hardcoded_source_file};
use crate::pck_reader::read_pck_files;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Files larger than this are art or audio in disguise, not text.
const MAX_TEXT_RESOURCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SNIPPET_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LeftoverLocation {
    /// String literal compiled into a .NET assembly.
    Dll,
    /// Scene, data or script resource outside the localization tables.
    Resource,
    /// Value inside a translation patch that is still in the source script.
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverFile {
    pub location: LeftoverLocation,
    /// Display path: a file path, or `<pck>::res://...` for packed resources.
    pub file: String,
    pub texts: Vec<LeftoverText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverText {
    pub text: String,
    /// Regex patterns and other strings the mod parses at runtime; changing
    /// them breaks the mod rather than translating it.
    pub code_like: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeftoverReport {
    pub files: Vec<LeftoverFile>,
    pub scanned_files: usize,
    /// Archives this scanner could not open (encrypted or unknown `.pck`).
    pub unreadable: Vec<String>,
}

impl LeftoverReport {
    pub fn total_texts(&self) -> usize {
        self.files.iter().map(|file| file.texts.len()).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Han,
    Kana,
    Hangul,
    Cyrillic,
}

fn script_of(character: char) -> Option<Script> {
    match character {
        '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{f900}'..='\u{faff}' => {
            Some(Script::Han)
        }
        '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' => Some(Script::Kana),
        '\u{ac00}'..='\u{d7af}' | '\u{1100}'..='\u{11ff}' | '\u{3130}'..='\u{318f}' => {
            Some(Script::Hangul)
        }
        '\u{0400}'..='\u{04ff}' => Some(Script::Cyrillic),
        _ => None,
    }
}

/// Scripts that belong in text of `target_language`. Latin is never flagged:
/// it is indistinguishable from identifiers and markup.
fn native_scripts(target_language: &str) -> &'static [Script] {
    match target_language.trim().to_ascii_lowercase().as_str() {
        "kor" | "ko" => &[Script::Hangul],
        "jpn" | "ja" => &[Script::Han, Script::Kana],
        "zhs" | "zht" | "zh" | "chs" | "cht" => &[Script::Han],
        "rus" | "ru" | "ukr" | "uk" => &[Script::Cyrillic],
        _ => &[],
    }
}

pub fn contains_foreign_script(text: &str, target_language: &str) -> bool {
    let native = native_scripts(target_language);
    text.chars()
        .filter_map(script_of)
        .any(|script| !native.contains(&script))
}

pub fn is_code_like_text(text: &str) -> bool {
    [
        "(?:", "\\d", "\\s", "\\w", "[^", "(\\", ")+", ")*", ".*", "+?", "*?",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

/// Scans a mod (folder, `.pck` or `.dll`) and the given translation patches.
pub fn scan_leftover_text(
    mod_path: &Path,
    patch_paths: &[&Path],
    target_language: &str,
) -> AppResult<LeftoverReport> {
    let mut report = LeftoverReport::default();
    scan_path(mod_path, target_language, false, &mut report)?;
    for patch in patch_paths {
        scan_path(patch, target_language, true, &mut report)?;
    }
    report.files.sort_by(|left, right| {
        left.location
            .cmp(&right.location)
            .then_with(|| right.texts.len().cmp(&left.texts.len()))
            .then_with(|| left.file.cmp(&right.file))
    });
    Ok(report)
}

fn scan_path(
    path: &Path,
    target_language: &str,
    is_patch: bool,
    report: &mut LeftoverReport,
) -> AppResult<()> {
    if path.is_dir() {
        let mut files = Vec::new();
        collect_files(path, &mut files)?;
        files.sort();
        for file in files {
            let relative = file
                .strip_prefix(path)
                .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            scan_file(&file, &relative, target_language, is_patch, report)?;
        }
        return Ok(());
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    scan_file(path, &name, target_language, is_patch, report)
}

fn collect_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> AppResult<()> {
    for entry in fs::read_dir(dir).map_err(|source| AppError::io(dir, source))? {
        let entry = entry.map_err(|source| AppError::io(dir, source))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn scan_file(
    path: &Path,
    relative: &str,
    target_language: &str,
    is_patch: bool,
    report: &mut LeftoverReport,
) -> AppResult<()> {
    if has_extension(path, "pck") {
        return scan_pck(path, target_language, is_patch, report);
    }
    if !is_patch && is_hardcoded_source_file(path) {
        report.scanned_files += 1;
        let values = flatten_hardcoded_values(path)?;
        push_texts(
            report,
            LeftoverLocation::Dll,
            path.display().to_string(),
            values
                .into_values()
                .filter(|value| contains_foreign_script(value, target_language)),
        );
        return Ok(());
    }
    if !is_text_resource(relative, is_patch) {
        return Ok(());
    }
    let size = fs::metadata(path)
        .map_err(|source| AppError::io(path, source))?
        .len();
    if size > MAX_TEXT_RESOURCE_BYTES {
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|source| AppError::io(path, source))?;
    report.scanned_files += 1;
    push_texts(
        report,
        resource_location(is_patch),
        path.display().to_string(),
        foreign_text_runs(&bytes, target_language),
    );
    Ok(())
}

fn scan_pck(
    pck: &Path,
    target_language: &str,
    is_patch: bool,
    report: &mut LeftoverReport,
) -> AppResult<()> {
    let Some(files) = read_pck_files(pck, |resource| is_text_resource(resource, is_patch))? else {
        report.unreadable.push(pck.display().to_string());
        return Ok(());
    };
    for (resource, bytes) in files {
        if bytes.len() as u64 > MAX_TEXT_RESOURCE_BYTES {
            continue;
        }
        report.scanned_files += 1;
        push_texts(
            report,
            resource_location(is_patch),
            format!("{}::res://{resource}", pck.display()),
            foreign_text_runs(&bytes, target_language),
        );
    }
    Ok(())
}

fn resource_location(is_patch: bool) -> LeftoverLocation {
    if is_patch {
        LeftoverLocation::Patch
    } else {
        LeftoverLocation::Resource
    }
}

fn push_texts(
    report: &mut LeftoverReport,
    location: LeftoverLocation,
    file: String,
    texts: impl IntoIterator<Item = String>,
) {
    let mut seen = BTreeSet::new();
    let texts = texts
        .into_iter()
        .map(|text| truncate_snippet(text.trim()))
        .filter(|text| !text.is_empty() && !looks_like_file_path(text) && seen.insert(text.clone()))
        .map(|text| LeftoverText {
            code_like: is_code_like_text(&text),
            text,
        })
        .collect::<Vec<_>>();
    if !texts.is_empty() {
        report.files.push(LeftoverFile {
            location,
            file,
            texts,
        });
    }
}

/// Asset paths (`V3赛璐璐_20261001/starter_HeroOrb.png`) contain source
/// script but are never shown to the player.
fn looks_like_file_path(text: &str) -> bool {
    if text.starts_with("res://") || text.starts_with("user://") {
        return true;
    }
    if text.chars().any(char::is_whitespace) || !text.contains(['/', '\\']) {
        return false;
    }
    text.rsplit_once('.').is_some_and(|(_, extension)| {
        (1..=5).contains(&extension.len())
            && extension
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    })
}

fn truncate_snippet(text: &str) -> String {
    if text.chars().count() <= MAX_SNIPPET_CHARS {
        return text.to_string();
    }
    let mut snippet = text.chars().take(MAX_SNIPPET_CHARS).collect::<String>();
    snippet.push('…');
    snippet
}

/// Resources that can carry player-facing text. Localization tables of the
/// original mod are the translation sheet's job and are skipped; in a patch
/// they are exactly what should be checked.
fn is_text_resource(resource: &str, is_patch: bool) -> bool {
    let lower = resource.to_ascii_lowercase();
    if !is_patch && is_localization_path(&lower) {
        return false;
    }
    let extension = lower.rsplit_once('.').map(|(_, extension)| extension);
    matches!(
        extension,
        Some(
            "json"
                | "loc"
                | "txt"
                | "csv"
                | "tsv"
                | "md"
                | "cfg"
                | "ini"
                | "xml"
                | "yaml"
                | "yml"
                | "tres"
                | "tscn"
                | "res"
                | "scn"
                | "gd"
                | "gdc"
                | "translation"
        )
    )
}

fn is_localization_path(lower: &str) -> bool {
    lower.split('/').any(|part| {
        matches!(
            part,
            "localization" | "localisation" | "translations" | "i18n"
        )
    }) || lower.ends_with(".translation")
}

/// UTF-8 text runs from a (possibly binary) file that contain foreign script.
/// Runs are cut at line breaks and quotes so one JSON or scene file yields one
/// entry per string instead of one per file.
fn foreign_text_runs(bytes: &[u8], target_language: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut flush = |current: &mut String| {
        if contains_foreign_script(current, target_language) {
            runs.push(std::mem::take(current));
        } else {
            current.clear();
        }
    };
    let mut index = 0;
    while index < bytes.len() {
        let Some((character, width)) = decode_utf8(&bytes[index..]) else {
            flush(&mut current);
            index += 1;
            continue;
        };
        index += width;
        if matches!(character, '\n' | '\r' | '"') || (character.is_control() && character != '\t') {
            flush(&mut current);
        } else {
            current.push(character);
        }
    }
    flush(&mut current);
    runs
}

fn decode_utf8(bytes: &[u8]) -> Option<(char, usize)> {
    let first = *bytes.first()?;
    let width = match first {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return None,
    };
    let text = std::str::from_utf8(bytes.get(..width)?).ok()?;
    text.chars().next().map(|character| (character, width))
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_script_depends_on_target_language() {
        assert!(contains_foreign_script("进行2场精英战斗。", "kor"));
        assert!(!contains_foreign_script("엘리트 전투 2회", "kor"));
        assert!(!contains_foreign_script("进行2场精英战斗。", "zhs"));
        assert!(contains_foreign_script("ゲーム", "zhs"));
        assert!(!contains_foreign_script("Play 2 elite fights", "eng"));
        assert!(contains_foreign_script("엘리트", "eng"));
    }

    #[test]
    fn regex_strings_are_marked_code_like() {
        assert!(is_code_like_text("造成(\\d+)点伤害"));
        assert!(is_code_like_text("(?:并使)?欲望增加\\d+"));
        assert!(!is_code_like_text("[gold]消耗[/gold]"));
        assert!(!is_code_like_text("进行"));
    }

    #[test]
    fn text_runs_split_strings_inside_binary_and_json() {
        let mut bytes = b"\x00\x05\x00\x00".to_vec();
        bytes.extend("傲慢".as_bytes());
        bytes.extend(b"\x00\x00\x04text");
        bytes.extend("{\"a\": \"节制\", \"b\": \"temperance\"}\n".as_bytes());

        let runs = foreign_text_runs(&bytes, "kor");

        assert!(runs.iter().any(|run| run.trim() == "傲慢"));
        assert!(runs.iter().any(|run| run.trim() == "节制"));
        assert!(!runs.iter().any(|run| run.contains("temperance")));
    }

    #[test]
    fn asset_paths_are_not_reported() {
        assert!(looks_like_file_path(
            "V3赛璐璐_20261001/starter_HeroOrb.png"
        ));
        assert!(looks_like_file_path("res://Mod/图片/卡.png"));
        assert!(!looks_like_file_path("进行2场精英战斗。"));
        assert!(!looks_like_file_path("堕落值 +2"));
    }

    #[test]
    fn localization_tables_are_skipped_only_for_the_original_mod() {
        assert!(!is_text_resource("Mod/localization/zhs/cards.json", false));
        assert!(is_text_resource("Mod/localization/kor/cards.json", true));
        assert!(is_text_resource("Mod/scenes/trial.tscn", false));
        assert!(!is_text_resource("Mod/images/card.png", false));
    }

    #[test]
    fn scans_folder_mod_resources_and_patch_values() {
        let root = std::env::temp_dir().join(format!("sts2-leftover-text-{}", std::process::id()));
        let mod_dir = root.join("Mod");
        let patch_dir = root.join("Mod_tr");
        fs::create_dir_all(mod_dir.join("localization/zhs")).expect("create mod");
        fs::create_dir_all(mod_dir.join("data")).expect("create data");
        fs::create_dir_all(patch_dir.join("localization/kor")).expect("create patch");
        fs::write(
            mod_dir.join("localization/zhs/cards.json"),
            r#"{"a":"节制"}"#,
        )
        .expect("write loc");
        fs::write(mod_dir.join("data/trials.json"), r#"{"name":"傲慢"}"#).expect("write data");
        fs::write(
            patch_dir.join("localization/kor/cards.json"),
            r#"{"a":"절제","b":"跳过"}"#,
        )
        .expect("write patch");

        let report = scan_leftover_text(&mod_dir, &[patch_dir.as_path()], "kor").expect("scan");
        fs::remove_dir_all(&root).ok();

        let texts = |location| {
            report
                .files
                .iter()
                .filter(|file| file.location == location)
                .flat_map(|file| file.texts.iter().map(|text| text.text.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(LeftoverLocation::Resource), vec!["傲慢".to_string()]);
        assert_eq!(texts(LeftoverLocation::Patch), vec!["跳过".to_string()]);
    }
}
