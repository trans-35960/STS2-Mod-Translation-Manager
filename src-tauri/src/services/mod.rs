use crate::dto::*;
use crate::fs_ops::{
    copy_dir_all, move_path_or_copy, nearest_existing_parent, open_path_in_system,
    remove_path_if_exists, replace_dir_or_file,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use sts2_mod_manager::ai_prompt::{
    DEFAULT_PROMPT_MAX_CHARS, PromptOptions, TermToTranslate, build_prompt_parts,
    build_term_prompt_parts, read_project_settings, write_project_settings,
};
use sts2_mod_manager::app::App;
use sts2_mod_manager::config::AppConfig;
use sts2_mod_manager::discovery::scan_mod_directory;
use sts2_mod_manager::domain::{ModFingerprint, ModKind, ModRecord, ModSource, ScanSummary};
use sts2_mod_manager::glossary::{
    GLOSSARY_ISSUE_KIND, GlossaryTerm, JOSA_ISSUE_KIND, glossary_issues, is_korean_language,
    korean_josa_issues, load_game_glossary, merge_glossary_terms, read_user_glossary,
    sheet_glossary_terms, sheet_source_language as glossary_source_language, write_user_glossary,
};
use sts2_mod_manager::json_translation::{
    JsonImportReport, JsonSheetReport, JsonTranslationEntry, JsonTranslationSheet,
    JsonTranslationStatus, JsonValidationReport, apply_sheet, apply_sheet_to_target_language,
    compact_source_translation_map_with_keys, compact_validation_issue_translation_map_with_keys,
    create_or_update_sheet, import_translations, is_hardcoded_source_file, number_mismatch_issues,
    read_sheet, target_language_output_path, translation_slot_entries, validate_sheet,
    validate_translation_sheet, write_sheet,
};
use sts2_mod_manager::launcher::{LaunchReport, LaunchStatus};
use sts2_mod_manager::path_guard::{ensure_existing_path_in_roots, ensure_path_in_roots};
use sts2_mod_manager::pck_reader::extract_pck_files;
use sts2_mod_manager::preset::{Preset, PresetApplyReport};
use sts2_mod_manager::process::{hidden_command, powershell_expand_archive};
use sts2_mod_manager::save_backup::{self, SaveBackupEntry};
use sts2_mod_manager::state::{
    ModStateEntry, desired_active_mod_keys, mod_record_state_key, read_mod_state_index,
    version_change, write_desired_active_mod_keys,
};
use sts2_mod_manager::term_candidates::extract_term_candidates as term_candidates;
use sts2_mod_manager::translation::{
    TranslationWorkspace, has_translatable_hardcoded_values, scan_translation_candidates,
};
use sts2_mod_manager::vendor_tools::VendorTool;

include!("common.rs");
include!("settings.rs");
include!("logs_diagnostics.rs");
include!("translation_preview/manifest.rs");
include!("translation_preview/resource_paths.rs");
include!("translation_preview/extraction_cache.rs");
include!("translation_preview/extraction_tree.rs");
include!("translation_preview/language_preview.rs");
include!("dashboard/setup_status.rs");
include!("dashboard/activation.rs");
include!("dashboard/dto_maps.rs");
include!("dashboard/mod_rows.rs");
include!("dashboard.rs");
include!("deleted_mods.rs");
include!("cache_cleanup.rs");
include!("translation_preview.rs");
include!("json_sheet.rs");
include!("json_sheet/dto.rs");
include!("pck.rs");
include!("glossary.rs");
include!("tests.rs");
