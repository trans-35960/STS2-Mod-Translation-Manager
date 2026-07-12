use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct DashboardDto {
    pub(crate) paths: PathsDto,
    pub(crate) settings: UiSettingsDto,
    pub(crate) stats: StatsDto,
    pub(crate) setup_issues: Vec<SetupIssueDto>,
    pub(crate) diagnostics: Vec<TroubleshootDiagnosticDto>,
    pub(crate) mods: Vec<ModRowDto>,
    pub(crate) presets: Vec<PresetDto>,
    pub(crate) translations: Vec<TranslationWorkspaceDto>,
    pub(crate) deleted_mods: Vec<DeletedModDto>,
    pub(crate) save_backups: Vec<SaveBackupDto>,
    pub(crate) cache_usage: CacheUsageDto,
    pub(crate) tools: Vec<ToolDto>,
    pub(crate) launch: LaunchStatusDto,
}

#[derive(Debug, Serialize)]
pub(crate) struct PathsDto {
    pub(crate) workspace: String,
    pub(crate) game: String,
    pub(crate) game_mods: String,
    pub(crate) save_dir: String,
    pub(crate) save_backup: String,
    pub(crate) disabled: String,
    pub(crate) presets: String,
    pub(crate) translation_work: String,
    pub(crate) state: String,
    pub(crate) vendor: String,
    pub(crate) external_manager_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiSettingsDto {
    pub(crate) translation_work_dir: String,
    pub(crate) target_language: String,
    pub(crate) game_exe_path: String,
    pub(crate) game_log_path: String,
    pub(crate) save_dir: String,
    pub(crate) save_backup_dir: String,
    pub(crate) save_backup_retention_days: u32,
    pub(crate) save_backup_max_entries: u32,
    pub(crate) deleted_retention_days: u32,
    pub(crate) mod_view_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSettingsRequest {
    pub(crate) translation_work_dir: String,
    pub(crate) target_language: String,
    pub(crate) game_exe_path: String,
    pub(crate) game_log_path: String,
    pub(crate) save_dir: String,
    pub(crate) save_backup_dir: String,
    pub(crate) save_backup_retention_days: u32,
    pub(crate) save_backup_max_entries: u32,
    pub(crate) deleted_retention_days: u32,
    pub(crate) mod_view_mode: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct StatsDto {
    pub(crate) active_mods: usize,
    pub(crate) inactive_mods: usize,
    pub(crate) disabled_mods: usize,
    pub(crate) external_mods: usize,
    pub(crate) presets: usize,
    pub(crate) translations: usize,
    pub(crate) detected_changes: usize,
    pub(crate) vanilla_safe: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetupIssueDto {
    pub(crate) field: String,
    pub(crate) message: String,
    pub(crate) blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TroubleshootDiagnosticDto {
    pub(crate) id: String,
    pub(crate) category: String,
    pub(crate) severity: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) action_label: String,
    pub(crate) related_path: String,
    pub(crate) mod_key: Option<String>,
    pub(crate) can_auto_fix: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModRowDto {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) manifest_id: Option<String>,
    pub(crate) group_name: Option<String>,
    pub(crate) active: bool,
    pub(crate) managed: bool,
    pub(crate) external: bool,
    pub(crate) source_label: String,
    pub(crate) kind: String,
    pub(crate) version_hint: Option<String>,
    pub(crate) bytes: u64,
    pub(crate) modified_epoch: Option<u64>,
    pub(crate) registered_epoch: Option<u64>,
    pub(crate) updated_epoch: Option<u64>,
    pub(crate) path: String,
    pub(crate) download_state: Option<String>,
    pub(crate) update_state: String,
    pub(crate) change_reasons: Vec<String>,
    pub(crate) translation_state: String,
    pub(crate) translation_applied: bool,
    pub(crate) translation_applied_epoch: Option<u64>,
    pub(crate) translation_patch_count: usize,
    pub(crate) translation_patch_active_count: usize,
    pub(crate) translation_patch_names: Vec<String>,
    pub(crate) needs_recheck: bool,
    pub(crate) translation_review_required: bool,
    pub(crate) safety_warnings: Vec<String>,
    pub(crate) extraction_hint: String,
    pub(crate) extraction_source_path: String,
    pub(crate) extraction_target: String,
    pub(crate) is_translation_patch: bool,
    pub(crate) translation_target_id: Option<String>,
    pub(crate) translation_target_key: Option<String>,
    pub(crate) translation_target_name: Option<String>,
    pub(crate) translation_target_version: Option<String>,
    pub(crate) dependencies: Vec<ModDependencyDto>,
    pub(crate) language_preview: Vec<LanguagePreviewDto>,
    pub(crate) extraction_tree: Vec<ExtractionTreeNodeDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ModDeleteDto {
    pub(crate) key: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModDependencyDto {
    pub(crate) id: String,
    pub(crate) key: Option<String>,
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) available: bool,
    pub(crate) version_required: Option<String>,
    pub(crate) version_current: Option<String>,
    pub(crate) version_matches: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct LanguagePreviewDto {
    pub(crate) code: String,
    pub(crate) label: String,
    pub(crate) files: usize,
    pub(crate) keys: usize,
    pub(crate) sample_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExtractionTreeNodeDto {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) source_path: String,
    pub(crate) children: Vec<ExtractionTreeNodeDto>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PresetDto {
    pub(crate) name: String,
    pub(crate) mod_count: usize,
    pub(crate) mods: Vec<PresetModDto>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PresetModDto {
    pub(crate) key: String,
    pub(crate) version_hint: Option<String>,
    pub(crate) bytes: Option<u64>,
    pub(crate) modified_epoch: Option<u64>,
    pub(crate) file_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TranslationWorkspaceDto {
    pub(crate) mod_key: String,
    pub(crate) version_id: String,
    pub(crate) review_required: bool,
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolDto {
    pub(crate) name: String,
    pub(crate) available: bool,
    pub(crate) purpose: String,
    pub(crate) expected_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct LaunchStatusDto {
    pub(crate) ready: bool,
    pub(crate) game_exe: Option<String>,
    pub(crate) steam_exe: Option<String>,
    pub(crate) target_label: String,
    pub(crate) running: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct GameLogDto {
    pub(crate) path: String,
    pub(crate) exists: bool,
    pub(crate) modified_epoch: Option<u64>,
    pub(crate) bytes: u64,
    pub(crate) lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeletedModDto {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) original_path: String,
    pub(crate) backup_path: String,
    pub(crate) deleted_epoch: u64,
    pub(crate) expires_epoch: Option<u64>,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SaveBackupDto {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) kind_label: String,
    pub(crate) created_epoch: u64,
    pub(crate) path: String,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CacheUsageDto {
    pub(crate) bytes: u64,
    pub(crate) files: usize,
    pub(crate) dirs: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActionDto {
    pub(crate) message: String,
    pub(crate) dashboard: DashboardDto,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModToggleDto {
    pub(crate) key: String,
    pub(crate) active: bool,
    pub(crate) force: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DroppedModPreviewDto {
    pub(crate) path: String,
    pub(crate) display_path: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) version_hint: Option<String>,
    pub(crate) bytes: u64,
    pub(crate) modified_epoch: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TranslationPreparationProgressDto {
    pub(crate) phase: String,
    pub(crate) detail: String,
    pub(crate) step: usize,
    pub(crate) total_steps: usize,
    pub(crate) source_bytes: u64,
    pub(crate) cache_hit: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DroppedModDecisionDto {
    pub(crate) path: String,
    pub(crate) mode: String,
    pub(crate) replace_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PresetApplyDto {
    pub(crate) disabled: usize,
    pub(crate) enabled: usize,
    pub(crate) missing: Vec<String>,
    pub(crate) version_warnings: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct JsonEntryDto {
    pub(crate) key: String,
    pub(crate) slot_id: Option<String>,
    #[serde(default)]
    pub(crate) previous_source_value: Option<String>,
    pub(crate) source_value: String,
    pub(crate) translated_value: String,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct JsonSheetDto {
    pub(crate) source_path: String,
    pub(crate) target_language: String,
    pub(crate) updated_epoch: u64,
    pub(crate) entries: Vec<JsonEntryDto>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonSheetActionDto {
    pub(crate) message: String,
    pub(crate) report: JsonSheetReportDto,
    pub(crate) sheet: JsonSheetDto,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonSheetReportDto {
    pub(crate) sheet_path: String,
    pub(crate) entries: usize,
    pub(crate) new_entries: usize,
    pub(crate) updated_entries: usize,
    pub(crate) missing_entries: usize,
    pub(crate) removed_entries: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonValidationDto {
    pub(crate) valid: bool,
    pub(crate) total_entries: usize,
    pub(crate) missing_entries: Vec<String>,
    pub(crate) updated_entries: Vec<String>,
    pub(crate) removed_entries: Vec<String>,
    pub(crate) format_issues: Vec<JsonValidationIssueDto>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonValidationIssueDto {
    pub(crate) key: String,
    pub(crate) kind: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonApplyDto {
    pub(crate) output_path: String,
    pub(crate) applied_entries: usize,
    pub(crate) language_output_path: String,
    pub(crate) packed_pck_path: String,
    pub(crate) installed_mod_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TranslationPatchExportDto {
    pub(crate) output_dir: String,
    pub(crate) manifest_path: String,
    pub(crate) pck_path: String,
    pub(crate) package_id: String,
    pub(crate) dependency_id: String,
    pub(crate) dependency_version: String,
    pub(crate) languages: Vec<String>,
    pub(crate) files: usize,
    pub(crate) applied_entries: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonImportDto {
    pub(crate) input_path: String,
    pub(crate) matched_entries: usize,
    pub(crate) unmatched_entries: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonCsvExportDto {
    pub(crate) output_path: String,
    pub(crate) rows: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ShortJsonExportDto {
    pub(crate) output_path: String,
    pub(crate) rows: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct NodeTranslationDto {
    pub(crate) message: String,
    pub(crate) source_path: String,
    pub(crate) existing_sheet_path: String,
    pub(crate) output_sheet_path: String,
    pub(crate) translated_output_path: String,
    pub(crate) copied_files: usize,
    pub(crate) mod_key: String,
    pub(crate) mod_path: String,
    pub(crate) mod_name: String,
    pub(crate) mod_version: String,
    pub(crate) mod_author: String,
    pub(crate) mod_description: String,
    pub(crate) available_languages: Vec<LanguagePreviewDto>,
    pub(crate) can_export_patch_mod: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct LanguageCompareValueDto {
    pub(crate) key: String,
    pub(crate) value: String,
}
