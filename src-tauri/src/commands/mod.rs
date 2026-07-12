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
pub(crate) async fn load_dashboard() -> Result<DashboardDto, String> {
    run_blocking(crate::services::load_dashboard).await
}

#[tauri::command]
pub(crate) async fn load_launch_status() -> Result<LaunchStatusDto, String> {
    run_blocking(crate::services::load_launch_status).await
}

#[tauri::command]
pub(crate) async fn load_cache_usage() -> Result<CacheUsageDto, String> {
    run_blocking(crate::services::load_cache_usage).await
}

#[tauri::command]
pub(crate) async fn save_settings(request: SaveSettingsRequest) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::save_settings(request)).await
}

#[tauri::command]
pub(crate) async fn save_mod_view_mode(mod_view_mode: String) -> Result<(), String> {
    run_blocking(move || crate::services::save_mod_view_mode(mod_view_mode)).await
}

#[tauri::command]
pub(crate) async fn read_game_logs() -> Result<Vec<GameLogDto>, String> {
    run_blocking(crate::services::read_game_logs).await
}

#[tauri::command]
pub(crate) async fn scan_updates() -> Result<ActionDto, String> {
    run_blocking(crate::services::scan_updates).await
}

#[tauri::command]
pub(crate) async fn repair_mod_installations() -> Result<ActionDto, String> {
    run_blocking(crate::services::repair_mod_installations).await
}

#[tauri::command]
pub(crate) async fn open_path(path: String) -> Result<(), String> {
    run_blocking(move || crate::services::open_path(path)).await
}

#[tauri::command]
pub(crate) async fn preview_dropped_mods(
    paths: Vec<String>,
) -> Result<Vec<DroppedModPreviewDto>, String> {
    run_blocking(move || crate::services::preview_dropped_mods(paths)).await
}

#[tauri::command]
pub(crate) async fn import_dropped_mod(
    path: String,
    replace_path: Option<String>,
) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::import_dropped_mod(path, replace_path)).await
}

#[tauri::command]
pub(crate) async fn import_dropped_mods(
    decisions: Vec<DroppedModDecisionDto>,
) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::import_dropped_mods(decisions)).await
}

#[tauri::command]
pub(crate) async fn toggle_mod(
    key: String,
    active: bool,
    force: Option<bool>,
) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::toggle_mod(key, active, force.unwrap_or(false))).await
}

#[tauri::command]
pub(crate) async fn toggle_mods(changes: Vec<ModToggleDto>) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::toggle_mods(changes)).await
}

#[tauri::command]
pub(crate) async fn delete_mod(key: String, path: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::delete_mod(key, path)).await
}

#[tauri::command]
pub(crate) async fn delete_mods(items: Vec<ModDeleteDto>) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::delete_mods(items)).await
}

#[tauri::command]
pub(crate) async fn restore_deleted_mod(id: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::restore_deleted_mod(id)).await
}

#[tauri::command]
pub(crate) async fn empty_deleted_mods() -> Result<ActionDto, String> {
    run_blocking(crate::services::empty_deleted_mods).await
}

#[tauri::command]
pub(crate) async fn cleanup_orphan_caches() -> Result<ActionDto, String> {
    run_blocking(crate::services::cleanup_orphan_caches).await
}

#[tauri::command]
pub(crate) async fn cleanup_dropped_mod_preview_cache() -> Result<(), String> {
    run_blocking(crate::services::cleanup_dropped_mod_preview_cache).await
}

