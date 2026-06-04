use crate::dto::*;

async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("백그라운드 작업 실패: {error}"))?
}

#[tauri::command]
pub(crate) fn load_dashboard() -> Result<DashboardDto, String> {
    crate::services::load_dashboard()
}

#[tauri::command]
pub(crate) fn save_settings(request: SaveSettingsRequest) -> Result<ActionDto, String> {
    crate::services::save_settings(request)
}

#[tauri::command]
pub(crate) fn save_mod_view_mode(mod_view_mode: String) -> Result<(), String> {
    crate::services::save_mod_view_mode(mod_view_mode)
}

#[tauri::command]
pub(crate) fn read_game_logs() -> Result<Vec<GameLogDto>, String> {
    crate::services::read_game_logs()
}

#[tauri::command]
pub(crate) fn scan_updates() -> Result<ActionDto, String> {
    crate::services::scan_updates()
}

#[tauri::command]
pub(crate) fn repair_mod_installations() -> Result<ActionDto, String> {
    crate::services::repair_mod_installations()
}

#[tauri::command]
pub(crate) fn open_path(path: String) -> Result<(), String> {
    crate::services::open_path(path)
}

#[tauri::command]
pub(crate) fn preview_dropped_mods(
    paths: Vec<String>,
) -> Result<Vec<DroppedModPreviewDto>, String> {
    crate::services::preview_dropped_mods(paths)
}

#[tauri::command]
pub(crate) fn import_dropped_mod(
    path: String,
    replace_path: Option<String>,
) -> Result<ActionDto, String> {
    crate::services::import_dropped_mod(path, replace_path)
}

#[tauri::command]
pub(crate) fn import_dropped_mods(
    decisions: Vec<DroppedModDecisionDto>,
) -> Result<ActionDto, String> {
    crate::services::import_dropped_mods(decisions)
}

#[tauri::command]
pub(crate) fn toggle_mod(
    key: String,
    active: bool,
    force: Option<bool>,
) -> Result<ActionDto, String> {
    crate::services::toggle_mod(key, active, force.unwrap_or(false))
}

#[tauri::command]
pub(crate) fn toggle_mods(changes: Vec<ModToggleDto>) -> Result<ActionDto, String> {
    crate::services::toggle_mods(changes)
}

#[tauri::command]
pub(crate) fn delete_mod(key: String, path: String) -> Result<ActionDto, String> {
    crate::services::delete_mod(key, path)
}

#[tauri::command]
pub(crate) fn delete_mods(items: Vec<ModDeleteDto>) -> Result<ActionDto, String> {
    crate::services::delete_mods(items)
}

#[tauri::command]
pub(crate) fn restore_deleted_mod(id: String) -> Result<ActionDto, String> {
    crate::services::restore_deleted_mod(id)
}

#[tauri::command]
pub(crate) fn empty_deleted_mods() -> Result<ActionDto, String> {
    crate::services::empty_deleted_mods()
}

#[tauri::command]
pub(crate) fn cleanup_orphan_caches() -> Result<ActionDto, String> {
    crate::services::cleanup_orphan_caches()
}

#[tauri::command]
pub(crate) fn cleanup_dropped_mod_preview_cache() -> Result<(), String> {
    crate::services::cleanup_dropped_mod_preview_cache()
}

#[tauri::command]
pub(crate) fn extract_translation(
    key: String,
    output_dir: Option<String>,
    resource_path: Option<String>,
    force: Option<bool>,
) -> Result<ActionDto, String> {
    crate::services::extract_translation(key, output_dir, resource_path, force.unwrap_or(false))
}

#[tauri::command]
pub(crate) fn clear_translation_extract_cache(key: String) -> Result<ActionDto, String> {
    crate::services::clear_translation_extract_cache(key)
}

