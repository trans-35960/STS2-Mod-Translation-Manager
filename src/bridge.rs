//! The in-game companion mod ("bridge", source in `bridge/`) and the shared
//! folder it reads.
//!
//! The app writes translations to a fixed folder
//! (`%APPDATA%\STS2ModTranslationManager\shared`); the bridge reads them and
//! puts them into the game's loc tables at runtime, so applying a sheet no
//! longer means building a patch mod or rewriting the original mod's files.
//! See `docs/architecture/companion-mod.md`.

use crate::error::{AppError, AppResult};
use crate::json_translation::{
    JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus, translation_slot_entries,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const BRIDGE_MOD_ID: &str = "STS2ModManagerBridge";
/// Layout version of the shared folder; the bridge refuses formats it does not know.
pub const SHARED_FORMAT: u32 = 1;

/// `table -> key -> value`
pub type TranslationTables = BTreeMap<String, BTreeMap<String, String>>;

/// The shared folder. `STS2_BRIDGE_SHARED_DIR` overrides it for tests.
pub fn shared_root() -> Option<PathBuf> {
    if let Some(path) = env::var_os("STS2_BRIDGE_SHARED_DIR") {
        return Some(PathBuf::from(path));
    }
    env::var_os("APPDATA").map(|appdata| {
        PathBuf::from(appdata)
            .join("STS2ModTranslationManager")
            .join("shared")
    })
}

pub fn translations_dir(shared: &Path, mod_id: &str, language: &str) -> PathBuf {
    shared.join("translations").join(mod_id).join(language)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnsupportedEntries {
    /// DLL string literals (`dll://`); applied by a later bridge stage.
    pub hardcoded: usize,
    /// Files outside `localization/<lang>/` or not `.json`.
    pub outside_tables: usize,
    /// Values nested deeper than one level; loc tables are flat.
    pub nested: usize,
}

impl UnsupportedEntries {
    pub fn total(&self) -> usize {
        self.hardcoded + self.outside_tables + self.nested
    }
}

/// What the in-game edit panel shows next to a key besides its translation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryInfo {
    /// The id the translation tool shows (`k001-ab`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Language folder the source text comes from (`zhs`, `eng`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_lang: Option<String>,
    /// The mod's own text in its other languages (language folder → text).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub langs: BTreeMap<String, String>,
}

/// `table -> key -> info`
pub type InfoTables = BTreeMap<String, BTreeMap<String, EntryInfo>>;

/// A sheet split into game loc tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetTables {
    /// Translated values (may be empty; empty ones are not written).
    pub values: TranslationTables,
    /// Id and source per key; other languages are added by the caller.
    pub info: InfoTables,
    pub unsupported: UnsupportedEntries,
}

/// Sheet values grouped by the game loc table they belong to. Only entries
/// from `localization/<lang>/<table>.json` with a top-level key qualify.
pub fn sheet_tables(sheet: &JsonTranslationSheet) -> SheetTables {
    let mut tables = SheetTables::default();
    let ids = translation_slot_entries(sheet)
        .into_iter()
        .map(|slot| (slot.entry.key.as_str(), slot.id))
        .collect::<BTreeMap<_, _>>();
    let unsupported = &mut tables.unsupported;
    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed || entry.source_value.trim().is_empty() {
            continue;
        }
        match table_location(&entry.key) {
            TableLocation::Table {
                table,
                key,
                language,
            } => {
                tables.info.entry(table.clone()).or_default().insert(
                    key.clone(),
                    EntryInfo {
                        id: ids.get(entry.key.as_str()).cloned(),
                        source: Some(entry.source_value.clone()),
                        source_lang: Some(language),
                        langs: BTreeMap::new(),
                    },
                );
                tables
                    .values
                    .entry(table)
                    .or_default()
                    .insert(key, entry.translated_value.clone());
            }
            TableLocation::Hardcoded => unsupported.hardcoded += 1,
            TableLocation::Nested => unsupported.nested += 1,
            TableLocation::Outside => unsupported.outside_tables += 1,
        }
    }
    tables
}

