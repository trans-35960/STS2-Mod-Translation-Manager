use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonTranslationSheet {
    pub source_path: String,
    pub target_language: String,
    pub updated_epoch: u64,
    pub entries: Vec<JsonTranslationEntry>,
}

impl JsonTranslationSheet {
    pub(crate) fn source_path_is_dir(&self) -> bool {
        PathBuf::from(&self.source_path).is_dir()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonTranslationEntry {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_id: Option<String>,
    pub source_value: String,
    pub translated_value: String,
    pub status: JsonTranslationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonTranslationStatus {
    New,
    Ready,
    Updated,
    Missing,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSheetReport {
    pub sheet_path: PathBuf,
    pub entries: usize,
    pub new_entries: usize,
    pub updated_entries: usize,
    pub missing_entries: usize,
    pub removed_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonValidationReport {
    pub valid: bool,
    pub total_entries: usize,
    pub missing_entries: Vec<String>,
    pub updated_entries: Vec<String>,
    pub removed_entries: Vec<String>,
    pub format_issues: Vec<JsonValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonValidationIssue {
    pub key: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonApplyReport {
    pub output_path: PathBuf,
    pub applied_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonImportReport {
    pub input_path: PathBuf,
    pub matched_entries: usize,
    pub unmatched_entries: usize,
}
