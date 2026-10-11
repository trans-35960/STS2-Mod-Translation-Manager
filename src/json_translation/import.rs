use super::sheet::{epoch_now, is_translatable_entry};
use super::slots::{
    compact_translation_file, is_translation_slot_id, single_translatable_file, slot_key_map,
};
use super::source_json::{
    directory_entry_key, flatten_source_values, flatten_string_values, json_text,
};
use super::types::{JsonImportReport, JsonTranslationSheet, JsonTranslationStatus};
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn import_translations(
    sheet: &JsonTranslationSheet,
    input_path: &Path,
) -> AppResult<(JsonTranslationSheet, JsonImportReport)> {
    let (imported_values, skipped) = imported_translation_values(sheet, input_path)?;
    let mut updated = sheet.clone();
    let mut matched_entries = 0;
    for entry in &mut updated.entries {
        if entry.status == JsonTranslationStatus::Removed {
            continue;
        }
        let Some(value) = imported_values.get(&entry.key) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        entry.translated_value = value.clone();
        entry.status = JsonTranslationStatus::Ready;
        matched_entries += 1;
    }
    updated.updated_epoch = epoch_now();
    let unmatched_entries = updated
        .entries
        .iter()
        .filter(|entry| {
            is_translatable_entry(entry)
                && entry.translated_value.is_empty()
                && !imported_values.contains_key(&entry.key)
        })
        .count();
    Ok((
        updated,
        JsonImportReport {
            input_path: input_path.to_path_buf(),
            matched_entries,
            unmatched_entries,
            skipped,
        },
    ))
}

fn imported_translation_values(
    sheet: &JsonTranslationSheet,
    input_path: &Path,
) -> AppResult<(BTreeMap<String, String>, Vec<String>)> {
    if input_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
    {
        return read_translation_csv(sheet, input_path).map(|values| (values, Vec::new()));
    }
    if input_path.is_dir() {
        return flatten_source_values(input_path).map(|values| (values, Vec::new()));
    }

    let raw_content =
        fs::read_to_string(input_path).map_err(|source| AppError::io(input_path, source))?;
    let content = json_text(&raw_content);
    if content.trim().is_empty() {
        return Err(AppError::InvalidCommand(format!(
            "invalid translated json: {} (file is empty)",
            input_path.display()
        )));
    }
    if let Ok(sheet) = serde_json::from_str::<JsonTranslationSheet>(content) {
        return Ok((
            sheet
                .entries
                .into_iter()
                .map(|entry| (entry.key, entry.translated_value))
                .collect(),
            Vec::new(),
        ));
    }
    let json = serde_json::from_str::<Value>(content).map_err(|source| {
        AppError::InvalidCommand(format!(
            "invalid translated json: {} ({source})",
            input_path.display()
        ))
    })?;
    if let Some(slots) = read_slot_translation_json(sheet, &json)? {
        return Ok(slots);
    }
    let raw_values = flatten_string_values(&json);
    if sheet.source_path_is_dir() {
        let relative = input_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "translation.json".to_string());
        return Ok((
            raw_values
                .into_iter()
                .map(|(pointer, value)| (directory_entry_key(&relative, &pointer), value))
                .collect(),
            Vec::new(),
        ));
    }
    Ok((raw_values, Vec::new()))
}