enum TableLocation {
    Table {
        table: String,
        key: String,
        language: String,
    },
    Hardcoded,
    Nested,
    Outside,
}

/// `(table, key)` of a sheet key that maps to a game loc table.
pub fn sheet_key_table(sheet_key: &str) -> Option<(String, String)> {
    match table_location(sheet_key) {
        TableLocation::Table { table, key, .. } => Some((table, key)),
        _ => None,
    }
}

fn table_location(sheet_key: &str) -> TableLocation {
    if sheet_key.starts_with("dll://") {
        return TableLocation::Hardcoded;
    }
    let Some((file, pointer)) = sheet_key
        .strip_prefix("file://")
        .and_then(|rest| rest.split_once('#'))
    else {
        return TableLocation::Outside;
    };
    let parts = file.split('/').collect::<Vec<_>>();
    let (language, table_file) = match parts.as_slice() {
        [.., localization, language, table_file]
            if localization.eq_ignore_ascii_case("localization") =>
        {
            (*language, *table_file)
        }
        _ => return TableLocation::Outside,
    };
    let Some(table) = table_file
        .strip_suffix(".json")
        .or_else(|| table_file.strip_suffix(".JSON"))
    else {
        return TableLocation::Outside;
    };
    let Some(segment) = pointer.strip_prefix('/') else {
        return TableLocation::Outside;
    };
    if segment.contains('/') {
        return TableLocation::Nested;
    }
    TableLocation::Table {
        table: table.to_string(),
        key: segment.replace("~1", "/").replace("~0", "~"),
        language: language.to_string(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,
    #[serde(default)]
    pub updated_at: u64,
    /// Last in-game edit id the app has taken in (used from bridge stage B-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits_ack: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub tables: usize,
    /// Non-empty values written.
    pub entries: usize,
    pub dir: PathBuf,
}

/// Writes one sheet's tables for `mod_id`/`language`. Each table file holds
/// exactly the sheet's non-empty values, so values cleared in the sheet stop
/// being applied; tables of other sheets for the same mod are left alone.
/// `_info/{table}.json` holds id, source and other languages of the written
/// keys for the edit panel.
pub fn write_translations(
    shared: &Path,
    mod_id: &str,
    language: &str,
    tables: &SheetTables,
    target_version: Option<&str>,
    updated_at: u64,
) -> AppResult<WriteReport> {
    write_atomic(
        &shared.join("format.json"),
        &format!("{{\n  \"format\": {SHARED_FORMAT}\n}}"),
    )?;
    let dir = translations_dir(shared, mod_id, language);
    let mut report = WriteReport {
        dir: dir.clone(),
        ..WriteReport::default()
    };
    for (table, values) in &tables.values {
        let path = dir.join(format!("{table}.json"));
        let info_path = dir.join("_info").join(format!("{table}.json"));
        // Written by the first B-3 builds; `_info` replaces it.
        remove_if_exists(&dir.join("_source").join(format!("{table}.json")))?;
        let values = values
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .collect::<BTreeMap<_, _>>();
        if values.is_empty() {
            remove_if_exists(&path)?;
            remove_if_exists(&info_path)?;
            continue;
        }
        let table_info = tables
            .info
            .get(table)
            .map(|info| {
                info.iter()
                    .filter(|(key, _)| values.contains_key(key))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let content = serde_json::to_string_pretty(&values)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        write_atomic(&path, &content)?;
        let content = serde_json::to_string_pretty(&table_info)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        write_atomic(&info_path, &content)?;
        report.tables += 1;
        report.entries += values.len();
    }

    let meta_path = dir.join("_meta.json");
    let mut meta = read_meta(&meta_path);
    meta.target_version = target_version.map(str::to_string);
    meta.updated_at = updated_at;
    let content = serde_json::to_string_pretty(&meta)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    write_atomic(&meta_path, &content)?;
    Ok(report)
}

pub(crate) fn read_meta(path: &Path) -> TranslationMeta {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn remove_if_exists(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::io(path, error)),
    }
}

pub(crate) fn write_atomic(path: &Path, content: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, content).map_err(|source| AppError::io(&temp, source))?;
    fs::rename(&temp, path).map_err(|source| {
        let _ = fs::remove_file(&temp);
        AppError::io(path, source)
    })
}

/// One DLL string literal and what the bridge replaces it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DllString {
    pub original: String,
    pub translation: String,
    /// The id the translation tool shows, for the edit panel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// The DLL strings of a sheet the bridge can replace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetStrings {
    /// Translated strings first come with their translation; the rest of
    /// what the bridge may offer for in-game editing has an empty one.
    pub strings: Vec<DllString>,
    /// How many of `strings` are translated.
    pub translated: usize,
    /// Regexes, format patterns and similar code, never replaced: changing
    /// them breaks the mod rather than translating anything.
    pub code_like: usize,
}

/// The `dll://` entries of a DLL sheet for the bridge: translated ones, and
/// untranslated ones `is_text` keeps (with an empty translation) so the
/// in-game edit panel can translate them. Anything that looks like code is
/// left out.
pub fn sheet_strings(
    sheet: &JsonTranslationSheet,
    is_text: impl Fn(&JsonTranslationEntry) -> bool,
) -> SheetStrings {
    let ids = translation_slot_entries(sheet)
        .into_iter()
        .map(|slot| (slot.entry.key.as_str(), slot.id))
        .collect::<BTreeMap<_, _>>();
    let mut result = SheetStrings::default();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed || !entry.key.starts_with("dll://") {
            continue;
        }
        let translated =
            !entry.translated_value.is_empty() && entry.translated_value != entry.source_value;
        if !translated && !is_text(entry) {
            continue;
        }
        if crate::leftover_text::is_code_like_text(&entry.source_value) {
            if translated {
                result.code_like += 1;
            }
            continue;
        }
        // The #US heap stores each literal once, but a sheet rebuilt from an
        // updated DLL may list it again under another offset.
        if !seen.insert(entry.source_value.clone()) {
            continue;
        }
        if translated {
            result.translated += 1;
        }
        result.strings.push(DllString {
            original: entry.source_value.clone(),
            translation: if translated {
                entry.translated_value.clone()
            } else {
                String::new()
            },
            id: ids.get(entry.key.as_str()).cloned(),
        });
    }
    result
}

