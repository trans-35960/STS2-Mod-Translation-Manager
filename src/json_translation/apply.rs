use super::hardcoded::{apply_hardcoded_sheet, is_hardcoded_source_file};
use super::language_path::{matching_target_language_path, target_language_relative_path};
use super::sheet::read_sheet;
use super::source_json::{collect_json_files, read_json, slash_path, split_directory_entry_key};
use super::types::{JsonApplyReport, JsonTranslationSheet, JsonTranslationStatus};
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn apply_sheet(sheet_path: &Path, output_path: &Path) -> AppResult<JsonApplyReport> {
    let sheet = read_sheet(sheet_path)?;
    let source_path = PathBuf::from(&sheet.source_path);
    if is_hardcoded_source_file(&source_path) {
        return apply_hardcoded_sheet(&sheet, &source_path, output_path);
    }
    if source_path.is_dir() {
        return apply_directory_sheet(&sheet, &source_path, output_path);
    }
    let mut json = read_json(&source_path)?;
    let mut applied_entries = 0;

    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed
            || entry.source_value.trim().is_empty()
            || entry.translated_value.is_empty()
        {
            continue;
        }

        let Some(target) = json.pointer_mut(&entry.key) else {
            continue;
        };
        if target.is_string() {
            *target = Value::String(entry.translated_value.clone());
            applied_entries += 1;
        }
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let output = serde_json::to_string_pretty(&json).map_err(|source| {
        AppError::InvalidCommand(format!("failed to serialize translated json: {source}"))
    })?;
    fs::write(output_path, output).map_err(|source| AppError::io(output_path, source))?;

    Ok(JsonApplyReport {
        output_path: output_path.to_path_buf(),
        applied_entries,
    })
}

pub fn target_language_output_path(sheet: &JsonTranslationSheet) -> Option<PathBuf> {
    let source_path = Path::new(&sheet.source_path);
    if is_hardcoded_source_file(source_path) {
        return None;
    }
    matching_target_language_path(source_path, &sheet.target_language)
}

pub fn apply_sheet_to_target_language(sheet_path: &Path) -> AppResult<JsonApplyReport> {
    let sheet = read_sheet(sheet_path)?;
    let output_path = target_language_output_path(&sheet).ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "target language folder cannot be inferred from {}",
            sheet.source_path
        ))
    })?;
    apply_sheet(sheet_path, &output_path)
}

fn apply_directory_sheet(
    sheet: &JsonTranslationSheet,
    source_dir: &Path,
    output_dir: &Path,
) -> AppResult<JsonApplyReport> {
    let files = collect_json_files(source_dir)?;
    let mut applied_entries = 0;
    for source_file in files {
        let relative = source_file.strip_prefix(source_dir).map_err(|_| {
            AppError::InvalidCommand(format!(
                "source file outside directory: {}",
                source_file.display()
            ))
        })?;
        let relative_text = slash_path(relative);
        let mut json = read_json(&source_file)?;
        for entry in &sheet.entries {
            if entry.status == JsonTranslationStatus::Removed
                || entry.source_value.trim().is_empty()
                || entry.translated_value.is_empty()
            {
                continue;
            }
            let Some((entry_file, pointer)) = split_directory_entry_key(&entry.key) else {
                continue;
            };
            if entry_file != relative_text {
                continue;
            }
            let Some(target) = json.pointer_mut(pointer) else {
                continue;
            };
            if target.is_string() {
                *target = Value::String(entry.translated_value.clone());
                applied_entries += 1;
            }
        }
        let output_relative = target_language_relative_path(relative, &sheet.target_language);
        let output_file = output_dir.join(output_relative);
        if let Some(parent) = output_file.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
        }
        let output = serde_json::to_string_pretty(&json).map_err(|source| {
            AppError::InvalidCommand(format!("failed to serialize translated json: {source}"))
        })?;
        fs::write(&output_file, output).map_err(|source| AppError::io(&output_file, source))?;
    }

    Ok(JsonApplyReport {
        output_path: output_dir.to_path_buf(),
        applied_entries,
    })
}
