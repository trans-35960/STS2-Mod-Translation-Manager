pub(crate) fn save_settings(
    translation_work_dir: String,
    target_language: String,
    game_exe_path: String,
    game_log_path: String,
    save_dir: String,
    save_backup_dir: String,
    save_backup_retention_days: u32,
    save_backup_max_entries: u32,
    deleted_retention_days: u32,
    mod_view_mode: String,
) -> Result<ActionDto, String> {
    let mut config = AppConfig::from_workspace(resolve_workspace_dir());
    if sts2_mod_manager::launcher::status(&config).running {
        return Err("게임 실행 중에는 설정을 변경할 수 없습니다.".to_string());
    }
    fs::create_dir_all(&config.state_dir).map_err(|error| error.to_string())?;
    let settings = UiSettingsDto {
        translation_work_dir: translation_work_dir.trim().to_string(),
        target_language: normalize_target_language(target_language.trim()).to_string(),
        game_exe_path: game_exe_path.trim().to_string(),
        game_log_path: game_log_path.trim().to_string(),
        save_dir: save_dir.trim().to_string(),
        save_backup_dir: save_backup_dir.trim().to_string(),
        save_backup_retention_days: sanitize_save_backup_retention_days(save_backup_retention_days),
        save_backup_max_entries: sanitize_save_backup_max_entries(save_backup_max_entries),
        deleted_retention_days: sanitize_deleted_retention_days(deleted_retention_days),
        mod_view_mode: normalize_mod_view_mode(&mod_view_mode).to_string(),
    };
    if !settings.game_exe_path.is_empty() {
        let path = PathBuf::from(&settings.game_exe_path);
        if !path.is_file() {
            return Err("게임 실행 파일 경로가 올바르지 않습니다.".to_string());
        }
    }
    if !settings.game_log_path.is_empty() {
        let path = PathBuf::from(&settings.game_log_path);
        if !path.is_file() {
            return Err("게임 로그 파일 경로가 올바르지 않습니다.".to_string());
        }
    }
    write_ui_settings(&config, &settings).map_err(|error| error.to_string())?;
    if !settings.translation_work_dir.is_empty() {
        config.translation_work_dir = PathBuf::from(&settings.translation_work_dir);
        fs::create_dir_all(&config.translation_work_dir).map_err(|error| error.to_string())?;
    }
    if !settings.save_backup_dir.is_empty() {
        config.save_backup_dir = PathBuf::from(&settings.save_backup_dir);
        fs::create_dir_all(&config.save_backup_dir).map_err(|error| error.to_string())?;
    }

    Ok(ActionDto {
        message: "설정 저장 완료".to_string(),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn save_mod_view_mode(mod_view_mode: String) -> Result<(), String> {
    let config = AppConfig::from_workspace(resolve_workspace_dir());
    fs::create_dir_all(&config.state_dir).map_err(|error| error.to_string())?;
    let mut settings = read_ui_settings(&config).map_err(|error| error.to_string())?;
    settings.mod_view_mode = normalize_mod_view_mode(&mod_view_mode).to_string();
    write_ui_settings(&config, &settings).map_err(|error| error.to_string())
}

fn read_ui_settings(config: &AppConfig) -> sts2_mod_manager::error::AppResult<UiSettingsDto> {
    let defaults = UiSettingsDto {
        translation_work_dir: display_path(&config.translation_work_dir),
        target_language: "kor".to_string(),
        game_exe_path: config
            .game_exe_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        game_log_path: default_sts2_game_log_path()
            .map(|path| display_path(&path))
            .unwrap_or_default(),
        save_dir: config
            .save_dir
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        save_backup_dir: display_path(&config.save_backup_dir),
        save_backup_retention_days: config.save_backup_retention_days,
        save_backup_max_entries: config.save_backup_max_entries as u32,
        deleted_retention_days: 30,
        mod_view_mode: "detail".to_string(),
    };
    let path = ui_settings_path(config);
    if !path.exists() {
        return Ok(defaults);
    }

    let content = fs::read_to_string(&path)
        .map_err(|source| sts2_mod_manager::error::AppError::io(path.as_path(), source))?;
    let mut settings = defaults;
    for line in content.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        match key {
            "translation_work_dir" if !value.trim().is_empty() => {
                settings.translation_work_dir = value.trim().to_string();
            }
            "target_language" if !value.trim().is_empty() => {
                settings.target_language = normalize_target_language(value.trim()).to_string();
            }
            "game_exe_path" => {
                settings.game_exe_path = value.trim().to_string();
            }
            "game_log_path" => {
                settings.game_log_path = value.trim().to_string();
            }
            "save_dir" => {
                settings.save_dir = value.trim().to_string();
            }
            "save_backup_dir" if !value.trim().is_empty() => {
                settings.save_backup_dir = value.trim().to_string();
            }
            "save_backup_retention_days" => {
                if let Ok(days) = value.trim().parse::<u32>() {
                    settings.save_backup_retention_days = sanitize_save_backup_retention_days(days);
                }
            }
            "save_backup_max_entries" => {
                if let Ok(entries) = value.trim().parse::<u32>() {
                    settings.save_backup_max_entries = sanitize_save_backup_max_entries(entries);
                }
            }
            "deleted_retention_days" => {
                if let Ok(days) = value.trim().parse::<u32>() {
                    settings.deleted_retention_days = sanitize_deleted_retention_days(days);
                }
            }
            "mod_view_mode" if !value.trim().is_empty() => {
                settings.mod_view_mode = normalize_mod_view_mode(value.trim()).to_string();
            }
            _ => {}
        }
    }
    if settings.save_backup_max_entries == 0 {
        settings.save_backup_max_entries = 14;
    }
    Ok(settings)
}

fn write_ui_settings(
    config: &AppConfig,
    settings: &UiSettingsDto,
) -> sts2_mod_manager::error::AppResult<()> {
    let path = ui_settings_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| sts2_mod_manager::error::AppError::io(parent, source))?;
    }
    let output = format!(
        "translation_work_dir\t{}\ntarget_language\t{}\ngame_exe_path\t{}\ngame_log_path\t{}\nsave_dir\t{}\nsave_backup_dir\t{}\nsave_backup_retention_days\t{}\nsave_backup_max_entries\t{}\ndeleted_retention_days\t{}\nmod_view_mode\t{}\n",
        settings.translation_work_dir,
        settings.target_language,
        settings.game_exe_path,
        settings.game_log_path,
        settings.save_dir,
        settings.save_backup_dir,
        settings.save_backup_retention_days,
        settings.save_backup_max_entries,
        settings.deleted_retention_days,
        settings.mod_view_mode
    );
    fs::write(&path, output)
        .map_err(|source| sts2_mod_manager::error::AppError::io(path.as_path(), source))
}

fn ui_settings_path(config: &AppConfig) -> PathBuf {
    config.state_dir.join("tauri_settings.tsv")
}

fn normalize_target_language(value: &str) -> &str {
    match value {
        "ko" => "kor",
        other => other,
    }
}

fn sanitize_deleted_retention_days(days: u32) -> u32 {
    days.min(365)
}

fn sanitize_save_backup_retention_days(days: u32) -> u32 {
    days.min(365)
}

fn sanitize_save_backup_max_entries(entries: u32) -> u32 {
    entries.clamp(1, 200)
}

fn normalize_mod_view_mode(value: &str) -> &'static str {
    if value.eq_ignore_ascii_case("simple") {
        "simple"
    } else {
        "detail"
    }
}


