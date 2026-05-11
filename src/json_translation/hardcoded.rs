use super::types::{JsonApplyReport, JsonTranslationSheet, JsonTranslationStatus};
use crate::error::{AppError, AppResult};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const HARDCODED_KEY_PREFIX: &str = "dll://";

#[derive(Debug, Clone, PartialEq, Eq)]
struct HardcodedString {
    offset: usize,
    byte_len: usize,
    value: String,
}

pub fn is_hardcoded_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "dll" | "exe"))
        .unwrap_or(false)
}

pub fn flatten_hardcoded_values(path: &Path) -> AppResult<BTreeMap<String, String>> {
    let bytes = fs::read(path).map_err(|source| AppError::io(path, source))?;
    let strings = scan_dotnet_user_strings(&bytes).unwrap_or_else(|| scan_utf16_strings(&bytes));
    Ok(strings
        .into_iter()
        .map(|entry| {
            (
                hardcoded_entry_key(entry.offset, entry.byte_len),
                entry.value,
            )
        })
        .collect())
}

pub fn apply_hardcoded_sheet(
    sheet: &JsonTranslationSheet,
    source_path: &Path,
    output_path: &Path,
) -> AppResult<JsonApplyReport> {
    let mut bytes = fs::read(source_path).map_err(|source| AppError::io(source_path, source))?;
    let mut applied_entries = 0usize;

    for entry in &sheet.entries {
        if entry.status == JsonTranslationStatus::Removed
            || entry.source_value.trim().is_empty()
            || entry.translated_value.is_empty()
        {
            continue;
        }
        let Some((offset, capacity)) = hardcoded_key_range(&entry.key) else {
            continue;
        };
        let translated_bytes = utf16le_bytes(&entry.translated_value);
        if translated_bytes.len() > capacity {
            return Err(AppError::InvalidCommand(format!(
                "DLL 문자열이 원문보다 깁니다: {} ({} > {} bytes)",
                entry.key,
                translated_bytes.len(),
                capacity
            )));
        }
        if offset
            .checked_add(capacity)
            .map(|end| end > bytes.len())
            .unwrap_or(true)
        {
            return Err(AppError::InvalidCommand(format!(
                "DLL 문자열 위치가 파일 범위를 벗어났습니다: {}",
                entry.key
            )));
        }

        for chunk in bytes[offset..offset + capacity].chunks_exact_mut(2) {
            chunk[0] = b' ';
            chunk[1] = 0;
        }
        bytes[offset..offset + translated_bytes.len()].copy_from_slice(&translated_bytes);
        applied_entries += 1;
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    fs::write(output_path, bytes).map_err(|source| AppError::io(output_path, source))?;
    Ok(JsonApplyReport {
        output_path: output_path.to_path_buf(),
        applied_entries,
    })
}

pub fn is_hardcoded_entry_key(key: &str) -> bool {
    key.starts_with(HARDCODED_KEY_PREFIX)
}

pub fn hardcoded_capacity_bytes(key: &str) -> Option<usize> {
    hardcoded_key_range(key).map(|(_, capacity)| capacity)
}

pub fn utf16le_byte_len(value: &str) -> usize {
    value.encode_utf16().count() * 2
}

fn hardcoded_entry_key(offset: usize, byte_len: usize) -> String {
    format!("{HARDCODED_KEY_PREFIX}{offset:x}:{byte_len}")
}

fn hardcoded_key_range(key: &str) -> Option<(usize, usize)> {
    let rest = key.strip_prefix(HARDCODED_KEY_PREFIX)?;
    let (offset, byte_len) = rest.split_once(':')?;
    Some((
        usize::from_str_radix(offset, 16).ok()?,
        byte_len.parse().ok()?,
    ))
}

fn scan_dotnet_user_strings(bytes: &[u8]) -> Option<Vec<HardcodedString>> {
    let metadata_offset = dotnet_metadata_offset(bytes)?;
    let streams = metadata_streams(bytes, metadata_offset)?;
    let user_strings = streams.into_iter().find(|stream| stream.name == "#US")?;
    let heap_start = metadata_offset.checked_add(user_strings.offset)?;
    let heap_end = heap_start.checked_add(user_strings.size)?;
    if heap_end > bytes.len() {
        return None;
    }
    let mut strings = scan_dotnet_user_string_heap(&bytes[heap_start..heap_end], heap_start);
    strings.sort_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
    });
    strings.dedup_by(|left, right| left.offset == right.offset && left.byte_len == right.byte_len);
    Some(strings)
}

