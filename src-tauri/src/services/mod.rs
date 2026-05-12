use crate::dto::*;
use crate::fs_ops::{
    copy_dir_all, move_path_or_copy, nearest_existing_parent, open_path_in_system,
    remove_path_if_exists, replace_dir_or_file,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use sts2_mod_manager::app::App;
use sts2_mod_manager::config::AppConfig;
use sts2_mod_manager::discovery::scan_mod_directory;
use sts2_mod_manager::domain::{ModFingerprint, ModKind, ModRecord, ModSource, ScanSummary};
use sts2_mod_manager::json_translation::{
    JsonImportReport, JsonSheetReport, JsonTranslationEntry, JsonTranslationSheet,
    JsonTranslationStatus, JsonValidationReport, apply_sheet, apply_sheet_to_target_language,
    compact_source_translation_map_with_keys, compact_validation_issue_translation_map_with_keys,
    create_or_update_sheet, flatten_hardcoded_values, import_translations,
    is_hardcoded_source_file, read_sheet, target_language_output_path, translation_slot_entries,
    validate_sheet, validate_translation_sheet, write_sheet,
};
use sts2_mod_manager::launcher::{LaunchReport, LaunchStatus};
use sts2_mod_manager::preset::{Preset, PresetApplyReport};
use sts2_mod_manager::process::hidden_command;
use sts2_mod_manager::save_backup::{self, SaveBackupEntry};
use sts2_mod_manager::state::{
    ModStateEntry, desired_active_mod_keys, mod_record_state_key, read_mod_state_index,
    write_desired_active_mod_keys,
};
use sts2_mod_manager::translation::{TranslationWorkspace, scan_translation_candidates};
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
include!("pck.rs");
include!("tests.rs");