fn read_translation_csv(
    sheet: &JsonTranslationSheet,
    path: &Path,
) -> AppResult<BTreeMap<String, String>> {
    let content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    let rows = parse_csv(&content);
    let Some(header) = rows.first() else {
        return Ok(BTreeMap::new());
    };
    let column = |name: &str| {
        header
            .iter()
            .position(|value| value.eq_ignore_ascii_case(name))
    };
    let id_index = column("id");
    let key_index = column("key").unwrap_or(0);
    let file_index = column("file");
    let translated_index = column("translated_value")
        .or_else(|| column("translation"))
        .or_else(|| column("translated"))
        .unwrap_or_else(|| header.len().saturating_sub(1));
    let slot_map = slot_key_map(sheet);
    let single_file = single_translatable_file(sheet);
    let mut values = BTreeMap::new();
    for row in rows.into_iter().skip(1) {
        if let Some(index) = id_index {
            let id = row.get(index).map(String::as_str).unwrap_or_default();
            let value = row.get(translated_index).cloned().unwrap_or_default();
            if let Some(key) = csv_id_key(id, &row, file_index, &slot_map, single_file.as_ref()) {
                if !value.is_empty() {
                    values.insert(key, value);
                }
                continue;
            }
        }
        let key = row.get(key_index).cloned().unwrap_or_default();
        let value = row.get(translated_index).cloned().unwrap_or_default();
        if key.trim().is_empty() || value.is_empty() {
            continue;
        }
        let entry_key = if key.starts_with("file://") {
            key
        } else if let Some(index) = file_index {
            let file = row.get(index).cloned().unwrap_or_default();
            if file.trim().is_empty() {
                key
            } else {
                directory_entry_key(&file, &key)
            }
        } else {
            key
        };
        values.insert(entry_key, value);
    }
    Ok(values)
}

fn csv_id_key(
    id: &str,
    row: &[String],
    file_index: Option<usize>,
    slot_map: &BTreeMap<(String, String), String>,
    single_file: Option<&String>,
) -> Option<String> {
    if is_translation_slot_id(id) {
        let file = file_index
            .and_then(|index| row.get(index))
            .map(|file| compact_translation_file(file))
            .filter(|file| !file.trim().is_empty())
            .or_else(|| single_file.cloned())?;
        return slot_map.get(&(file, id.to_string())).cloned();
    }
    None
}

type SlotValues = (BTreeMap<String, String>, Vec<String>);

/// Collects slot-id translations, tolerating the small mistakes AI answers
/// make. Matching entries are kept; empty values, unknown ids and duplicates
/// are skipped and reported. Only an answer where nothing matches is an error.
struct SlotImport {
    slots: BTreeMap<(String, String), String>,
    values: BTreeMap<String, String>,
    seen_ids: usize,
    empty: Vec<String>,
    unknown: Vec<String>,
    duplicate: Vec<String>,
}

impl SlotImport {
    fn new(sheet: &JsonTranslationSheet) -> Self {
        Self {
            slots: slot_key_map(sheet)
                .into_iter()
                .filter_map(|((file, id), key)| Some(((file, normalize_slot_id(&id)?), key)))
                .collect(),
            values: BTreeMap::new(),
            seen_ids: 0,
            empty: Vec::new(),
            unknown: Vec::new(),
            duplicate: Vec::new(),
        }
    }

    fn add(&mut self, file: &str, id: &str, translated: &str) {
        let Some(normalized) = normalize_slot_id(id) else {
            return;
        };
        self.seen_ids += 1;
        let label = format!("{file}:{}", id.trim());
        if translated.trim().is_empty() {
            self.empty.push(label);
            return;
        }
        let Some(key) = self.slots.get(&(file.to_string(), normalized)) else {
            self.unknown.push(label);
            return;
        };
        if self.values.contains_key(key) {
            self.duplicate.push(label);
            return;
        }
        self.values.insert(key.clone(), translated.to_string());
    }

    fn finish(self) -> AppResult<Option<SlotValues>> {
        if self.seen_ids == 0 {
            return Ok(None);
        }
        if self.values.is_empty() {
            let reason = if !self.unknown.is_empty() {
                format!("unknown translation slot id {}", preview_ids(&self.unknown))
            } else if !self.empty.is_empty() {
                format!(
                    "empty translated value for translation slot id {}",
                    preview_ids(&self.empty)
                )
            } else {
                format!(
                    "duplicate translation slot id {}",
                    preview_ids(&self.duplicate)
                )
            };
            return Err(AppError::InvalidCommand(reason));
        }
        let mut skipped = Vec::new();
        for (label, ids) in [
            ("빈 번역값", &self.empty),
            ("시트에 없는 id", &self.unknown),
            ("중복 id", &self.duplicate),
        ] {
            if !ids.is_empty() {
                skipped.push(format!("{label} {}개: {}", ids.len(), preview_ids(ids)));
            }
        }
        Ok(Some((self.values, skipped)))
    }
}