#[tauri::command]
pub(crate) async fn prepare_translation_node(
    key: String,
    resource_path: String,
    output_dir: Option<String>,
    force: Option<bool>,
) -> Result<NodeTranslationDto, String> {
    run_blocking(move || {
        crate::services::prepare_translation_node(
            key,
            resource_path,
            output_dir,
            force.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub(crate) fn save_preset(name: String) -> Result<ActionDto, String> {
    crate::services::save_preset(name)
}

#[tauri::command]
pub(crate) fn apply_preset(name: String) -> Result<ActionDto, String> {
    crate::services::apply_preset(name)
}

#[tauri::command]
pub(crate) fn export_preset(name: String, archive_path: String) -> Result<ActionDto, String> {
    crate::services::export_preset(name, archive_path)
}

#[tauri::command]
pub(crate) fn import_preset_archive(archive_path: String) -> Result<ActionDto, String> {
    crate::services::import_preset_archive(archive_path)
}

#[tauri::command]
pub(crate) fn launch_current() -> Result<ActionDto, String> {
    crate::services::launch_current()
}

#[tauri::command]
pub(crate) fn launch_vanilla() -> Result<ActionDto, String> {
    crate::services::launch_vanilla()
}

#[tauri::command]
pub(crate) fn create_save_backup() -> Result<ActionDto, String> {
    crate::services::create_save_backup()
}

#[tauri::command]
pub(crate) fn clear_current_runs() -> Result<ActionDto, String> {
    crate::services::clear_current_runs()
}

#[tauri::command]
pub(crate) fn restore_save_backup(id: String) -> Result<ActionDto, String> {
    crate::services::restore_save_backup(id)
}

#[tauri::command]
pub(crate) fn delete_save_backups(ids: Vec<String>) -> Result<ActionDto, String> {
    crate::services::delete_save_backups(ids)
}

#[tauri::command]
pub(crate) async fn create_json_translation_sheet(
    source_path: String,
    existing_sheet_path: Option<String>,
    output_path: Option<String>,
    target_language: Option<String>,
) -> Result<JsonSheetActionDto, String> {
    run_blocking(move || {
        crate::services::create_json_translation_sheet(
            source_path,
            existing_sheet_path,
            output_path,
            target_language,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn recalculate_json_translation_sheet(
    source_path: String,
    current_sheet_path: String,
    output_path: Option<String>,
    target_language: Option<String>,
) -> Result<JsonSheetActionDto, String> {
    run_blocking(move || {
        crate::services::recalculate_json_translation_sheet(
            source_path,
            current_sheet_path,
            output_path,
            target_language,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn load_json_translation_sheet(sheet_path: String) -> Result<JsonSheetDto, String> {
    run_blocking(move || crate::services::load_json_translation_sheet(sheet_path)).await
}

#[tauri::command]
pub(crate) fn validate_json_translation_sheet(
    sheet_path: String,
) -> Result<JsonValidationDto, String> {
    crate::services::validate_json_translation_sheet(sheet_path)
}

#[tauri::command]
pub(crate) fn validate_json_translation_sheet_data(
    sheet: JsonSheetDto,
) -> Result<JsonValidationDto, String> {
    crate::services::validate_json_translation_sheet_data(sheet)
}

#[tauri::command]
pub(crate) async fn save_json_translation_sheet(
    sheet_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    run_blocking(move || crate::services::save_json_translation_sheet(sheet_path, sheet)).await
}

#[tauri::command]
pub(crate) fn export_json_translation_csv(
    output_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonCsvExportDto, String> {
    crate::services::export_json_translation_csv(output_path, sheet)
}

#[tauri::command]
pub(crate) fn export_json_translation_short_json(
    output_path: String,
    sheet: JsonSheetDto,
    only_empty: Option<bool>,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    crate::services::export_json_translation_short_json(
        output_path,
        sheet,
        only_empty,
        include_keys,
    )
}

#[tauri::command]
pub(crate) fn export_json_translation_warning_json(
    output_path: String,
    sheet: JsonSheetDto,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    crate::services::export_json_translation_warning_json(output_path, sheet, include_keys)
}

#[tauri::command]
pub(crate) fn export_json_translation_change_json(
    output_path: String,
    sheet: JsonSheetDto,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    crate::services::export_json_translation_change_json(output_path, sheet, include_keys)
}

#[tauri::command]
pub(crate) fn import_json_translation_values(
    input_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    crate::services::import_json_translation_values(input_path, sheet)
}

#[tauri::command]
pub(crate) fn compare_translation_language(
    sheet_path: String,
    sample_path: String,
) -> Result<Vec<LanguageCompareValueDto>, String> {
    crate::services::compare_translation_language(sheet_path, sample_path)
}

#[tauri::command]
pub(crate) fn apply_json_translation_sheet(
    sheet_path: String,
    output_path: String,
    pck_target_path: Option<String>,
) -> Result<JsonApplyDto, String> {
    crate::services::apply_json_translation_sheet(sheet_path, output_path, pck_target_path)
}

#[tauri::command]
pub(crate) fn export_translation_patch_mod(
    sheet_path: String,
    output_dir: String,
) -> Result<TranslationPatchExportDto, String> {
    crate::services::export_translation_patch_mod(sheet_path, output_dir)
}
