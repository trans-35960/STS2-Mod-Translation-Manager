fn paths_dto(config: &AppConfig) -> PathsDto {
    PathsDto {
        workspace: display_path(&config.workspace_dir),
        game: display_path(&config.game_dir),
        game_mods: display_path(&config.game_mods_dir),
        save_dir: config
            .save_dir
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        save_backup: display_path(&config.save_backup_dir),
        disabled: display_path(&game_disabled_dir(&config.game_mods_dir)),
        presets: display_path(&config.presets_dir),
        translation_work: display_path(&config.translation_work_dir),
        state: display_path(&config.state_dir),
        vendor: display_path(&config.vendor_dir),
        external_manager_dirs: config
            .external_manager_dirs
            .iter()
            .map(|path| display_path(path))
            .collect(),
    }
}


fn preset_dto(preset: Preset) -> PresetDto {
    PresetDto {
        name: preset.name,
        mod_count: preset.mods.len(),
        mods: preset
            .mods
            .into_iter()
            .map(|entry| PresetModDto {
                key: entry.key,
                version_hint: entry.version_hint,
                bytes: Some(entry.bytes),
                modified_epoch: entry.modified_epoch,
                file_name: Some(entry.file_name),
            })
            .collect(),
    }
}

fn translation_dto(workspace: TranslationWorkspace) -> TranslationWorkspaceDto {
    TranslationWorkspaceDto {
        mod_key: workspace.mod_key,
        version_id: workspace.version_id,
        review_required: workspace.review_required,
        path: display_path(&workspace.path),
    }
}

fn tool_dto(tool: VendorTool) -> ToolDto {
    ToolDto {
        name: tool.name.to_string(),
        available: tool.available,
        purpose: tool.purpose.to_string(),
        expected_path: display_path(&tool.expected_path),
    }
}

fn launch_dto(status: LaunchStatus) -> LaunchStatusDto {
    let game_exe = status.game_exe.as_deref().map(display_path);
    let steam_exe = status.steam_exe.as_deref().map(display_path);
    let target_label = game_exe
        .clone()
        .or_else(|| {
            steam_exe
                .clone()
                .map(|path| format!("Steam fallback ({path})"))
        })
        .unwrap_or_else(|| "not found".to_string());
    LaunchStatusDto {
        ready: status.ready,
        game_exe,
        steam_exe,
        target_label,
        running: status.running,
    }
}


fn preset_apply_message(name: &str, report: &PresetApplyReport) -> String {
    let dto = PresetApplyDto {
        disabled: report.disabled.len(),
        enabled: report.enabled.len(),
        missing: report.missing.clone(),
        version_warnings: report.version_warnings.clone(),
    };

    let mut parts = vec![format!(
        "'{name}' 프리셋 적용 완료: {}개 활성화, {}개 비활성화",
        dto.enabled, dto.disabled
    )];
    if !dto.missing.is_empty() {
        parts.push(format!("누락 {}개", dto.missing.len()));
    }
    if !dto.version_warnings.is_empty() {
        parts.push(format!("버전 확인 필요 {}개", dto.version_warnings.len()));
    }
    parts.join(" / ")
}