/// `strings/{modId}/{lang}/{assembly}.json`
pub fn strings_path(shared: &Path, mod_id: &str, language: &str, assembly: &str) -> PathBuf {
    shared
        .join("strings")
        .join(mod_id)
        .join(language)
        .join(format!("{assembly}.json"))
}

#[derive(Serialize)]
struct StringsFile<'a> {
    assembly: &'a str,
    updated_at: u64,
    strings: &'a [DllString],
}

/// Writes the strings the bridge replaces in `assembly`; an empty list
/// removes the file so nothing is replaced any more.
pub fn write_strings(
    shared: &Path,
    mod_id: &str,
    language: &str,
    assembly: &str,
    strings: &[DllString],
    updated_at: u64,
) -> AppResult<PathBuf> {
    write_atomic(
        &shared.join("format.json"),
        &format!("{{\n  \"format\": {SHARED_FORMAT}\n}}"),
    )?;
    let path = strings_path(shared, mod_id, language, assembly);
    if strings.is_empty() {
        remove_if_exists(&path)?;
        return Ok(path);
    }
    let content = serde_json::to_string_pretty(&StringsFile {
        assembly,
        updated_at,
        strings,
    })
    .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    write_atomic(&path, &content)?;
    Ok(path)
}

/// `status/bridge.json`, written by the bridge after each apply.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BridgeStatus {
    #[serde(default)]
    pub bridge_version: String,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub applied_at: String,
    /// Mod id → keys applied.
    #[serde(default)]
    pub mods: BTreeMap<String, usize>,
    #[serde(default)]
    pub invalid_count: usize,
    #[serde(default)]
    pub invalid: Vec<BridgeInvalidEntry>,
    #[serde(default)]
    pub missing_tables: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BridgeInvalidEntry {
    pub mod_id: String,
    pub table: String,
    pub key: String,
    pub error: String,
}

