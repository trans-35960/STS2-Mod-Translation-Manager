mod apply;
mod hardcoded;
mod import;
mod language_path;
mod sheet;
mod slots;
mod source_json;
mod types;

pub use apply::{apply_sheet, apply_sheet_to_target_language, target_language_output_path};
pub use hardcoded::{
    flatten_hardcoded_values, hardcoded_capacity_bytes, is_hardcoded_entry_key,
    is_hardcoded_source_file, utf16le_byte_len,
};
pub use import::import_translations;
pub use sheet::{
    create_or_update_sheet, read_sheet, validate_sheet, validate_translation_sheet, write_sheet,
};
pub use slots::{
    compact_source_translation_map, compact_source_translation_map_with_keys,
    compact_validation_issue_translation_map, compact_validation_issue_translation_map_with_keys,
    translation_short_id, translation_slot_entries,
};
pub use types::{
    JsonApplyReport, JsonImportReport, JsonSheetReport, JsonTranslationEntry, JsonTranslationSheet,
    JsonTranslationStatus, JsonValidationIssue, JsonValidationReport,
};

#[cfg(test)]
mod tests;
