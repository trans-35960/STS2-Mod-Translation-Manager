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
    let imported_values = imported_translation_values(sheet, input_path)?;
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
        },
    ))
}

fn imported_translation_values(
    sheet: &JsonTranslationSheet,
    input_path: &Path,
) -> AppResult<BTreeMap<String, String>> {
    if input_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
    {
        return read_translation_csv(sheet, input_path);
    }
    if input_path.is_dir() {
        return flatten_source_values(input_path);
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
        return Ok(sheet
            .entries
            .into_iter()
            .map(|entry| (entry.key, entry.translated_value))
            .collect());
    }
    let json = serde_json::from_str::<Value>(content).map_err(|source| {
        AppError::InvalidCommand(format!(
            "invalid translated json: {} ({source})",
            input_path.display()
        ))
    })?;
    if let Some(values) = read_slot_translation_json(sheet, &json)? {
        return Ok(values);
    }
    let raw_values = flatten_string_values(&json);
    if sheet.source_path_is_dir() {
        let relative = input_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "translation.json".to_string());
        return Ok(raw_values
            .into_iter()
            .map(|(pointer, value)| (directory_entry_key(&relative, &pointer), value))
            .collect());
    }
    Ok(raw_values)
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

fn read_slot_translation_json(
    sheet: &JsonTranslationSheet,
    json: &Value,
) -> AppResult<Option<BTreeMap<String, String>>> {
    if let Some(values) = read_compact_translation_json(sheet, json)? {
        return Ok(Some(values));
    }
    let Some(entries) = json.get("entries").and_then(Value::as_array) else {
        return Ok(None);
    };
    let slot_map = slot_key_map(sheet);
    let single_file = single_translatable_file(sheet);
    let mut values = BTreeMap::new();
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
        if !is_translation_slot_id(id) {
            continue;
        }
        let translated = entry
            .get("translated_value")
            .or_else(|| entry.get("translation"))
            .or_else(|| entry.get("translated"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if translated.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "empty translated_value for translation slot id {id}"
            )));
        }
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
        let key = slot_map
            .get(&(file.clone(), id.to_string()))
            .ok_or_else(|| {
                AppError::InvalidCommand(format!("unknown translation slot id {id} in {file}"))
            })?;
        if values.contains_key(key) {
            return Err(AppError::InvalidCommand(format!(
                "duplicate translation slot id {id} in {file}"
            )));
        }
        values.insert(key.clone(), translated.to_string());
    }
    Ok(Some(values))
}

fn read_compact_translation_json(
    sheet: &JsonTranslationSheet,
    json: &Value,
) -> AppResult<Option<BTreeMap<String, String>>> {
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
) -> AppResult<Option<BTreeMap<String, String>>> {
    let slot_map = slot_key_map(sheet);
    let mut matched_ids = 0;
    let mut values = BTreeMap::new();
    for (file, value) in object {
        let Some(group) = value.as_object() else {
            continue;
        };
        for (id, value) in group {
            if !is_translation_slot_id(id) {
                continue;
            }
            let translated = value.as_str().unwrap_or_default();
            if translated.is_empty() {
                return Err(AppError::InvalidCommand(format!(
                    "empty translated value for translation slot id {id} in {file}"
                )));
            }
            let compact_file = compact_translation_file(file);
            let key = slot_map
                .get(&(compact_file.clone(), id.clone()))
                .ok_or_else(|| {
                    AppError::InvalidCommand(format!(
                        "unknown translation slot id {id} in {compact_file}"
                    ))
                })?;
            if values.contains_key(key) {
                return Err(AppError::InvalidCommand(format!(
                    "duplicate translation slot id {id} in {compact_file}"
                )));
            }
            matched_ids += 1;
            values.insert(key.clone(), translated.to_string());
        }
    }
    Ok((matched_ids > 0).then_some(values))
}

fn read_flat_compact_translation_json(
    sheet: &JsonTranslationSheet,
    object: &serde_json::Map<String, Value>,
) -> AppResult<Option<BTreeMap<String, String>>> {
    let Some(file) = single_translatable_file(sheet) else {
        if object.keys().any(|id| is_translation_slot_id(id)) {
            return Err(AppError::InvalidCommand(
                "flat translation slot JSON is only supported for single-file sheets; group entries by file path".to_string(),
            ));
        }
        return Ok(None);
    };
    let slot_map = slot_key_map(sheet);
    let mut values = BTreeMap::new();
    let mut matched_ids = 0;
    for (id, value) in object {
        if !is_translation_slot_id(id) {
            continue;
        }
        let translated = value.as_str().unwrap_or_default();
        if translated.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "empty translated value for translation slot id {id}"
            )));
        }
        let key = slot_map.get(&(file.clone(), id.clone())).ok_or_else(|| {
            AppError::InvalidCommand(format!("unknown translation slot id {id} in {file}"))
        })?;
        if values.contains_key(key) {
            return Err(AppError::InvalidCommand(format!(
                "duplicate translation slot id {id} in {file}"
            )));
        }
        matched_ids += 1;
        values.insert(key.clone(), translated.to_string());
    }
    Ok((matched_ids > 0).then_some(values))
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