/// `K0012-AB`, ` k12-ab ` and `k012-ab` all name slot 12 with checksum `ab`.
pub(crate) fn normalize_slot_id(id: &str) -> Option<String> {
    let id = id
        .trim()
        .trim_matches(['"', '\'', '`'])
        .to_ascii_lowercase();
    let (prefix, checksum) = id.split_once('-')?;
    let digits = prefix.strip_prefix('k')?;
    if digits.is_empty()
        || !digits.chars().all(|ch| ch.is_ascii_digit())
        || checksum.len() != 2
        || !checksum.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return None;
    }
    let index = digits.trim_start_matches('0');
    Some(format!(
        "k{}-{checksum}",
        if index.is_empty() { "0" } else { index }
    ))
}

fn preview_ids(ids: &[String]) -> String {
    let mut preview = ids.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
    if ids.len() > 8 {
        preview.push_str(" ...");
    }
    preview
}

fn read_slot_translation_json(
    sheet: &JsonTranslationSheet,
    json: &Value,
) -> AppResult<Option<SlotValues>> {
    if let Some(values) = read_compact_translation_json(sheet, json)? {
        return Ok(Some(values));
    }
    let Some(entries) = json.get("entries").and_then(Value::as_array) else {
        return Ok(None);
    };
    let single_file = single_translatable_file(sheet);
    let mut import = SlotImport::new(sheet);
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
        if normalize_slot_id(id).is_none() {
            continue;
        }
        let translated = entry
            .get("translated_value")
            .or_else(|| entry.get("translation"))
            .or_else(|| entry.get("translated"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let file = entry
            .get("source")
            .or_else(|| entry.get("file"))
            .and_then(Value::as_str)
            .map(compact_translation_file)
            .or_else(|| single_file.clone())
            .ok_or_else(|| {
                AppError::InvalidCommand(format!(
                    "translation slot id {id} needs a file because this sheet has multiple files"
                ))
            })?;
        import.add(&file, id, translated);
    }
    Ok(Some(import.finish()?.unwrap_or_default()))
}

fn read_compact_translation_json(
    sheet: &JsonTranslationSheet,
    json: &Value,
) -> AppResult<Option<SlotValues>> {
    let Some(object) = json.as_object() else {
        return Ok(None);
    };
    if let Some(values) = read_grouped_compact_translation_json(sheet, object)? {
        return Ok(Some(values));
    }
    read_flat_compact_translation_json(sheet, object)
}

fn read_grouped_compact_translation_json(
    sheet: &JsonTranslationSheet,
    object: &serde_json::Map<String, Value>,
) -> AppResult<Option<SlotValues>> {
    let mut import = SlotImport::new(sheet);
    for (file, value) in object {
        let Some(group) = value.as_object() else {
            continue;
        };
        let compact_file = compact_translation_file(file);
        for (id, value) in group {
            import.add(&compact_file, id, value.as_str().unwrap_or_default());
        }
    }
    import.finish()
}

fn read_flat_compact_translation_json(
    sheet: &JsonTranslationSheet,
    object: &serde_json::Map<String, Value>,
) -> AppResult<Option<SlotValues>> {
    let Some(file) = single_translatable_file(sheet) else {
        if object.keys().any(|id| normalize_slot_id(id).is_some()) {
            return Err(AppError::InvalidCommand(
                "flat translation slot JSON is only supported for single-file sheets; group entries by file path".to_string(),
            ));
        }
        return Ok(None);
    };
    let mut import = SlotImport::new(sheet);
    for (id, value) in object {
        import.add(&file, id, value.as_str().unwrap_or_default());
    }
    import.finish()
}

fn parse_csv(content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = content.chars().peekable();
    let mut in_quotes = false;
    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !in_quotes => {}
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}
