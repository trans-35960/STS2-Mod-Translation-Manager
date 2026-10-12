//! Saving translations into Sts2ModTranslator's `overrides` instead of a
//! separate patch mod.
//!
//! Sts2ModTranslator (ing-gom/sts2-mod-translator) injects
//! `Translations/overrides/{modId}/{lang}/{table}.txt` (flat JSON) on top of
//! every loaded table, and lets the player edit those files in game. This app
//! stays the source of truth: it writes its sheet values there, keeps the last
//! synced value per key as a base, and never overwrites a value the player
//! changed in game since then. See `docs/translation/translator-link.md`.

use crate::error::{AppError, AppResult};
use crate::json_translation::{JsonTranslationSheet, JsonTranslationStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const TRANSLATOR_MOD_ID: &str = "Sts2ModTranslator";
/// Oldest version whose storage layout (mod-folder root, `.txt` data files)
/// was checked against the Translator source.
pub const MIN_TRANSLATOR_VERSION: [u64; 3] = [1, 19, 0];
const DATA_EXT: &str = "txt";

/// `table -> key -> value`
pub type OverrideTables = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnsupportedEntries {
    /// DLL string literals (`dll://`); Translator only patches loc tables.
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

pub fn translations_root(translator_dir: &Path) -> PathBuf {
    translator_dir.join("Translations")
}

pub fn version_supported(version: &str) -> bool {
    let parts = version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .chain(std::iter::repeat(0))
        .take(3)
        .collect::<Vec<_>>();
    parts.as_slice() >= MIN_TRANSLATOR_VERSION.as_slice()
}

/// Sheet values Translator can carry, grouped by loc table. Only entries
/// from `localization/<lang>/<table>.json` with a top-level key qualify.
pub fn sheet_override_tables(sheet: &JsonTranslationSheet) -> (OverrideTables, UnsupportedEntries) {
    let mut tables = OverrideTables::new();
    let mut unsupported = UnsupportedEntries::default();
    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed || entry.source_value.trim().is_empty() {
            continue;
        }
        match override_location(&entry.key) {
            OverrideLocation::Table { table, key } => {
                tables
                    .entry(table)
                    .or_default()
                    .insert(key, entry.translated_value.clone());
            }
            OverrideLocation::Hardcoded => unsupported.hardcoded += 1,
            OverrideLocation::Nested => unsupported.nested += 1,
            OverrideLocation::Outside => unsupported.outside_tables += 1,
        }
    }
    (tables, unsupported)
}

enum OverrideLocation {
    Table { table: String, key: String },
    Hardcoded,
    Nested,
    Outside,
}

fn override_location(sheet_key: &str) -> OverrideLocation {
    if sheet_key.starts_with("dll://") {
        return OverrideLocation::Hardcoded;
    }
    let Some((file, pointer)) = sheet_key
        .strip_prefix("file://")
        .and_then(|rest| rest.split_once('#'))
    else {
        return OverrideLocation::Outside;
    };
    let parts = file.split('/').collect::<Vec<_>>();
    let table_file = match parts.as_slice() {
        [.., localization, _lang, table_file]
            if localization.eq_ignore_ascii_case("localization") =>
        {
            *table_file
        }
        _ => return OverrideLocation::Outside,
    };
    let Some(table) = table_file
        .strip_suffix(".json")
        .or_else(|| table_file.strip_suffix(".JSON"))
    else {
        return OverrideLocation::Outside;
    };
    let Some(segment) = pointer.strip_prefix('/') else {
        return OverrideLocation::Outside;
    };
    if segment.contains('/') {
        return OverrideLocation::Nested;
    }
    OverrideLocation::Table {
        table: table.to_string(),
        key: segment.replace("~1", "/").replace("~0", "~"),
    }
}

pub fn overrides_dir(root: &Path, mod_id: &str, language: &str) -> PathBuf {
    root.join("overrides").join(mod_id).join(language)
}

fn table_path(dir: &Path, table: &str) -> PathBuf {
    dir.join(format!("{table}.{DATA_EXT}"))
}

/// Reads one override table. `Ok(None)` when the file does not exist; an
/// error when it is not a flat string map, in which case it must not be
/// rewritten (Translator keeps broken files for the player to fix).
pub fn read_override_table(path: &Path) -> AppResult<Option<BTreeMap<String, String>>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(path, error)),
    };
    let text = text.trim_start_matches('\u{feff}');
    let value = serde_json::from_str::<serde_json::Value>(text)
        .or_else(|_| serde_json::from_str::<serde_json::Value>(&strip_lenient_json(text)))
        .map_err(|error| {
            AppError::InvalidCommand(format!(
                "Translator 번역 파일을 읽지 못했습니다: {} ({error})",
                path.display()
            ))
        })?;
    let serde_json::Value::Object(map) = value else {
        return Err(not_flat(path));
    };
    map.into_iter()
        .map(|(key, value)| match value {
            serde_json::Value::String(text) => Ok((key, text)),
            _ => Err(not_flat(path)),
        })
        .collect::<AppResult<BTreeMap<_, _>>>()
        .map(Some)
}

