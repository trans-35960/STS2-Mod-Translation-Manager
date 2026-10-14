//! In-game edits and mistranslation reports the bridge writes to
//! `edits/{modId}/{lang}.jsonl`, and taking them into a sheet.
//!
//! The bridge shows an edit only while the app's value for the key is still
//! the one the edit replaced (`before`), so the app never has to remove an
//! edit: changing the key in the sheet and applying wins on its own. Taking
//! edits in means copying the chosen values into the sheet and moving
//! `edits_ack` past them. Reports the user has not dealt with yet are kept in
//! the app's state so acking does not lose them.
//!
//! Loc table edits and DLL string edits (`{lang}.dll.jsonl`, table
//! `dll:{assembly}`, key = the literal) are separate logs with separate acks:
//! a mod has one sheet for each, and taking in one must not ack the other.

use crate::bridge::{read_meta, sheet_key_table, translations_dir, write_atomic};
use crate::error::{AppError, AppResult};
use crate::json_translation::{
    JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus, is_hardcoded_entry_key,
    is_hardcoded_source_file,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const KIND_EDIT: &str = "edit";
pub const KIND_REVERT: &str = "revert";
pub const KIND_REPORT: &str = "report";

/// One line of the edit log (written by the bridge's `EditLog`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRecord {
    pub id: String,
    pub kind: String,
    pub table: String,
    pub key: String,
    #[serde(default)]
    pub source: Option<String>,
    /// For edits: the app's value when the edit was made. For reports: the
    /// text shown in game.
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub note: String,
}

/// Which edit log a sheet takes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditChannel {
    /// Loc table keys (`translations/`).
    Loc,
    /// DLL string literals (`strings/`).
    Dll,
}

impl EditChannel {
    pub fn of_sheet(sheet: &JsonTranslationSheet) -> Self {
        if is_hardcoded_source_file(Path::new(&sheet.source_path)) {
            Self::Dll
        } else {
            Self::Loc
        }
    }

    fn log_name(self, language: &str) -> String {
        match self {
            Self::Loc => format!("{language}.jsonl"),
            Self::Dll => format!("{language}.dll.jsonl"),
        }
    }

    /// Where `_meta.json` (and its `edits_ack`) lives.
    fn meta_path(self, shared: &Path, mod_id: &str, language: &str) -> PathBuf {
        match self {
            Self::Loc => translations_dir(shared, mod_id, language),
            Self::Dll => shared.join("strings").join(mod_id).join(language),
        }
        .join("_meta.json")
    }
}

/// The `(table, key)` an entry has in the edit log: its loc table and key,
/// or `dll:{assembly}` and the literal for a DLL sheet.
pub fn sheet_slot(
    sheet: &JsonTranslationSheet,
    entry: &JsonTranslationEntry,
) -> Option<(String, String)> {
    if is_hardcoded_entry_key(&entry.key) {
        let assembly = Path::new(&sheet.source_path).file_stem()?.to_string_lossy();
        return Some((format!("dll:{assembly}"), entry.source_value.clone()));
    }
    sheet_key_table(&entry.key)
}

pub fn edits_path(shared: &Path, mod_id: &str, language: &str, channel: EditChannel) -> PathBuf {
    shared
        .join("edits")
        .join(mod_id)
        .join(channel.log_name(language))
}

/// Records in file order; lines that do not parse (a write cut off by a
/// crash) are skipped.
pub fn read_edits(
    shared: &Path,
    mod_id: &str,
    language: &str,
    channel: EditChannel,
) -> Vec<EditRecord> {
    let Ok(text) = fs::read_to_string(edits_path(shared, mod_id, language, channel)) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<EditRecord>(line.trim_start_matches('\u{feff}')).ok()
        })
        .filter(|record| !record.id.is_empty())
        .collect()
}

pub fn read_edits_ack(
    shared: &Path,
    mod_id: &str,
    language: &str,
    channel: EditChannel,
) -> Option<String> {
    read_meta(&channel.meta_path(shared, mod_id, language)).edits_ack
}

