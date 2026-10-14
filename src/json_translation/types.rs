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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_source_value: Option<String>,
    pub source_value: String,
    pub translated_value: String,
    pub status: JsonTranslationStatus,
    /// Warnings the user checked and marked as fine (false positives).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledged_issues: Vec<AcknowledgedIssue>,
}

impl JsonTranslationEntry {
    /// Whether a warning of `kind` was marked as fine for the current source
    /// and translation. Editing either brings the warning back.
    pub fn acknowledges(&self, kind: &str) -> bool {
        self.acknowledged_issues.iter().any(|issue| {
            issue.kind == kind
                && issue.source_value == self.source_value
                && issue.translated_value == self.translated_value
        })
    }
}

/// A warning the user marked as fine, for the texts it was raised on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcknowledgedIssue {
    pub kind: String,
    pub source_value: String,
    pub translated_value: String,
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
    /// Ids from the input that were skipped (empty, unknown, duplicate).
    pub skipped: Vec<String>,
}