pub fn read_status(shared: &Path) -> Option<BridgeStatus> {
    let text = fs::read_to_string(shared.join("status").join("bridge.json")).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Translations the shared folder holds for one mod in one language.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredTranslation {
    pub language: String,
    /// Loc table values.
    pub entries: usize,
    /// Translated DLL strings.
    pub dll_strings: usize,
}

/// What the bridge will apply for `mod_id`, per language; languages with
/// nothing to apply are left out.
pub fn stored_translations(shared: &Path, mod_id: &str) -> Vec<StoredTranslation> {
    let mut by_language = BTreeMap::<String, StoredTranslation>::new();
    for (language, dir) in language_dirs(&shared.join("translations").join(mod_id)) {
        let entries = json_files(&dir)
            .filter_map(|path| read_json::<BTreeMap<String, serde_json::Value>>(&path))
            .map(|table| table.len())
            .sum();
        by_language.entry(language.clone()).or_default().entries = entries;
    }
    for (language, dir) in language_dirs(&shared.join("strings").join(mod_id)) {
        let dll_strings = json_files(&dir)
            .filter_map(|path| read_json::<StoredStringsFile>(&path))
            .map(|file| {
                file.strings
                    .iter()
                    .filter(|string| !string.translation.is_empty())
                    .count()
            })
            .sum();
        by_language.entry(language).or_default().dll_strings = dll_strings;
    }
    by_language
        .into_iter()
        .map(|(language, stored)| StoredTranslation { language, ..stored })
        .filter(|stored| stored.entries + stored.dll_strings > 0)
        .collect()
}

#[derive(Deserialize)]
struct StoredStringsFile {
    #[serde(default)]
    strings: Vec<DllString>,
}

fn language_dirs(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| Some((path.file_name()?.to_str()?.to_string(), path)))
}

/// Table and strings files of a language folder; `_meta.json` and the
/// `_info` folder are bookkeeping, not translations.
fn json_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('_'))
        })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Version from a bridge manifest (`STS2ModManagerBridge.json`).