fn not_flat(path: &Path) -> AppError {
    AppError::InvalidCommand(format!(
        "Translator 번역 파일 형식이 예상과 다릅니다(키-문자열 맵이 아님): {}",
        path.display()
    ))
}

/// Translator's reader accepts `//` and `/* */` comments and trailing commas.
fn strip_lenient_json(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(character) = chars.next() {
        if in_string {
            output.push(character);
            if character == '\\' {
                if let Some(escaped) = chars.next() {
                    output.push(escaped);
                }
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                output.push(character);
            }
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            ',' => {
                let rest = chars.clone().find(|next| !next.is_whitespace());
                if !matches!(rest, Some('}' | ']')) {
                    output.push(character);
                }
            }
            _ => output.push(character),
        }
    }
    output
}

/// Same layout Translator (.NET on Windows) writes: ordinal key order,
/// two-space indent, non-ASCII kept as is, CRLF line breaks and no final
/// newline, so files diff cleanly against its own output. An existing file
/// keeps the line breaks it already uses.
fn format_override_table(table: &BTreeMap<String, String>, crlf: bool) -> String {
    let output = serde_json::to_string_pretty(table).unwrap_or_else(|_| "{}".to_string());
    if crlf {
        output.replace('\n', "\r\n")
    } else {
        output
    }
}

/// New files and files without line breaks get Translator's CRLF.
fn uses_crlf(path: &Path) -> bool {
    fs::read(path)
        .map(|bytes| !bytes.contains(&b'\n') || bytes.windows(2).any(|pair| pair == b"\r\n"))
        .unwrap_or(true)
}

