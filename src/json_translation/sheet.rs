use super::hardcoded::{hardcoded_capacity_bytes, is_hardcoded_entry_key, utf16le_byte_len};
use super::language_path::{matching_target_language_values, source_path_matches_target_language};
use super::slots::{ensure_translation_slot_ids, stable_slot_key};
use super::source_json::{flatten_source_values, json_text};
use super::types::{
    JsonSheetReport, JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus,
    JsonValidationIssue, JsonValidationReport,
};
use crate::error::{AppError, AppResult};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_or_update_sheet(
    source_path: &Path,
    target_language: &str,
    existing_sheet_path: Option<&Path>,
    output_path: &Path,
) -> AppResult<JsonSheetReport> {
    let current_entries = flatten_source_values(source_path)?;
    let target_entries = if source_path_matches_target_language(source_path, target_language) {
        current_entries.clone()
    } else {
        matching_target_language_values(source_path, target_language)?
    };
    let existing_sheet = match existing_sheet_path {
        Some(path) if path.exists() => {
            let sheet = read_sheet(path)?;
            if sheet.target_language.eq_ignore_ascii_case(target_language) {
                Some(sheet)
            } else {
                None
            }
        }
        _ => None,
    };
    let existing_entries = existing_sheet
        .as_ref()
        .map(|sheet| {
            sheet
                .entries
                .iter()
                .cloned()
                .map(|entry| (entry.key.clone(), entry))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let existing_entries_by_stable_key = existing_sheet
        .as_ref()
        .map(|sheet| {
            sheet
                .entries
                .iter()
                .cloned()
                .map(|entry| (stable_slot_key(&entry.key), entry))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let current_keys = current_entries.keys().cloned().collect::<BTreeSet<_>>();
    let current_stable_keys = current_entries
        .keys()
        .map(|key| stable_slot_key(key))
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::new();

    for (key, source_value) in current_entries {
        let previous_entry = existing_entries
            .get(&key)
            .or_else(|| existing_entries_by_stable_key.get(&stable_slot_key(&key)));
        let (translated_value, status, previous_source_value) = match previous_entry {
            Some(previous) if previous.source_value == source_value => {
                if previous.translated_value.is_empty() {
                    match target_entries.get(&key) {
                        Some(target_value) if !target_value.is_empty() => {
                            (target_value.clone(), JsonTranslationStatus::Ready, None)
                        }
                        _ => (String::new(), JsonTranslationStatus::Missing, None),
                    }
                } else {
                    let status = if previous.status == JsonTranslationStatus::Updated {
                        JsonTranslationStatus::Updated
                    } else {
                        JsonTranslationStatus::Ready
                    };
                    (
                        previous.translated_value.clone(),
                        status,
                        (status == JsonTranslationStatus::Updated)
                            .then(|| previous.previous_source_value.clone())
                            .flatten(),
                    )
                }
            }
            Some(previous) => {
                let status = if target_entries.contains_key(&key) {
                    JsonTranslationStatus::Ready
                } else {
                    JsonTranslationStatus::Updated
                };
                (
                    target_entries
                        .get(&key)
                        .filter(|value| !value.is_empty())
                        .cloned()
                        .unwrap_or_else(|| previous.translated_value.clone()),
                    status,
                    (status == JsonTranslationStatus::Updated)
                        .then(|| previous.source_value.clone()),
                )
            }
            None => match target_entries.get(&key) {
                Some(target_value) if !target_value.is_empty() => {
                    (target_value.clone(), JsonTranslationStatus::Ready, None)
                }
                _ => (String::new(), JsonTranslationStatus::New, None),
            },
        };

        entries.push(JsonTranslationEntry {
            key,
            slot_id: previous_entry.and_then(|previous| previous.slot_id.clone()),
            previous_source_value,
            source_value,
            translated_value,
            status,
        });
    }

    for (key, previous) in existing_entries {
        if !current_keys.contains(&key)
            && !current_stable_keys.contains(&stable_slot_key(&key))
            && !is_hardcoded_entry_key(&key)
        {
            entries.push(JsonTranslationEntry {
                key,
                slot_id: previous.slot_id,
                previous_source_value: None,
                source_value: previous.source_value,
                translated_value: previous.translated_value,
                status: JsonTranslationStatus::Removed,
            });
        }
    }

    entries.sort_by(|left, right| left.key.cmp(&right.key));
    ensure_translation_slot_ids(&mut entries);
    let sheet = JsonTranslationSheet {
        source_path: source_path.to_string_lossy().to_string(),
        target_language: target_language.to_string(),
        updated_epoch: epoch_now(),
        entries,
    };

    write_sheet(output_path, &sheet)?;
    Ok(report_for_sheet(output_path.to_path_buf(), &sheet))
}

pub fn validate_sheet(path: &Path) -> AppResult<JsonValidationReport> {
    let sheet = read_sheet(path)?;
    Ok(validate_translation_sheet(&sheet))
}

pub fn validate_translation_sheet(sheet: &JsonTranslationSheet) -> JsonValidationReport {
    let mut missing_entries = Vec::new();
    let mut updated_entries = Vec::new();
    let mut removed_entries = Vec::new();
    let mut format_issues = Vec::new();

    for entry in &sheet.entries {
        match entry.status {
            JsonTranslationStatus::Removed => removed_entries.push(entry.key.clone()),
            JsonTranslationStatus::Updated => {
                updated_entries.push(entry.key.clone());
                if is_translatable_entry(entry) && entry.translated_value.is_empty() {
                    missing_entries.push(entry.key.clone());
                }
            }
            _ if is_translatable_entry(entry) && entry.translated_value.is_empty() => {
                missing_entries.push(entry.key.clone())
            }
            _ => {}
        }
        format_issues.extend(validation_format_issues(entry));
    }

    JsonValidationReport {
        valid: missing_entries.is_empty() && updated_entries.is_empty() && format_issues.is_empty(),
        total_entries: sheet
            .entries
            .iter()
            .filter(|entry| is_translatable_entry(entry))
            .count(),
        missing_entries,
        updated_entries,
        removed_entries,
        format_issues,
    }
}

pub fn read_sheet(path: &Path) -> AppResult<JsonTranslationSheet> {
    let raw_content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    let content = json_text(&raw_content);
    if content.trim().is_empty() {
        return Err(AppError::InvalidCommand(format!(
            "invalid translation sheet json: {} (file is empty)",
            path.display()
        )));
    }
    let mut sheet: JsonTranslationSheet = serde_json::from_str(&content).map_err(|source| {
        AppError::InvalidCommand(format!(
            "invalid translation sheet json: {} ({source})",
            path.display()
        ))
    })?;
    ensure_translation_slot_ids(&mut sheet.entries);
    Ok(sheet)
}

pub fn write_sheet(path: &Path, sheet: &JsonTranslationSheet) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let output = serde_json::to_string_pretty(sheet).map_err(|source| {
        AppError::InvalidCommand(format!("failed to serialize translation sheet: {source}"))
    })?;
    fs::write(path, output).map_err(|source| AppError::io(path, source))
}

fn report_for_sheet(sheet_path: PathBuf, sheet: &JsonTranslationSheet) -> JsonSheetReport {
    JsonSheetReport {
        sheet_path,
        entries: sheet.entries.len(),
        new_entries: count_status(sheet, JsonTranslationStatus::New),
        updated_entries: count_status(sheet, JsonTranslationStatus::Updated),
        missing_entries: count_status(sheet, JsonTranslationStatus::Missing),
        removed_entries: count_status(sheet, JsonTranslationStatus::Removed),
    }
}

fn count_status(sheet: &JsonTranslationSheet, status: JsonTranslationStatus) -> usize {
    sheet
        .entries
        .iter()
        .filter(|entry| {
            entry.status == status
                && (status == JsonTranslationStatus::Removed || is_translatable_entry(entry))
        })
        .count()
}

pub(crate) fn is_translatable_entry(entry: &JsonTranslationEntry) -> bool {
    entry.status != JsonTranslationStatus::Removed && !entry.source_value.trim().is_empty()
}

fn validation_format_issues(entry: &JsonTranslationEntry) -> Vec<JsonValidationIssue> {
    if !is_translatable_entry(entry) || entry.translated_value.is_empty() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    if is_hardcoded_entry_key(&entry.key) {
        if let Some(capacity) = hardcoded_capacity_bytes(&entry.key) {
            let translated_len = utf16le_byte_len(&entry.translated_value);
            if translated_len > capacity {
                issues.push(JsonValidationIssue {
                    key: entry.key.clone(),
                    kind: "hardcoded_capacity".to_string(),
                    message: format!(
                        "DLL 고정 문자열보다 번역이 깁니다: 번역 {translated_len} bytes / 허용 {capacity} bytes"
                    ),
                });
            }
        }
    }
    let source_newlines = entry.source_value.matches('\n').count();
    let translated_newlines = entry.translated_value.matches('\n').count();
    if source_newlines != translated_newlines {
        issues.push(JsonValidationIssue {
            key: entry.key.clone(),
            kind: "line_breaks".to_string(),
            message: format!(
                "줄바꿈 수가 다릅니다: 원본 {source_newlines} / 번역 {translated_newlines}"
            ),
        });
    }

    let source_nl_markers = count_word_marker(&entry.source_value, "NL");
    let translated_nl_markers = count_word_marker(&entry.translated_value, "NL");
    if source_nl_markers != translated_nl_markers {
        issues.push(JsonValidationIssue {
            key: entry.key.clone(),
            kind: "line_break_marker".to_string(),
            message: format!(
                "NL 줄바꿈 마커 수가 다릅니다: 원본 {source_nl_markers} / 번역 {translated_nl_markers}"
            ),
        });
    }

    compare_token_family(
        entry,
        "angle_tags",
        "꺾쇠 태그",
        bracket_tokens(&entry.source_value, '<', '>'),
        bracket_tokens(&entry.translated_value, '<', '>'),
        &mut issues,
    );
    compare_token_family(
        entry,
        "square_tags",
        "대괄호 태그",
        bracket_tokens(&entry.source_value, '[', ']'),
        bracket_tokens(&entry.translated_value, '[', ']'),
        &mut issues,
    );
    compare_token_family(
        entry,
        "placeholders",
        "중괄호 플레이스홀더",
        placeholder_tokens(&entry.source_value),
        placeholder_tokens(&entry.translated_value),
        &mut issues,
    );
    compare_token_family(
        entry,
        "bang_tokens",
        "느낌표 토큰",
        bang_tokens(&entry.source_value),
        bang_tokens(&entry.translated_value),
        &mut issues,
    );

    issues
}

fn compare_token_family(
    entry: &JsonTranslationEntry,
    kind: &str,
    label: &str,
    source: Vec<String>,
    translated: Vec<String>,
    issues: &mut Vec<JsonValidationIssue>,
) {
    if token_counts(source) == token_counts(translated) {
        return;
    }
    issues.push(JsonValidationIssue {
        key: entry.key.clone(),
        kind: kind.to_string(),
        message: format!("{label} 구성이 원본과 다릅니다."),
    });
}

fn token_counts(tokens: Vec<String>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for token in tokens {
        *counts.entry(token).or_insert(0) += 1;
    }
    counts
}

fn bracket_tokens(value: &str, open: char, close: char) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != open {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len() && chars[index] != close && chars[index] != '\n' {
            index += 1;
        }
        if index >= chars.len() || chars[index] != close {
            continue;
        }
        let token = chars[start..=index].iter().collect::<String>();
        if token.len() <= 80 && token_has_structural_shape(&token) {
            output.push(token);
        }
        index += 1;
    }
    output
}

fn placeholder_tokens(value: &str) -> Vec<String> {
    placeholder_bracket_tokens(value)
        .into_iter()
        .map(normalize_placeholder_token)
        .collect()
}

fn placeholder_bracket_tokens(value: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '{' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len() && chars[index] != '}' && chars[index] != '\n' {
            index += 1;
        }
        if index >= chars.len() || chars[index] != '}' {
            continue;
        }
        let token = chars[start..=index].iter().collect::<String>();
        if token.len() <= 120 && token_has_placeholder_shape(&token) {
            output.push(token);
        }
        index += 1;
    }
    output
}

fn normalize_placeholder_token(token: String) -> String {
    let Some(inner) = token
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return token;
    };
    if let Some(show_index) = inner.find(":show:") {
        return format!("{{{}:show:<text>}}", &inner[..show_index]);
    }
    if let Some((prefix, variants)) = inner.split_once(':') {
        if variants.contains('|') {
            let placeholders = variants
                .split('|')
                .map(|_| "<text>")
                .collect::<Vec<_>>()
                .join("|");
            return format!("{{{prefix}:{placeholders}}}");
        }
    }
    token
}

fn token_has_placeholder_shape(token: &str) -> bool {
    token.chars().any(|character| {
        character.is_ascii_alphabetic() || matches!(character, '/' | '_' | ':' | '#')
    })
}

fn token_has_structural_shape(token: &str) -> bool {
    token.chars().any(|character| {
        character.is_ascii_alphabetic() || matches!(character, '/' | '_' | ':' | '#')
    }) && !token.chars().any(|character| character.is_whitespace())
}

fn bang_tokens(value: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '!' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len()
            && chars[index] != '!'
            && chars[index].is_ascii_alphanumeric()
            && index - start <= 12
        {
            index += 1;
        }
        if index < chars.len() && chars[index] == '!' && index > start + 1 {
            output.push(chars[start..=index].iter().collect());
            index += 1;
        }
    }
    output
}

fn count_word_marker(value: &str, marker: &str) -> usize {
    value
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|part| *part == marker)
        .count()
}

pub(crate) fn epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