fn dotnet_metadata_offset(bytes: &[u8]) -> Option<usize> {
    if bytes.get(0..2)? != b"MZ" {
        return None;
    }
    let pe_offset = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe_offset..pe_offset.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let coff_offset = pe_offset.checked_add(4)?;
    let section_count = read_u16(bytes, coff_offset.checked_add(2)?)? as usize;
    let optional_header_size = read_u16(bytes, coff_offset.checked_add(16)?)? as usize;
    let optional_offset = coff_offset.checked_add(20)?;
    let optional_magic = read_u16(bytes, optional_offset)?;
    let data_directories_offset = match optional_magic {
        0x10b => optional_offset.checked_add(96)?,
        0x20b => optional_offset.checked_add(112)?,
        _ => return None,
    };
    let clr_directory_offset = data_directories_offset.checked_add(14 * 8)?;
    let clr_rva = read_u32(bytes, clr_directory_offset)?;
    if clr_rva == 0 {
        return None;
    }
    let sections_offset = optional_offset.checked_add(optional_header_size)?;
    let clr_offset = rva_to_offset(bytes, sections_offset, section_count, clr_rva)?;
    let metadata_rva = read_u32(bytes, clr_offset.checked_add(8)?)?;
    if metadata_rva == 0 {
        return None;
    }
    rva_to_offset(bytes, sections_offset, section_count, metadata_rva)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataStream {
    name: String,
    offset: usize,
    size: usize,
}

fn metadata_streams(bytes: &[u8], metadata_offset: usize) -> Option<Vec<MetadataStream>> {
    if read_u32(bytes, metadata_offset)? != 0x424a_5342 {
        return None;
    }
    let version_len = read_u32(bytes, metadata_offset.checked_add(12)?)? as usize;
    let version_end = metadata_offset.checked_add(16)?.checked_add(version_len)?;
    let stream_count_offset = align4(version_end)?.checked_add(2)?;
    let stream_count = read_u16(bytes, stream_count_offset)? as usize;
    let mut cursor = stream_count_offset.checked_add(2)?;
    let mut streams = Vec::new();

    for _ in 0..stream_count {
        let offset = read_u32(bytes, cursor)? as usize;
        let size = read_u32(bytes, cursor.checked_add(4)?)? as usize;
        cursor = cursor.checked_add(8)?;
        let name_start = cursor;
        while cursor < bytes.len() && bytes[cursor] != 0 {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return None;
        }
        let name = std::str::from_utf8(bytes.get(name_start..cursor)?)
            .ok()?
            .to_string();
        cursor = align4(cursor.checked_add(1)?)?;
        streams.push(MetadataStream { name, offset, size });
    }

    Some(streams)
}

fn scan_dotnet_user_string_heap(heap: &[u8], heap_file_offset: usize) -> Vec<HardcodedString> {
    let mut strings = Vec::new();
    let mut cursor = 1usize;

    while cursor < heap.len() {
        let Some((blob_len, prefix_len)) = read_compressed_uint(heap, cursor) else {
            break;
        };
        cursor += prefix_len;
        let blob_len = blob_len as usize;
        if blob_len == 0 {
            continue;
        }
        let Some(blob_end) = cursor.checked_add(blob_len) else {
            break;
        };
        if blob_end > heap.len() {
            break;
        }
        let text_len = blob_len.saturating_sub(1);
        if text_len > 0 && text_len % 2 == 0 {
            let units = heap[cursor..cursor + text_len]
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            let value = String::from_utf16_lossy(&units);
            if is_dotnet_user_text(&value) {
                strings.push(HardcodedString {
                    offset: heap_file_offset + cursor,
                    byte_len: text_len,
                    value,
                });
            }
        }
        cursor = blob_end;
    }

    strings
}

fn scan_utf16_strings(bytes: &[u8]) -> Vec<HardcodedString> {
    let mut strings = Vec::new();
    for alignment in 0..2 {
        let mut index = alignment;
        while index + 1 < bytes.len() {
            let start = index;
            let mut units = Vec::new();
            while index + 1 < bytes.len() {
                let unit = u16::from_le_bytes([bytes[index], bytes[index + 1]]);
                let Some(character) = char::from_u32(unit as u32) else {
                    break;
                };
                if !is_candidate_character(character) {
                    break;
                }
                units.push(unit);
                index += 2;
            }

            if units.len() >= 4 {
                let value = String::from_utf16_lossy(&units);
                if is_candidate_text(&value) {
                    strings.push(HardcodedString {
                        offset: start,
                        byte_len: units.len() * 2,
                        value,
                    });
                }
            }
            index = index.max(start + 2);
        }
    }
    strings.sort_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
    });
    strings.dedup_by(|left, right| left.offset == right.offset && left.byte_len == right.byte_len);
    strings
}