fn write_atomic(path: &Path, content: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let temp = path.with_extension(format!("{DATA_EXT}.tmp"));
    fs::write(&temp, content).map_err(|source| AppError::io(&temp, source))?;
    fs::rename(&temp, path).map_err(|source| {
        let _ = fs::remove_file(&temp);
        AppError::io(path, source)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncConflict {
    pub table: String,
    pub key: String,
    pub ours: String,
    pub theirs: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncPlan {
    /// Values to write into overrides.
    pub writes: OverrideTables,
    /// Values the player changed in game since the last sync; left in place
    /// for a later import into the sheet.
    pub kept_in_game: OverrideTables,
    pub conflicts: Vec<SyncConflict>,
    pub unchanged: usize,
}

/// Per-key three-way decision between the sheet (`ours`), the override files
/// (`theirs`, `None` when the key or file is missing) and the value written
/// at the last sync (`base`).
pub fn plan_sync(ours: &OverrideTables, theirs: &OverrideTables, base: &OverrideTables) -> SyncPlan {
    let mut plan = SyncPlan::default();
    for (table, keys) in ours {
        for (key, ours_value) in keys {
            let theirs_value = theirs.get(table).and_then(|values| values.get(key));
            let base_value = base.get(table).and_then(|values| values.get(key));
            match decide(ours_value, theirs_value, base_value) {
                Decision::Unchanged => plan.unchanged += 1,
                Decision::Write => {
                    plan.writes
                        .entry(table.clone())
                        .or_default()
                        .insert(key.clone(), ours_value.clone());
                }
                Decision::KeepInGame => {
                    plan.kept_in_game.entry(table.clone()).or_default().insert(
                        key.clone(),
                        theirs_value.cloned().unwrap_or_default(),
                    );
                }
                Decision::Conflict => plan.conflicts.push(SyncConflict {
                    table: table.clone(),
                    key: key.clone(),
                    ours: ours_value.clone(),
                    theirs: theirs_value.cloned().unwrap_or_default(),
                }),
            }
        }
    }
    plan
}

enum Decision {
    Unchanged,
    Write,
    KeepInGame,
    Conflict,
}

fn decide(ours: &str, theirs: Option<&String>, base: Option<&String>) -> Decision {
    let Some(theirs) = theirs else {
        // Missing key or file (never written, or wiped by a Workshop update).
        return if ours.is_empty() {
            Decision::Unchanged
        } else {
            Decision::Write
        };
    };
    if ours == theirs {
        return Decision::Unchanged;
    }
    match base {
        Some(base) => match (ours == base, theirs == base) {
            (true, _) => Decision::KeepInGame,
            (false, true) => Decision::Write,
            (false, false) => Decision::Conflict,
        },
        // First sync: an empty template slot takes our value, an empty sheet
        // value leaves the player's, two different translations conflict.
        None if theirs.is_empty() => Decision::Write,
        None if ours.is_empty() => Decision::KeepInGame,
        None => Decision::Conflict,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written_entries: usize,
    pub written_tables: Vec<String>,
    /// Keys dropped from `baseline/` so Translator stops flagging values we
    /// just retranslated as "source changed".
    pub baseline_keys_cleared: usize,
}

/// Writes `plan.writes` into the override files, keeping every key this app
/// does not manage. Files are re-read right before writing so a concurrent
/// in-game save is not lost.
pub fn apply_writes(
    root: &Path,
    mod_id: &str,
    language: &str,
    writes: &OverrideTables,
) -> AppResult<WriteReport> {
    let dir = overrides_dir(root, mod_id, language);
    let mut report = WriteReport::default();
    for (table, values) in writes {
        if values.is_empty() {
            continue;
        }
        let path = table_path(&dir, table);
        let mut current = read_override_table(&path)?.unwrap_or_default();
        for (key, value) in values {
            current.insert(key.clone(), value.clone());
        }
        write_atomic(&path, &format_override_table(&current, uses_crlf(&path)))?;
        report.written_entries += values.len();
        report.written_tables.push(table.clone());
        report.baseline_keys_cleared += clear_baseline_keys(root, mod_id, language, table, values)?;
    }
    Ok(report)
}

fn clear_baseline_keys(
    root: &Path,
    mod_id: &str,
    language: &str,
    table: &str,
    keys: &BTreeMap<String, String>,
) -> AppResult<usize> {
    let path = table_path(&root.join("baseline").join(mod_id).join(language), table);
    // Never create a baseline and never touch one we cannot parse.
    let Ok(Some(mut baseline)) = read_override_table(&path) else {
        return Ok(0);
    };
    let before = baseline.len();
    baseline.retain(|key, _| !keys.contains_key(key));
    let cleared = before - baseline.len();
    if cleared > 0 {
        write_atomic(&path, &format_override_table(&baseline, uses_crlf(&path)))?;
    }
    Ok(cleared)
}

/// Records the target mod version the translation is based on, so Translator
/// does not report the overrides as outdated.
pub fn record_target_version(root: &Path, mod_id: &str, version: &str) -> AppResult<()> {
    let version = version.trim();
    if version.is_empty() {
        return Ok(());
    }
    let path = root.join(format!("target_versions.{DATA_EXT}"));
    let mut versions = read_override_table(&path)?.unwrap_or_default();
    if versions.get(mod_id).map(String::as_str) == Some(version) {
        return Ok(());
    }
    versions.insert(mod_id.to_string(), version.to_string());
    write_atomic(&path, &format_override_table(&versions, uses_crlf(&path)))
}

/// Reads every table `ours` mentions from the override folder.
pub fn read_override_tables(
    root: &Path,
    mod_id: &str,
    language: &str,
    tables: impl IntoIterator<Item = String>,
) -> AppResult<OverrideTables> {
    let dir = overrides_dir(root, mod_id, language);
    let mut output = OverrideTables::new();
    for table in tables {
        if let Some(values) = read_override_table(&table_path(&dir, &table))? {
            output.insert(table, values);
        }
    }
    Ok(output)
}

/// What this app last synced for one mod and language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBase {
    pub mod_id: String,
    pub translator_root: String,
    #[serde(default)]
    pub patch_mods_disabled: bool,
    pub tables: OverrideTables,
}

pub fn base_path(state_dir: &Path, mod_key: &str, language: &str) -> PathBuf {
    state_dir
        .join("translator_link")
        .join(sanitize_file_name(mod_key))
        .join(format!("{}.json", sanitize_file_name(language)))
}

pub fn read_base(path: &Path) -> AppResult<Option<SyncBase>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            AppError::InvalidCommand(format!(
                "Translator 동기화 기록을 읽지 못했습니다: {} ({error})",
                path.display()
            ))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::io(path, error)),
    }
}

pub fn write_base(path: &Path, base: &SyncBase) -> AppResult<()> {
    let content = serde_json::to_string_pretty(base)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    fs::write(path, content).map_err(|source| AppError::io(path, source))
}

/// Base after a sync: written and agreeing values become the new base;
/// values kept in game or in conflict keep their old base so the next sync
/// still sees them as changed.
pub fn next_base(ours: &OverrideTables, previous: &OverrideTables, plan: &SyncPlan) -> OverrideTables {
    let mut base = previous.clone();
    for (table, keys) in ours {
        for (key, value) in keys {
            let pending = plan
                .kept_in_game
                .get(table)
                .is_some_and(|values| values.contains_key(key))
                || plan
                    .conflicts
                    .iter()
                    .any(|conflict| &conflict.table == table && &conflict.key == key);
            if !pending {
                base.entry(table.clone())
                    .or_default()
                    .insert(key.clone(), value.clone());
            }
        }
    }
    base
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveTargetMode {
    Auto,
    Patch,
    Translator,
}

fn settings_path(state_dir: &Path) -> PathBuf {
    state_dir.join("translator_link").join("settings.json")
}

/// Per-mod forced save target; mods without an entry use `Auto`.
pub fn read_save_target_mode(state_dir: &Path, mod_key: &str) -> SaveTargetMode {
    fs::read_to_string(settings_path(state_dir))
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeMap<String, SaveTargetMode>>(&text).ok())
        .and_then(|modes| modes.get(mod_key).copied())
        .unwrap_or(SaveTargetMode::Auto)
}

pub fn write_save_target_mode(
    state_dir: &Path,
    mod_key: &str,
    mode: SaveTargetMode,
) -> AppResult<()> {
    let path = settings_path(state_dir);
    let mut modes = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeMap<String, SaveTargetMode>>(&text).ok())
        .unwrap_or_default();
    if mode == SaveTargetMode::Auto {
        modes.remove(mod_key);
    } else {
        modes.insert(mod_key.to_string(), mode);
    }
    let content = serde_json::to_string_pretty(&modes)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    fs::write(&path, content).map_err(|source| AppError::io(&path, source))
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_translation::JsonTranslationEntry;

    fn tables(entries: &[(&str, &str, &str)]) -> OverrideTables {
        let mut output = OverrideTables::new();
        for (table, key, value) in entries {
            output
                .entry(table.to_string())
                .or_default()
                .insert(key.to_string(), value.to_string());
        }
        output
    }

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

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "sts2-translator-link-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn version_guard_requires_checked_layout() {
        assert!(version_supported("1.20.0"));
        assert!(version_supported("v1.19.0"));
        assert!(version_supported("2.0"));
        assert!(!version_supported("1.18.9"));
        assert!(!version_supported(""));
    }

    #[test]
    fn sheet_keys_map_to_tables_and_skip_unsupported() {
        let sheet = JsonTranslationSheet {
            source_path: String::new(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: vec![
                entry("file://Mod/localization/zhs/cards.json#/A.title", "节制", "절제"),
                entry("file://localization/zhs/powers.json#/a~1b", "跳过", ""),
                entry("file://Mod/localization/zhs/cards.json#/nested/inner", "x", "y"),
                entry("file://Mod/data/trials.json#/name", "傲慢", "오만"),
                entry("dll://1a2b:4", "进行", "진행"),
            ],
        };

        let (converted, unsupported) = sheet_override_tables(&sheet);

        assert_eq!(
            converted,
            tables(&[("cards", "A.title", "절제"), ("powers", "a/b", "")])
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
    fn plan_follows_three_way_rules() {
        let ours = tables(&[
            ("t", "same", "A"),
            ("t", "ours_changed", "new"),
            ("t", "game_changed", "old"),
            ("t", "both_changed", "mine"),
            ("t", "both_same", "x"),
            ("t", "missing", "fill"),
            ("t", "first_empty_slot", "v"),
            ("t", "first_conflict", "mine"),
            ("t", "first_sheet_empty", ""),
        ]);
        let theirs = tables(&[
            ("t", "same", "A"),
            ("t", "ours_changed", "old"),
            ("t", "game_changed", "edited"),
            ("t", "both_changed", "theirs"),
            ("t", "both_same", "x"),
            ("t", "first_empty_slot", ""),
            ("t", "first_conflict", "theirs"),
            ("t", "first_sheet_empty", "game"),
        ]);
        let base = tables(&[
            ("t", "same", "A"),
            ("t", "ours_changed", "old"),
            ("t", "game_changed", "old"),
            ("t", "both_changed", "old"),
            ("t", "both_same", "old"),
            ("t", "missing", "fill"),
        ]);

        let plan = plan_sync(&ours, &theirs, &base);

        assert_eq!(
            plan.writes,
            tables(&[
                ("t", "ours_changed", "new"),
                ("t", "missing", "fill"),
                ("t", "first_empty_slot", "v"),
            ])
        );
        assert_eq!(
            plan.kept_in_game,
            tables(&[("t", "game_changed", "edited"), ("t", "first_sheet_empty", "game")])
        );
        let conflicts = plan
            .conflicts
            .iter()
            .map(|conflict| conflict.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(conflicts, vec!["both_changed", "first_conflict"]);
        assert_eq!(plan.unchanged, 2);
    }

    #[test]
    fn wiped_override_folder_is_rewritten_from_the_sheet() {
        let ours = tables(&[("t", "a", "번역"), ("t", "b", "")]);
        let base = ours.clone();

        let plan = plan_sync(&ours, &OverrideTables::new(), &base);

        assert_eq!(plan.writes, tables(&[("t", "a", "번역")]));
        assert_eq!(plan.unchanged, 1);
    }

    #[test]
    fn next_base_keeps_old_value_for_pending_keys() {
        let ours = tables(&[("t", "w", "new"), ("t", "g", "old"), ("t", "c", "mine")]);
        let previous = tables(&[("t", "w", "old"), ("t", "g", "old"), ("t", "c", "old")]);
        let theirs = tables(&[("t", "w", "old"), ("t", "g", "edited"), ("t", "c", "theirs")]);
        let plan = plan_sync(&ours, &theirs, &previous);

        let base = next_base(&ours, &previous, &plan);

        assert_eq!(
            base,
            tables(&[("t", "w", "new"), ("t", "g", "old"), ("t", "c", "old")])
        );
    }

    #[test]
    fn writes_keep_unknown_keys_and_translator_format() {
        let root = temp_root("write");
        let dir = overrides_dir(&root, "MaidenSuccubus", "kor");
        fs::create_dir_all(&dir).expect("create overrides");
        fs::write(
            dir.join("cards.txt"),
            "{\n  // template\n  \"B\": \"\",\n  \"A\": \"\",\n}\n",
        )
        .expect("write template");
        let baseline_dir = root.join("baseline/MaidenSuccubus/kor");
        fs::create_dir_all(&baseline_dir).expect("create baseline");
        fs::write(baseline_dir.join("cards.txt"), "{\"A\": \"旧\", \"C\": \"留\"}")
            .expect("write baseline");

        let report = apply_writes(
            &root,
            "MaidenSuccubus",
            "kor",
            &tables(&[("cards", "A", "절제"), ("relics", "R", "유물")]),
        )
        .expect("apply");
        record_target_version(&root, "MaidenSuccubus", "0.1.0").expect("version");

        let cards = fs::read_to_string(dir.join("cards.txt")).expect("read cards");
        let relics = fs::read_to_string(dir.join("relics.txt")).expect("read relics");
        let baseline = read_override_table(&baseline_dir.join("cards.txt"))
            .expect("read baseline")
            .expect("baseline exists");
        let versions = read_override_table(&root.join("target_versions.txt"))
            .expect("read versions")
            .expect("versions exist");
        let relic_baseline_created = root.join("baseline/MaidenSuccubus/kor/relics.txt").exists();
        fs::remove_dir_all(&root).ok();

        // The LF template keeps LF; the new table gets Translator's CRLF.
        assert_eq!(cards, "{\n  \"A\": \"절제\",\n  \"B\": \"\"\n}");
        assert_eq!(relics, "{\r\n  \"R\": \"유물\"\r\n}");
        assert_eq!(report.written_entries, 2);
        assert_eq!(report.baseline_keys_cleared, 1);
        assert_eq!(baseline.keys().collect::<Vec<_>>(), vec!["C"]);
        assert!(!relic_baseline_created);
        assert_eq!(versions.get("MaidenSuccubus").map(String::as_str), Some("0.1.0"));
    }

    #[test]
    fn broken_override_files_are_never_rewritten() {
        let root = temp_root("broken");
        let dir = overrides_dir(&root, "Mod", "kor");
        fs::create_dir_all(&dir).expect("create overrides");
        fs::write(dir.join("cards.txt"), "{\"A\": \"절제\"").expect("write broken");

        let result = apply_writes(&root, "Mod", "kor", &tables(&[("cards", "A", "x")]));
        let content = fs::read_to_string(dir.join("cards.txt")).expect("read");
        fs::remove_dir_all(&root).ok();

        assert!(result.is_err());
        assert_eq!(content, "{\"A\": \"절제\"");
    }

    #[test]
    fn save_target_mode_defaults_to_auto() {
        let root = temp_root("mode");
        assert_eq!(read_save_target_mode(&root, "mod-a"), SaveTargetMode::Auto);
        write_save_target_mode(&root, "mod-a", SaveTargetMode::Patch).expect("write");
        assert_eq!(read_save_target_mode(&root, "mod-a"), SaveTargetMode::Patch);
        write_save_target_mode(&root, "mod-a", SaveTargetMode::Auto).expect("reset");
        assert_eq!(read_save_target_mode(&root, "mod-a"), SaveTargetMode::Auto);
        fs::remove_dir_all(&root).ok();
    }
}
