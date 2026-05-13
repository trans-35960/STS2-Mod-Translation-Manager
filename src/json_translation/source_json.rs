use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::hardcoded::{flatten_hardcoded_values, is_hardcoded_source_file};

pub(crate) fn read_json(path: &Path) -> AppResult<Value> {
    let raw_content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    let content = json_text(&raw_content);
    if content.trim().is_empty() {
        return Err(AppError::InvalidCommand(format!(
            "invalid json: {} (file is empty)",
            path.display()
        )));
    }
    serde_json::from_str(content).map_err(|source| {
        AppError::InvalidCommand(format!("invalid json: {} ({source})", path.display()))
    })
}

pub(crate) fn json_text(content: &str) -> &str {
    content.trim_start_matches('\u{feff}')
}

pub(crate) fn flatten_source_values(source_path: &Path) -> AppResult<BTreeMap<String, String>> {
    if source_path.is_file() && is_hardcoded_source_file(source_path) {
        return flatten_hardcoded_values(source_path);
    }
    if source_path.is_dir() {
        let mut entries = BTreeMap::new();
        for file in collect_json_files(source_path)? {
            let relative = file.strip_prefix(source_path).map_err(|_| {
                AppError::InvalidCommand(format!("json outside source dir: {}", file.display()))
            })?;
            let relative_text = slash_path(relative);
            let json = read_json(&file)?;
            for (pointer, value) in flatten_string_values(&json) {
                entries.insert(directory_entry_key(&relative_text, &pointer), value);
            }
        }
        return Ok(entries);
    }
    let current_json = read_json(source_path)?;
    Ok(flatten_string_values(&current_json))
}

pub(crate) fn collect_json_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_json_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|source| AppError::io(path, source))?;
    if metadata.is_file() {
        if is_json_translation_file(path) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|source| AppError::io(path, source))? {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        collect_json_files_inner(&entry.path(), files)?;
    }
    Ok(())
}

fn is_json_translation_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "json" | "loc"))
        .unwrap_or(false)
}

pub(crate) fn directory_entry_key(relative_file: &str, pointer: &str) -> String {
    format!("file://{relative_file}#{pointer}")
}

pub(crate) fn split_directory_entry_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("file://")?;
    rest.split_once('#')
}

pub(crate) fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn flatten_string_values(value: &Value) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    flatten_inner(value, "", &mut entries);
    entries
}

fn flatten_inner(value: &Value, pointer: &str, entries: &mut BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                return;
            }
            entries.insert(
                if pointer.is_empty() {
                    "/".to_string()
                } else {
                    pointer.to_string()
                },
                text.clone(),
            );
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten_inner(item, &format!("{pointer}/{index}"), entries);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                flatten_inner(item, &format!("{pointer}/{}", escape_pointer(key)), entries);
            }
        }
        _ => {}
    }
}

fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