fn is_candidate_character(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\t')
        || character.is_ascii_graphic()
        || character == ' '
        || ('\u{ac00}'..='\u{d7af}').contains(&character)
        || ('\u{3040}'..='\u{30ff}').contains(&character)
        || ('\u{4e00}'..='\u{9fff}').contains(&character)
}

fn is_candidate_text(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 4 {
        return false;
    }
    if is_probably_mojibake_utf16(trimmed) {
        return false;
    }
    let has_letter = trimmed.chars().any(|character| {
        character.is_ascii_alphabetic()
            || ('\u{ac00}'..='\u{d7af}').contains(&character)
            || ('\u{3040}'..='\u{30ff}').contains(&character)
            || ('\u{4e00}'..='\u{9fff}').contains(&character)
    });
    let has_text_separator = trimmed.chars().any(|character| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | ',' | ':' | ';' | '!' | '?' | '[' | ']' | '(' | ')' | '/' | '-'
            )
    });
    has_letter && (has_text_separator || trimmed.chars().count() >= 10)
}

fn is_dotnet_user_text(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.chars().count() < 2 || trimmed.contains('\u{fffd}') {
        return false;
    }
    if is_probably_mojibake_utf16(trimmed) {
        return false;
    }
    if is_likely_technical_identifier(trimmed) {
        return false;
    }
    let has_visible = trimmed.chars().any(|character| !character.is_whitespace());
    let has_text = trimmed
        .chars()
        .any(|character| character.is_alphanumeric() || !character.is_ascii_punctuation());
    has_visible && has_text
}

fn is_likely_technical_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if matches!(
        lower.rsplit('.').next(),
        Some("dll" | "exe" | "json" | "pck" | "png" | "import" | "xml" | "txt")
    ) {
        return true;
    }
    value.contains('.')
        && !value.chars().any(char::is_whitespace)
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn is_probably_mojibake_utf16(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < 3 {
        return false;
    }
    let ascii_packed = chars
        .iter()
        .filter(|character| {
            let codepoint = **character as u32;
            codepoint <= 0xffff
                && is_printable_ascii_byte((codepoint & 0xff) as u8)
                && is_printable_ascii_byte((codepoint >> 8) as u8)
        })
        .count();
    let ascii_shifted = chars
        .iter()
        .filter(|character| {
            let codepoint = **character as u32;
            codepoint <= 0xffff
                && (codepoint & 0xff) == 0
                && is_printable_ascii_byte((codepoint >> 8) as u8)
        })
        .count();
    ascii_packed * 2 >= chars.len() || ascii_shifted * 2 >= chars.len()
}

fn is_printable_ascii_byte(value: u8) -> bool {
    matches!(value, 0x20..=0x7e)
}

fn utf16le_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
}

