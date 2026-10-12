//! The in-game companion mod ("bridge", source in `bridge/`) and the shared
//! folder it reads.
//!
//! The app writes translations to a fixed folder
//! (`%APPDATA%\STS2ModTranslationManager\shared`); the bridge reads them and
//! puts them into the game's loc tables at runtime, so applying a sheet no
//! longer means building a patch mod or rewriting the original mod's files.
//! See `docs/architecture/companion-mod.md`.

use crate::error::{AppError, AppResult};
use crate::json_translation::{JsonTranslationSheet, JsonTranslationStatus};
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

/// A sheet split into game loc tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetTables {
    /// Translated values (may be empty; empty ones are not written).
    pub values: TranslationTables,
    /// Source text per key, shown and searched in the in-game edit panel.
    pub sources: TranslationTables,
    pub unsupported: UnsupportedEntries,
}

/// Sheet values grouped by the game loc table they belong to. Only entries
/// from `localization/<lang>/<table>.json` with a top-level key qualify.
pub fn sheet_tables(sheet: &JsonTranslationSheet) -> SheetTables {
    let mut tables = SheetTables::default();
    let unsupported = &mut tables.unsupported;
    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed || entry.source_value.trim().is_empty() {
            continue;
        }
        match table_location(&entry.key) {
            TableLocation::Table { table, key } => {
                tables
                    .sources
                    .entry(table.clone())
                    .or_default()
                    .insert(key.clone(), entry.source_value.clone());
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
    Table { table: String, key: String },
    Hardcoded,
    Nested,
    Outside,
}

/// `(table, key)` of a sheet key that maps to a game loc table.
pub fn sheet_key_table(sheet_key: &str) -> Option<(String, String)> {
    match table_location(sheet_key) {
        TableLocation::Table { table, key } => Some((table, key)),
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
    let table_file = match parts.as_slice() {
        [.., localization, _lang, table_file]
            if localization.eq_ignore_ascii_case("localization") =>
        {
            *table_file
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
/// `_source/{table}.json` holds the source text of the written keys.
pub fn write_translations(
    shared: &Path,
    mod_id: &str,
    language: &str,
    tables: &TranslationTables,
    sources: &TranslationTables,
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
    for (table, values) in tables {
        let path = dir.join(format!("{table}.json"));
        let source_path = dir.join("_source").join(format!("{table}.json"));
        let values = values
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .collect::<BTreeMap<_, _>>();
        if values.is_empty() {
            remove_if_exists(&path)?;
            remove_if_exists(&source_path)?;
            continue;
        }
        let table_sources = sources
            .get(table)
            .map(|sources| {
                sources
                    .iter()
                    .filter(|(key, _)| values.contains_key(key))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let content = serde_json::to_string_pretty(&values)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        write_atomic(&path, &content)?;
        let content = serde_json::to_string_pretty(&table_sources)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        write_atomic(&source_path, &content)?;
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
            sources,
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
        assert_eq!(sources["cards"]["A.title"], "节制");
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
        let mut tables = TranslationTables::new();
        tables.entry("cards".to_string()).or_default().extend([
            ("A".to_string(), "절제".to_string()),
            ("B".to_string(), String::new()),
        ]);
        tables
            .entry("powers".to_string())
            .or_default()
            .insert("old".to_string(), String::new());

        let mut sources = TranslationTables::new();
        sources.entry("cards".to_string()).or_default().extend([
            ("A".to_string(), "节制".to_string()),
            ("B".to_string(), "空".to_string()),
        ]);

        let report = write_translations(
            &shared,
            "MaidenSuccubus",
            "kor",
            &tables,
            &sources,
            Some("0.1.0"),
            42,
        )
        .expect("write");
        let cards = fs::read_to_string(dir.join("cards.json")).expect("cards");
        let card_sources =
            fs::read_to_string(dir.join("_source").join("cards.json")).expect("sources");
        let meta = read_meta(&dir.join("_meta.json"));
        let format = fs::read_to_string(shared.join("format.json")).expect("format");
        let powers_exists = dir.join("powers.json").exists();
        fs::remove_dir_all(&shared).ok();

        assert_eq!(cards, "{\n  \"A\": \"절제\"\n}");
        assert_eq!(card_sources, "{\n  \"A\": \"节制\"\n}");
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
}
