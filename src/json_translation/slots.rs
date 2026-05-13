use super::sheet::is_translatable_entry;
use super::types::{JsonTranslationEntry, JsonTranslationSheet, JsonValidationReport};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn compact_source_translation_map(
    sheet: &JsonTranslationSheet,
    only_empty: bool,
) -> BTreeMap<String, BTreeMap<String, String>> {
    compact_source_translation_map_with_keys(sheet, only_empty, None)
}

pub fn compact_source_translation_map_with_keys(
    sheet: &JsonTranslationSheet,
    only_empty: bool,
    include_keys: Option<&BTreeSet<String>>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut output = BTreeMap::<String, BTreeMap<String, String>>::new();
    for slot in translation_slot_entries(sheet) {
        if include_keys.is_some_and(|keys| !keys.contains(&slot.entry.key)) {
            continue;
        }
        if only_empty && !slot.entry.translated_value.is_empty() {
            continue;
        }
        output
            .entry(slot.compact_file)
            .or_default()
            .insert(slot.id, slot.entry.source_value.clone());
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactValidationIssueEntry {
    pub source: String,
    pub translation: String,
    pub issues: Vec<String>,
}

pub fn compact_validation_issue_translation_map(
    sheet: &JsonTranslationSheet,
    validation: &JsonValidationReport,
) -> BTreeMap<String, BTreeMap<String, CompactValidationIssueEntry>> {
    compact_validation_issue_translation_map_with_keys(sheet, validation, None)
}

pub fn compact_validation_issue_translation_map_with_keys(
    sheet: &JsonTranslationSheet,
    validation: &JsonValidationReport,
    include_keys: Option<&BTreeSet<String>>,
) -> BTreeMap<String, BTreeMap<String, CompactValidationIssueEntry>> {
    let issues_by_key = validation_issues_by_key(validation);
    let mut output = BTreeMap::<String, BTreeMap<String, CompactValidationIssueEntry>>::new();
    for slot in translation_slot_entries(sheet) {
        if include_keys.is_some_and(|keys| !keys.contains(&slot.entry.key)) {
            continue;
        }
        let Some(issues) = issues_by_key.get(&slot.entry.key) else {
            continue;
        };
        output.entry(slot.compact_file).or_default().insert(
            slot.id,
            CompactValidationIssueEntry {
                source: slot.entry.source_value.clone(),
                translation: slot.entry.translated_value.clone(),
                issues: issues.clone(),
            },
        );
    }
    output
}

fn validation_issues_by_key(validation: &JsonValidationReport) -> BTreeMap<String, Vec<String>> {
    let mut output = BTreeMap::<String, Vec<String>>::new();
    for key in &validation.missing_entries {
        push_unique_issue(&mut output, key, "번역값이 비어 있습니다.");
    }
    for key in &validation.updated_entries {
        push_unique_issue(&mut output, key, "원본 값이 바뀐 항목입니다.");
    }
    for key in &validation.removed_entries {
        push_unique_issue(&mut output, key, "원본에서 삭제된 항목입니다.");
    }
    for issue in &validation.format_issues {
        push_unique_issue(&mut output, &issue.key, &issue.message);
    }
    output
}

fn push_unique_issue(output: &mut BTreeMap<String, Vec<String>>, key: &str, message: &str) {
    let messages = output.entry(key.to_string()).or_default();
    if !messages.iter().any(|existing| existing == message) {
        messages.push(message.to_string());
    }
}

fn split_translation_key(key: &str) -> (String, String) {
    if let Some(rest) = key.strip_prefix("file://") {
        let mut parts = rest.splitn(2, '#');
        let file = parts.next().unwrap_or("source.json").to_string();
        let key = parts.next().unwrap_or("").to_string();
        return (file, key);
    }
    ("source.json".to_string(), key.to_string())
}

#[derive(Debug, Clone)]
pub struct TranslationSlotEntry<'a> {
    pub compact_file: String,
    pub id: String,
    pub entry: &'a JsonTranslationEntry,
}

pub fn translation_slot_entries(sheet: &JsonTranslationSheet) -> Vec<TranslationSlotEntry<'_>> {
    translation_slot_entries_from_entries(&sheet.entries)
}

