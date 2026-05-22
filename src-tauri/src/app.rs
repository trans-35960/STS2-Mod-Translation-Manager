use crate::commands;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::load_dashboard,
            commands::save_settings,
            commands::save_mod_view_mode,
            commands::read_game_logs,
            commands::scan_updates,
            commands::repair_mod_installations,
            commands::open_path,
            commands::preview_dropped_mods,
            commands::import_dropped_mod,
            commands::toggle_mod,
            commands::toggle_mods,
            commands::delete_mod,
            commands::delete_mods,
            commands::restore_deleted_mod,
            commands::empty_deleted_mods,
            commands::cleanup_orphan_caches,
            commands::cleanup_dropped_mod_preview_cache,
            commands::extract_translation,
            commands::clear_translation_extract_cache,
            commands::prepare_translation_node,
            commands::save_preset,
            commands::apply_preset,
            commands::export_preset,
            commands::import_preset_archive,
            commands::launch_current,
            commands::launch_vanilla,
            commands::create_save_backup,
            commands::clear_current_runs,
            commands::restore_save_backup,
            commands::delete_save_backups,
            commands::create_json_translation_sheet,
            commands::recalculate_json_translation_sheet,
            commands::load_json_translation_sheet,
            commands::validate_json_translation_sheet,
            commands::validate_json_translation_sheet_data,
            commands::save_json_translation_sheet,
            commands::export_json_translation_csv,
            commands::export_json_translation_short_json,
            commands::export_json_translation_warning_json,
            commands::export_json_translation_change_json,
            commands::import_json_translation_values,
            commands::compare_translation_language,
            commands::apply_json_translation_sheet,
            commands::export_translation_patch_mod
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .setup(|app| {
            crate::services::configure_runtime_paths(
                app.path().app_data_dir().ok(),
                app.path().resource_dir().ok(),
            );
            if std::env::var_os("STS2_E2E_NO_FOCUS").is_some()
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.hide();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri app");
}