#[tauri::command]
pub(crate) async fn extract_translation(
    key: String,
    output_dir: Option<String>,
    resource_path: Option<String>,
    force: Option<bool>,
) -> Result<ActionDto, String> {
    run_blocking(move || {
        crate::services::extract_translation(key, output_dir, resource_path, force.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_translation_extract_cache(key: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::clear_translation_extract_cache(key)).await
}

#[tauri::command]
pub(crate) async fn prepare_translation_node(
    key: String,
    resource_path: String,
    output_dir: Option<String>,
    force: Option<bool>,
    on_progress: tauri::ipc::Channel<TranslationPreparationProgressDto>,
) -> Result<NodeTranslationDto, String> {
    run_blocking(move || {
        crate::services::prepare_translation_node(
            key,
            resource_path,
            output_dir,
            force.unwrap_or(false),
            move |progress| {
                let _ = on_progress.send(progress);
            },
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn save_preset(name: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::save_preset(name)).await
}

#[tauri::command]
pub(crate) async fn apply_preset(name: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::apply_preset(name)).await
}

#[tauri::command]
pub(crate) async fn export_preset(name: String, archive_path: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::export_preset(name, archive_path)).await
}

#[tauri::command]
pub(crate) async fn import_preset_archive(archive_path: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::import_preset_archive(archive_path)).await
}

#[tauri::command]
pub(crate) async fn launch_current() -> Result<ActionDto, String> {
    run_blocking(crate::services::launch_current).await
}

#[tauri::command]
pub(crate) async fn launch_vanilla() -> Result<ActionDto, String> {
    run_blocking(crate::services::launch_vanilla).await
}

#[tauri::command]
pub(crate) async fn create_save_backup() -> Result<ActionDto, String> {
    run_blocking(crate::services::create_save_backup).await
}

#[tauri::command]
pub(crate) async fn clear_current_runs() -> Result<ActionDto, String> {
    run_blocking(crate::services::clear_current_runs).await
}

#[tauri::command]
pub(crate) async fn restore_save_backup(id: String) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::restore_save_backup(id)).await
}

#[tauri::command]
pub(crate) async fn delete_save_backups(ids: Vec<String>) -> Result<ActionDto, String> {
    run_blocking(move || crate::services::delete_save_backups(ids)).await
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
pub(crate) async fn validate_json_translation_sheet(
    sheet_path: String,
) -> Result<JsonValidationDto, String> {
    run_blocking(move || crate::services::validate_json_translation_sheet(sheet_path)).await
}

#[tauri::command]
pub(crate) async fn validate_json_translation_sheet_data(
    sheet: JsonSheetDto,
) -> Result<JsonValidationDto, String> {
    run_blocking(move || crate::services::validate_json_translation_sheet_data(sheet)).await
}

#[tauri::command]
pub(crate) async fn save_json_translation_sheet(
    sheet_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    run_blocking(move || crate::services::save_json_translation_sheet(sheet_path, sheet)).await
}

#[tauri::command]
pub(crate) async fn export_json_translation_csv(
    output_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonCsvExportDto, String> {
    run_blocking(move || crate::services::export_json_translation_csv(output_path, sheet)).await
}

#[tauri::command]
pub(crate) async fn export_json_translation_short_json(
    output_path: String,
    sheet: JsonSheetDto,
    only_empty: Option<bool>,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    run_blocking(move || {
        crate::services::export_json_translation_short_json(
            output_path,
            sheet,
            only_empty,
            include_keys,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_json_translation_warning_json(
    output_path: String,
    sheet: JsonSheetDto,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    run_blocking(move || {
        crate::services::export_json_translation_warning_json(output_path, sheet, include_keys)
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_json_translation_change_json(
    output_path: String,
    sheet: JsonSheetDto,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    run_blocking(move || {
        crate::services::export_json_translation_change_json(output_path, sheet, include_keys)
    })
    .await
}

#[tauri::command]
pub(crate) async fn import_json_translation_values(
    input_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    run_blocking(move || crate::services::import_json_translation_values(input_path, sheet)).await
}

#[tauri::command]
pub(crate) async fn compare_translation_language(
    sheet_path: String,
    sample_path: String,
) -> Result<Vec<LanguageCompareValueDto>, String> {
    run_blocking(move || crate::services::compare_translation_language(sheet_path, sample_path)).await
}

#[tauri::command]
pub(crate) async fn apply_json_translation_sheet(
    sheet_path: String,
    output_path: String,
    pck_target_path: Option<String>,
) -> Result<JsonApplyDto, String> {
    run_blocking(move || {
        crate::services::apply_json_translation_sheet(sheet_path, output_path, pck_target_path)
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_translation_patch_mod(
    sheet_path: String,
    output_dir: String,
) -> Result<TranslationPatchExportDto, String> {
    run_blocking(move || crate::services::export_translation_patch_mod(sheet_path, output_dir)).await
}