/// Moves `edits_ack` to `id`; never backwards.
pub fn write_edits_ack(
    shared: &Path,
    mod_id: &str,
    language: &str,
    channel: EditChannel,
    id: &str,
) -> AppResult<()> {
    let path = channel.meta_path(shared, mod_id, language);
    let mut meta = read_meta(&path);
    if meta.edits_ack.as_deref().is_some_and(|ack| ack >= id) {
        return Ok(());
    }
    meta.edits_ack = Some(id.to_string());
    let content = serde_json::to_string_pretty(&meta)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    write_atomic(&path, &content)
}

/// What the bridge has written since the app last took edits in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingEdits {
    /// Latest edit per `(table, key)`; a revert removes the key.
    pub edits: BTreeMap<(String, String), EditRecord>,
    pub reports: Vec<EditRecord>,
    /// Highest id after the ack, to ack once these are dealt with.
    pub last_id: Option<String>,
}

pub fn pending_edits(records: &[EditRecord], ack: Option<&str>) -> PendingEdits {
    let mut pending = PendingEdits::default();
    for record in records {
        if ack.is_some_and(|ack| record.id.as_str() <= ack) {
            continue;
        }
        if pending
            .last_id
            .as_deref()
            .is_none_or(|last| record.id.as_str() > last)
        {
            pending.last_id = Some(record.id.clone());
        }
        let slot = (record.table.clone(), record.key.clone());
        match record.kind.as_str() {
            KIND_EDIT if record.after.is_some() => {
                pending.edits.insert(slot, record.clone());
            }
            KIND_REVERT => {
                pending.edits.remove(&slot);
            }
            KIND_REPORT => pending.reports.push(record.clone()),
            _ => {}
        }
    }
    pending
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditImportKind {
    /// Only the game changed the key: the sheet still has `before`.
    GameChanged,
    /// The sheet changed the key after the edit was made too.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditImportItem {
    pub kind: EditImportKind,
    pub sheet_key: String,
    pub table: String,
    pub key: String,
    pub source: Option<String>,
    pub sheet_value: String,
    pub game_value: String,
}

/// Pending edits that differ from the sheet. Edits whose key is no longer in
/// the sheet, or that the sheet already matches, need no choice.
pub fn edit_import_items(
    sheet: &JsonTranslationSheet,
    pending: &PendingEdits,
) -> Vec<EditImportItem> {
    let mut items = Vec::new();
    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed {
            continue;
        }
        let Some(slot) = sheet_slot(sheet, entry) else {
            continue;
        };
        let Some(edit) = pending.edits.get(&slot) else {
            continue;
        };
        let game_value = edit.after.clone().unwrap_or_default();
        if entry.translated_value == game_value {
            continue;
        }
        items.push(EditImportItem {
            kind: if Some(&entry.translated_value) == edit.before.as_ref() {
                EditImportKind::GameChanged
            } else {
                EditImportKind::Conflict
            },
            sheet_key: entry.key.clone(),
            table: slot.0,
            key: slot.1,
            source: edit
                .source
                .clone()
                .or_else(|| Some(entry.source_value.clone())),
            sheet_value: entry.translated_value.clone(),
            game_value,
        });
    }
    items
}

/// A mistranslation report the app has taken in. Handled reports stay
/// (marked `resolved`) until acked, so the edit log cannot bring them back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredReport {
    pub mod_id: String,
    pub language: String,
    /// From the DLL string log, whose ids are acked separately.
    #[serde(default)]
    pub dll: bool,
    #[serde(default)]
    pub resolved: bool,
    #[serde(flatten)]
    pub record: EditRecord,
}

fn reports_path(state_dir: &Path) -> PathBuf {
    state_dir.join("bridge").join("reports.json")
}