pub(crate) fn ensure_translation_slot_ids(entries: &mut [JsonTranslationEntry]) {
    let mut file_counts = BTreeMap::<String, usize>::new();
    let mut max_indexes = BTreeMap::<String, usize>::new();
    let mut used_ids = BTreeMap::<String, BTreeSet<String>>::new();

    for entry in entries.iter() {
        if !is_translatable_entry(entry) {
            continue;
        }
        let compact_file = compact_file_for_key(&entry.key);
        *file_counts.entry(compact_file.clone()).or_default() += 1;
        if let Some(slot_id) = entry
            .slot_id
            .as_deref()
            .filter(|id| is_translation_slot_id(id))
        {
            used_ids
                .entry(compact_file.clone())
                .or_default()
                .insert(slot_id.to_string());
            if let Some(index) = translation_slot_index(slot_id) {
                let max_index = max_indexes.entry(compact_file).or_default();
                *max_index = (*max_index).max(index);
            }
        }
    }

    let file_widths = file_counts
        .iter()
        .map(|(file, count)| {
            let existing_width = used_ids
                .get(file)
                .and_then(|ids| ids.iter().map(|id| id_prefix_width(id)).max())
                .unwrap_or(3);
            (
                file.clone(),
                count.to_string().len().max(existing_width).max(3),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut next_indexes = max_indexes
        .into_iter()
        .map(|(file, index)| (file, index + 1))
        .collect::<BTreeMap<_, _>>();

    for entry in entries.iter_mut() {
        if !is_translatable_entry(entry) {
            continue;
        }
        if entry.slot_id.as_deref().is_some_and(is_translation_slot_id) {
            continue;
        }
        let compact_file = compact_file_for_key(&entry.key);
        let width = file_widths.get(&compact_file).copied().unwrap_or(3);
        let used = used_ids.entry(compact_file.clone()).or_default();
        let next_index = next_indexes.entry(compact_file.clone()).or_insert(1);
        loop {
            let candidate = translation_slot_id(*next_index, width, &stable_slot_key(&entry.key));
            *next_index += 1;
            if used.insert(candidate.clone()) {
                entry.slot_id = Some(candidate);
                break;
            }
        }
    }
}

fn translation_slot_entries_from_entries(
    entries: &[JsonTranslationEntry],
) -> Vec<TranslationSlotEntry<'_>> {
    let mut file_counts = BTreeMap::<String, usize>::new();
    for entry in entries {
        if !is_translatable_entry(entry) {
            continue;
        }
        let (file, _) = split_translation_key(&entry.key);
        *file_counts
            .entry(compact_translation_file(&file))
            .or_default() += 1;
    }
    let file_widths = file_counts
        .iter()
        .map(|(file, count)| (file.clone(), count.to_string().len().max(3)))
        .collect::<BTreeMap<_, _>>();
    let mut file_indexes = BTreeMap::<String, usize>::new();
    let mut slots = Vec::new();
    for entry in entries {
        if !is_translatable_entry(entry) {
            continue;
        }
        let (file, _) = split_translation_key(&entry.key);
        let compact_file = compact_translation_file(&file);
        let index = file_indexes.entry(compact_file.clone()).or_default();
        *index += 1;
        let width = file_widths.get(&compact_file).copied().unwrap_or(3);
        let id = entry
            .slot_id
            .as_deref()
            .filter(|id| is_translation_slot_id(id))
            .map(ToString::to_string)
            .unwrap_or_else(|| translation_slot_id(*index, width, &stable_slot_key(&entry.key)));
        slots.push(TranslationSlotEntry {
            id,
            compact_file,
            entry,
        });
    }
    slots
}

pub(crate) fn slot_key_map(sheet: &JsonTranslationSheet) -> BTreeMap<(String, String), String> {
    translation_slot_entries(sheet)
        .into_iter()
        .map(|slot| ((slot.compact_file, slot.id), slot.entry.key.clone()))
        .collect()
}

pub(crate) fn single_translatable_file(sheet: &JsonTranslationSheet) -> Option<String> {
    let files = translation_slot_entries(sheet)
        .into_iter()
        .map(|slot| slot.compact_file)
        .collect::<BTreeSet<_>>();
    if files.len() == 1 {
        files.into_iter().next()
    } else {
        None
    }
}

pub(crate) fn compact_translation_file(file: &str) -> String {
    let normalized = file.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let Some(index) = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))
    else {
        return normalized;
    };
    if index + 2 >= parts.len() {
        return normalized;
    }
    parts[index + 2..].join("/")
}

fn compact_file_for_key(key: &str) -> String {
    let (file, _) = split_translation_key(key);
    compact_translation_file(&file)
}

pub(crate) fn stable_slot_key(key: &str) -> String {
    let (file, pointer) = split_translation_key(key);
    format!("{}#{pointer}", compact_translation_file(&file))
}

fn translation_slot_id(index: usize, width: usize, stable_key: &str) -> String {
    let checksum = slot_checksum(stable_key);
    format!("k{index:0width$}-{checksum}")
}

fn slot_checksum(stable_key: &str) -> String {
    let hash = fnv64(stable_key.as_bytes());
    let value = hash % 1296;
    format!("{:0>2}", base36(value))
}

fn translation_slot_index(value: &str) -> Option<usize> {
    let (prefix, _) = value.split_once('-')?;
    prefix.strip_prefix('k')?.parse().ok()
}

fn id_prefix_width(value: &str) -> usize {
    value
        .split_once('-')
        .and_then(|(prefix, _)| prefix.strip_prefix('k'))
        .map(str::len)
        .unwrap_or(3)
}

pub(crate) fn is_translation_slot_id(value: &str) -> bool {
    let Some((prefix, checksum)) = value.split_once('-') else {
        return false;
    };
    prefix.len() >= 2
        && prefix.starts_with('k')
        && prefix[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
        && checksum.len() == 2
        && checksum
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn base36(mut value: u64) -> String {
    let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut output = Vec::new();
    while value > 0 {
        output.push(alphabet[(value % 36) as usize] as char);
        value /= 36;
    }
    output.iter().rev().collect()
}