pub fn manifest_version(manifest_path: &Path) -> Option<String> {
    let text = fs::read_to_string(manifest_path).ok()?;
    let value =
        serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Whether `candidate` is a newer dotted version than `installed`.
pub fn is_newer_version(candidate: &str, installed: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    let (mut candidate, mut installed) = (parts(candidate), parts(installed));
    let length = candidate.len().max(installed.len());
    candidate.resize(length, 0);
    installed.resize(length, 0);
    candidate > installed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_translation::JsonTranslationEntry;

    fn entry(key: &str, source: &str, translated: &str) -> JsonTranslationEntry {
        JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: source.to_string(),
            translated_value: translated.to_string(),
            status: JsonTranslationStatus::Ready,
            acknowledged_issues: Vec::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("sts2-bridge-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn sheet_keys_map_to_tables_and_skip_unsupported() {
        let sheet = JsonTranslationSheet {
            source_path: String::new(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: vec![
                entry(
                    "file://Mod/localization/zhs/cards.json#/A.title",
                    "节制",
                    "절제",
                ),
                entry("file://localization/zhs/powers.json#/a~1b", "跳过", ""),
                entry(
                    "file://Mod/localization/zhs/cards.json#/nested/inner",
                    "x",
                    "y",
                ),
                entry("file://Mod/data/trials.json#/name", "傲慢", "오만"),
                entry("dll://1a2b:4", "进行", "진행"),
            ],
        };

        let SheetTables {
            values: tables,
            info,
            unsupported,
        } = sheet_tables(&sheet);

        let mut expected = TranslationTables::new();
        expected
            .entry("cards".to_string())
            .or_default()
            .insert("A.title".to_string(), "절제".to_string());
        expected
            .entry("powers".to_string())
            .or_default()
            .insert("a/b".to_string(), String::new());
        assert_eq!(tables, expected);
        assert_eq!(info["cards"]["A.title"].source.as_deref(), Some("节制"));
        assert_eq!(info["cards"]["A.title"].source_lang.as_deref(), Some("zhs"));
        assert!(
            info["cards"]["A.title"]
                .id
                .as_deref()
                .is_some_and(|id| id.starts_with("k001-")),
            "the id the translation tool shows"
        );
        assert_eq!(
            unsupported,
            UnsupportedEntries {
                hardcoded: 1,
                outside_tables: 1,
                nested: 1
            }
        );
    }

    #[test]
    fn writes_only_non_empty_values_and_keeps_edit_ack() {
        let shared = temp_dir("write");
        let dir = translations_dir(&shared, "MaidenSuccubus", "kor");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("powers.json"), "{\"old\": \"x\"}").expect("old table");
        fs::write(dir.join("_meta.json"), "{\"edits_ack\": \"e-7\"}").expect("old meta");
        fs::create_dir_all(dir.join("_source")).expect("old source dir");
        fs::write(dir.join("_source").join("cards.json"), "{}").expect("old source");
        let mut tables = SheetTables::default();
        tables
            .values
            .entry("cards".to_string())
            .or_default()
            .extend([
                ("A".to_string(), "절제".to_string()),
                ("B".to_string(), String::new()),
            ]);
        tables
            .values
            .entry("powers".to_string())
            .or_default()
            .insert("old".to_string(), String::new());
        let info = |id: &str, source: &str| EntryInfo {
            id: Some(id.to_string()),
            source: Some(source.to_string()),
            source_lang: Some("zhs".to_string()),
            langs: BTreeMap::from([("eng".to_string(), "Temperance".to_string())]),
        };
        tables.info.entry("cards".to_string()).or_default().extend([
            ("A".to_string(), info("k001-aa", "节制")),
            ("B".to_string(), info("k002-bb", "空")),
        ]);

        let report =
            write_translations(&shared, "MaidenSuccubus", "kor", &tables, Some("0.1.0"), 42)
                .expect("write");
        let cards = fs::read_to_string(dir.join("cards.json")).expect("cards");
        let card_info: BTreeMap<String, EntryInfo> = serde_json::from_str(
            &fs::read_to_string(dir.join("_info").join("cards.json")).expect("info"),
        )
        .expect("info json");
        let old_source_exists = dir.join("_source").join("cards.json").exists();
        let meta = read_meta(&dir.join("_meta.json"));
        let format = fs::read_to_string(shared.join("format.json")).expect("format");
        let powers_exists = dir.join("powers.json").exists();
        fs::remove_dir_all(&shared).ok();

        assert_eq!(cards, "{\n  \"A\": \"절제\"\n}");
        assert_eq!(card_info.len(), 1, "only keys that were written");
        assert_eq!(card_info["A"].id.as_deref(), Some("k001-aa"));
        assert_eq!(card_info["A"].langs["eng"], "Temperance");
        assert!(!old_source_exists);
        assert!(
            !powers_exists,
            "a table emptied in the sheet stops being applied"
        );
        assert_eq!(report.tables, 1);
        assert_eq!(report.entries, 1);
        assert_eq!(meta.edits_ack.as_deref(), Some("e-7"));
        assert_eq!(meta.target_version.as_deref(), Some("0.1.0"));
        assert_eq!(meta.updated_at, 42);
        assert!(format.contains("\"format\": 1"));
    }

    #[test]
    fn dll_strings_skip_code_untranslated_and_repeats() {
        let sheet = JsonTranslationSheet {
            source_path: "Mod/Mod.dll".to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: vec![
                entry("dll://10:4", "傲慢", "오만"),
                entry("dll://20:6", "^(?:进行)\\d+$", "^(?:진행)\\d+$"),
                entry("dll://30:4", "跳过", ""),
                entry("dll://40:4", "same", "same"),
                entry("dll://50:4", "傲慢", "오만2"),
            ],
        };

        let result = sheet_strings(&sheet, |_| false);
        let with_text = sheet_strings(&sheet, |entry| entry.source_value == "跳过");

        assert_eq!(result.strings.len(), 1);
        assert_eq!(result.translated, 1);
        assert_eq!(with_text.strings.len(), 2);
        assert_eq!(with_text.strings[1].translation, "");
        assert_eq!(with_text.translated, 1);
        assert_eq!(result.strings[0].original, "傲慢");
        assert_eq!(result.strings[0].translation, "오만");
        assert!(result.strings[0].id.is_some());
        assert_eq!(result.code_like, 1);
    }

    #[test]
    fn writes_and_clears_dll_strings() {
        let shared = temp_dir("strings");
        let strings = vec![DllString {
            original: "傲慢".to_string(),
            translation: "오만".to_string(),
            id: Some("k001-aa".to_string()),
        }];

        let path = write_strings(&shared, "Mod", "kor", "Mod", &strings, 7).expect("write");
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        write_strings(&shared, "Mod", "kor", "Mod", &[], 8).expect("clear");
        let cleared = path.exists();
        fs::remove_dir_all(&shared).ok();

        assert!(path.ends_with("strings/Mod/kor/Mod.json"));
        assert_eq!(written["assembly"], "Mod");
        assert_eq!(written["strings"][0]["translation"], "오만");
        assert!(!cleared);
    }

    #[test]
    fn reads_bridge_status_written_by_the_mod() {
        let shared = temp_dir("status");
        fs::create_dir_all(shared.join("status")).expect("create");
        fs::write(
            shared.join("status").join("bridge.json"),
            r#"{"bridge_version":"0.2.2","game_version":"v0.111.0","language":"kor","applied_at":"2026-10-12T00:00:00+09:00","mods":{"MaidenSuccubus":1367},"invalid_count":1,"invalid":[{"mod_id":"M","table":"cards","key":"K","error":"bad"}],"missing_tables":[],"errors":[]}"#,
        )
        .expect("write status");

        let status = read_status(&shared).expect("status");
        fs::remove_dir_all(&shared).ok();

        assert_eq!(status.mods.get("MaidenSuccubus"), Some(&1367));
        assert_eq!(status.invalid[0].key, "K");
    }

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer_version("0.2.3", "0.2.2"));
        assert!(is_newer_version("1.0", "0.9.9"));
        assert!(!is_newer_version("0.2.2", "0.2.2"));
        assert!(!is_newer_version("0.2.2", "v0.2.10"));
    }

    #[test]
    fn stored_translations_count_what_the_bridge_applies() {
        let shared = temp_dir("stored");
        let mut tables = SheetTables::default();
        tables
            .values
            .entry("cards".to_string())
            .or_default()
            .extend([
                ("A".to_string(), "절제".to_string()),
                ("B".to_string(), "오만".to_string()),
            ]);
        write_translations(&shared, "MaidenSuccubus", "kor", &tables, None, 1).expect("tables");
        let string = |original: &str, translation: &str| DllString {
            original: original.to_string(),
            translation: translation.to_string(),
            id: None,
        };
        write_strings(
            &shared,
            "MaidenSuccubus",
            "kor",
            "MaidenSuccubus",
            &[string("进行", "진행"), string("等待", "")],
            1,
        )
        .expect("kor strings");
        // Only untranslated strings: offered for in-game editing, nothing applied.
        write_strings(
            &shared,
            "MaidenSuccubus",
            "jpn",
            "MaidenSuccubus",
            &[string("等待", "")],
            1,
        )
        .expect("jpn strings");

        let stored = stored_translations(&shared, "MaidenSuccubus");
        let other = stored_translations(&shared, "OtherMod");
        fs::remove_dir_all(&shared).ok();

        assert_eq!(
            stored,
            vec![StoredTranslation {
                language: "kor".to_string(),
                entries: 2,
                dll_strings: 1,
            }]
        );
        assert!(other.is_empty());
    }
}