pub fn read_stored_reports(state_dir: &Path) -> Vec<StoredReport> {
    fs::read_to_string(reports_path(state_dir))
        .ok()
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

fn write_stored_reports(state_dir: &Path, reports: &[StoredReport]) -> AppResult<()> {
    let content = serde_json::to_string_pretty(reports)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    write_atomic(&reports_path(state_dir), &content)
}

/// Keeps the pending reports (by id, so taking them in twice is harmless)
/// and then acks everything up to `ack_id`.
pub fn acknowledge(
    shared: &Path,
    state_dir: &Path,
    mod_id: &str,
    language: &str,
    channel: EditChannel,
    ack_id: &str,
) -> AppResult<()> {
    let ack = read_edits_ack(shared, mod_id, language, channel);
    let pending = pending_edits(
        &read_edits(shared, mod_id, language, channel),
        ack.as_deref(),
    );
    let mut stored = read_stored_reports(state_dir);
    for report in pending
        .reports
        .into_iter()
        .filter(|report| report.id.as_str() <= ack_id)
    {
        if !stored.iter().any(|item| item.record.id == report.id) {
            stored.push(StoredReport {
                mod_id: mod_id.to_string(),
                language: language.to_string(),
                dll: channel == EditChannel::Dll,
                resolved: false,
                record: report,
            });
        }
    }
    // Acked reports never come back from the log, so handled ones can go.
    stored.retain(|item| {
        !(item.resolved
            && item.mod_id == mod_id
            && item.language == language
            && item.dll == (channel == EditChannel::Dll)
            && item.record.id.as_str() <= ack_id)
    });
    write_stored_reports(state_dir, &stored)?;
    write_edits_ack(shared, mod_id, language, channel, ack_id)
}

/// In-game edits and open reports for `mod_id` in every language, for the
/// mod list. Edits are counted per key without comparing to the sheet.
pub fn pending_counts(shared: &Path, state_dir: &Path, mod_id: &str) -> (usize, usize) {
    let stored = read_stored_reports(state_dir);
    let mut edits = 0;
    let mut reports = stored
        .iter()
        .filter(|item| item.mod_id == mod_id && !item.resolved)
        .count();
    let Ok(files) = fs::read_dir(shared.join("edits").join(mod_id)) else {
        return (edits, reports);
    };
    for file in files.flatten() {
        let path = file.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let (language, channel) = match stem.strip_suffix(".dll") {
            Some(language) => (language, EditChannel::Dll),
            None => (stem, EditChannel::Loc),
        };
        let ack = read_edits_ack(shared, mod_id, language, channel);
        let pending = pending_edits(
            &read_edits(shared, mod_id, language, channel),
            ack.as_deref(),
        );
        edits += pending.edits.len();
        reports += pending
            .reports
            .iter()
            .filter(|report| !stored.iter().any(|item| item.record.id == report.id))
            .count();
    }
    (edits, reports)
}

/// Marks a report handled, whether the app took it in already or it is
/// still only in the edit log. Does not move the ack.
pub fn resolve_report(
    shared: &Path,
    state_dir: &Path,
    mod_id: &str,
    language: &str,
    channel: EditChannel,
    id: &str,
) -> AppResult<()> {
    let mut stored = read_stored_reports(state_dir);
    if let Some(item) = stored.iter_mut().find(|item| item.record.id == id) {
        item.resolved = true;
    } else if let Some(record) = read_edits(shared, mod_id, language, channel)
        .into_iter()
        .find(|record| record.id == id && record.kind == KIND_REPORT)
    {
        stored.push(StoredReport {
            mod_id: mod_id.to_string(),
            language: language.to_string(),
            dll: channel == EditChannel::Dll,
            resolved: true,
            record,
        });
    }
    write_stored_reports(state_dir, &stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_translation::JsonTranslationEntry;
    use std::env;

    fn record(id: &str, kind: &str, key: &str, before: &str, after: Option<&str>) -> EditRecord {
        EditRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            table: "cards".to_string(),
            key: key.to_string(),
            source: None,
            before: Some(before.to_string()),
            after: after.map(str::to_string),
            note: String::new(),
        }
    }

    fn sheet(values: &[(&str, &str)]) -> JsonTranslationSheet {
        JsonTranslationSheet {
            source_path: String::new(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: values
                .iter()
                .map(|(key, value)| JsonTranslationEntry {
                    key: format!("file://Mod/localization/zhs/cards.json#/{key}"),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "原文".to_string(),
                    translated_value: value.to_string(),
                    status: JsonTranslationStatus::Ready,
                })
                .collect(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("sts2-edits-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn pending_edits_follow_ack_reverts_and_latest_edit() {
        let records = vec![
            record("1", KIND_EDIT, "A", "가", Some("나")),
            record("2", KIND_EDIT, "A", "가", Some("다")),
            record("3", KIND_EDIT, "B", "x", Some("y")),
            record("4", KIND_REVERT, "B", "x", None),
            record("5", KIND_REPORT, "C", "틀림", None),
        ];

        let all = pending_edits(&records, None);
        let after_two = pending_edits(&records, Some("2"));

        assert_eq!(all.edits.len(), 1);
        assert_eq!(
            all.edits[&("cards".to_string(), "A".to_string())]
                .after
                .as_deref(),
            Some("다")
        );
        assert_eq!(all.reports.len(), 1);
        assert_eq!(all.last_id.as_deref(), Some("5"));
        assert!(after_two.edits.is_empty());
    }

    #[test]
    fn import_items_tell_game_only_changes_from_conflicts() {
        let pending = pending_edits(
            &[
                record("1", KIND_EDIT, "A", "가", Some("나")),
                record("2", KIND_EDIT, "B", "x", Some("y")),
                record("3", KIND_EDIT, "C", "p", Some("q")),
                record("4", KIND_EDIT, "Gone", "m", Some("n")),
            ],
            None,
        );
        // A untouched in the sheet, B changed in the sheet too, C already matches.
        let items = edit_import_items(&sheet(&[("A", "가"), ("B", "z"), ("C", "q")]), &pending);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, EditImportKind::GameChanged);
        assert_eq!(items[0].game_value, "나");
        assert_eq!(items[1].kind, EditImportKind::Conflict);
        assert_eq!(items[1].sheet_value, "z");
    }

    #[test]
    fn dll_edits_match_sheet_entries_by_their_literal() {
        let mut sheet = sheet(&[]);
        sheet.source_path = "Z:/work/source/Mod.dll".to_string();
        sheet.entries = vec![JsonTranslationEntry {
            key: "dll://10:8".to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "攻击牌".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::Missing,
        }];
        let mut edit = record("1", KIND_EDIT, "攻击牌", "", Some("공격 카드"));
        edit.table = "dll:Mod".to_string();

        let items = edit_import_items(&sheet, &pending_edits(&[edit], None));

        assert_eq!(EditChannel::of_sheet(&sheet), EditChannel::Dll);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, EditImportKind::GameChanged);
        assert_eq!(items[0].sheet_key, "dll://10:8");
        assert_eq!(items[0].game_value, "공격 카드");
    }

    #[test]
    fn reports_survive_until_handled_and_ack_moves_forward_only() {
        let root = temp_dir("ack");
        let shared = root.join("shared");
        let state = root.join("state");
        let log = edits_path(&shared, "M", "kor", EditChannel::Loc);
        fs::create_dir_all(log.parent().unwrap()).expect("dir");
        let lines = [
            record("1", KIND_EDIT, "A", "가", Some("나")),
            record("2", KIND_REPORT, "B", "틀림", None),
        ]
        .iter()
        .map(|record| serde_json::to_string(record).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        fs::write(&log, format!("{lines}\n{{broken\n")).expect("log");

        resolve_report(&shared, &state, "M", "kor", EditChannel::Loc, "2").expect("resolve before ack");
        let resolved_early = read_stored_reports(&state);
        let ack_before = read_edits_ack(&shared, "M", "kor", EditChannel::Loc);
        acknowledge(&shared, &state, "M", "kor", EditChannel::Loc, "2").expect("ack");
        acknowledge(&shared, &state, "M", "kor", EditChannel::Loc, "1").expect("ack again");
        let ack = read_edits_ack(&shared, "M", "kor", EditChannel::Loc);
        let dll_ack = read_edits_ack(&shared, "M", "kor", EditChannel::Dll);
        let after_ack = read_stored_reports(&state);
        fs::remove_dir_all(&root).ok();

        assert!(resolved_early[0].resolved);
        assert_eq!(resolved_early[0].record.key, "B");
        assert_eq!(ack_before, None, "resolving a report does not ack edits");
        assert_eq!(ack.as_deref(), Some("2"));
        assert_eq!(dll_ack, None, "the DLL log keeps its own ack");
        assert!(
            after_ack.is_empty(),
            "handled and acked reports are dropped"
        );
    }
}