fn rva_to_offset(
    bytes: &[u8],
    sections_offset: usize,
    section_count: usize,
    rva: u32,
) -> Option<usize> {
    for index in 0..section_count {
        let section_offset = sections_offset.checked_add(index.checked_mul(40)?)?;
        let virtual_size = read_u32(bytes, section_offset.checked_add(8)?)?;
        let virtual_address = read_u32(bytes, section_offset.checked_add(12)?)?;
        let raw_size = read_u32(bytes, section_offset.checked_add(16)?)?;
        let raw_pointer = read_u32(bytes, section_offset.checked_add(20)?)?;
        let span = virtual_size.max(raw_size);
        if rva >= virtual_address && rva < virtual_address.saturating_add(span) {
            return Some(raw_pointer.checked_add(rva.checked_sub(virtual_address)?)? as usize);
        }
    }
    None
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn read_compressed_uint(bytes: &[u8], offset: usize) -> Option<(u32, usize)> {
    let first = *bytes.get(offset)?;
    if first & 0x80 == 0 {
        return Some((first as u32, 1));
    }
    if first & 0xc0 == 0x80 {
        let second = *bytes.get(offset.checked_add(1)?)?;
        return Some(((((first & 0x3f) as u32) << 8) | second as u32, 2));
    }
    if first & 0xe0 == 0xc0 {
        let second = *bytes.get(offset.checked_add(1)?)?;
        let third = *bytes.get(offset.checked_add(2)?)?;
        let fourth = *bytes.get(offset.checked_add(3)?)?;
        return Some((
            (((first & 0x1f) as u32) << 24)
                | ((second as u32) << 16)
                | ((third as u32) << 8)
                | fourth as u32,
            4,
        ));
    }
    None
}

fn align4(value: usize) -> Option<usize> {
    value.checked_add(3).map(|value| value & !3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extracts_utf16_hardcoded_strings() {
        let path = temp_path("hardcoded-source.dll");
        fs::write(
            &path,
            [
                b"prefix".as_slice(),
                &[0, 0xff],
                &utf16le_bytes("Current ascension state unavailable."),
                &[0, 0xff],
                b"suffix".as_slice(),
            ]
            .concat(),
        )
        .expect("write dll");

        let values = flatten_hardcoded_values(&path).expect("flatten hardcoded");

        assert!(
            values
                .values()
                .any(|value| value == "Current ascension state unavailable.")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn applies_fixed_width_hardcoded_translation() {
        let source = temp_path("hardcoded-apply-source.dll");
        let output = temp_path("hardcoded-apply-output.dll");
        fs::write(&source, utf16le_bytes("Applied value")).expect("write source");
        let values = flatten_hardcoded_values(&source).expect("flatten hardcoded");
        let key = values.keys().next().expect("key").clone();
        let sheet = JsonTranslationSheet {
            source_path: source.to_string_lossy().to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: vec![super::super::types::JsonTranslationEntry {
                key,
                slot_id: None,
                source_value: "Applied value".to_string(),
                translated_value: "적용됨".to_string(),
                status: JsonTranslationStatus::Ready,
            }],
        };

        let report = apply_hardcoded_sheet(&sheet, &source, &output).expect("apply");

        assert_eq!(report.applied_entries, 1);
        let output_bytes = fs::read(&output).expect("read output");
        let patched_units = output_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        let patched_text = String::from_utf16_lossy(&patched_units);
        assert_eq!(patched_text.trim_end(), "적용됨");
        let _ = fs::remove_file(source);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn extracts_dotnet_user_string_heap_entries() {
        let mut heap = vec![0];
        let text = utf16le_bytes("Unified Save Path");
        heap.push((text.len() + 1) as u8);
        heap.extend(text);
        heap.push(0);

        let values = scan_dotnet_user_string_heap(&heap, 100);

        assert_eq!(
            values,
            vec![HardcodedString {
                offset: 102,
                byte_len: 34,
                value: "Unified Save Path".to_string(),
            }]
        );
    }

    #[test]
    fn rejects_ascii_bytes_misread_as_utf16() {
        assert!(is_probably_mojibake_utf16("吀爀愀渀猀氀愀琀椀漀渀"));
        assert!(is_probably_mojibake_utf16("剳湵楮杮潍摤摥慐捴"));
        assert!(!is_probably_mojibake_utf16("Unified Save Path"));
        assert!(!is_probably_mojibake_utf16("승천 단계가 잠겨 있습니다."));
    }

    #[test]
    fn skips_dotnet_technical_identifiers() {
        assert!(!is_dotnet_user_text("com.unifiedsavepath.sts2"));
        assert!(!is_dotnet_user_text("UnifiedSavePath.dll"));
        assert!(is_dotnet_user_text("Current ascension state unavailable."));
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "sts2-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }
}
