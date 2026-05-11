pub(crate) fn cleanup_orphan_caches() -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    let report = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?;
    let connected_keys = connected_mod_keys(&report.summary);
    let current_cache_dirs = current_language_preview_scan_dirs(&report.summary, config);
    let mut removed_dirs = 0usize;
    let mut removed_files = 0usize;

    cleanup_translation_work_children(
        &config.translation_work_dir,
        &connected_keys,
        &mut removed_dirs,
        &mut removed_files,
    )?;
    cleanup_named_cache_children(
        &config.translation_work_dir.join("selected"),
        &connected_keys,
        &mut removed_dirs,
        &mut removed_files,
    )?;
    cleanup_named_cache_children(
        &config.translation_work_dir.join("translation_memory"),
        &connected_keys,
        &mut removed_dirs,
        &mut removed_files,
    )?;
    cleanup_translation_memory_payloads(
        &config.translation_work_dir.join("translation_memory"),
        &mut removed_dirs,
        &mut removed_files,
    )?;
    cleanup_translation_work_payloads(
        &config.translation_work_dir,
        &mut removed_dirs,
        &mut removed_files,
    )?;
    cleanup_language_preview_extract(
        &config.state_dir.join("language_preview_extract"),
        &current_cache_dirs,
        &mut removed_dirs,
        &mut removed_files,
    )?;
    prune_language_preview_cache(config, &report.summary)?;

    Ok(ActionDto {
        message: format!("작업 캐시 정리 완료: 폴더 {removed_dirs}개, 파일 {removed_files}개"),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}


fn cleanup_translation_work_children(
    root: &Path,
    connected_keys: &BTreeSet<String>,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(name.as_str(), "selected" | "translation_memory") {
            continue;
        }
        if connected_keys.contains(&name) {
            continue;
        }
        remove_cache_path(&path, removed_dirs, removed_files)?;
    }
    Ok(())
}

fn cleanup_named_cache_children(
    root: &Path,
    connected_keys: &BTreeSet<String>,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if connected_keys.contains(&name) {
            continue;
        }
        remove_cache_path(&path, removed_dirs, removed_files)?;
    }
    Ok(())
}

fn cleanup_translation_memory_payloads(
    root: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let mut payloads = Vec::new();
    collect_translation_memory_payloads(root, &mut payloads)?;
    for path in payloads {
        remove_cache_path(&path, removed_dirs, removed_files)?;
    }
    Ok(())
}

fn collect_translation_memory_payloads(
    root: &Path,
    payloads: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("pck_build")
            {
                payloads.push(path);
                continue;
            }
            collect_translation_memory_payloads(&path, payloads)?;
            continue;
        }
        if is_translation_memory_payload_file(&path) {
            payloads.push(path);
        }
    }
    Ok(())
}

fn cleanup_translation_work_payloads(
    root: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    cleanup_translation_work_payloads_inner(root, root, removed_dirs, removed_files)
}

fn cleanup_translation_work_payloads_inner(
    base: &Path,
    root: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name == "expanded_archive" || name == "pck_build" || name.ends_with(".pck.contents")
            {
                remove_cache_path(&path, removed_dirs, removed_files)?;
                continue;
            }
            cleanup_translation_work_payloads_inner(base, &path, removed_dirs, removed_files)?;
            continue;
        }
        if is_selected_translation_payload_file(base, &path) {
            remove_cache_path(&path, removed_dirs, removed_files)?;
        }
    }
    Ok(())
}

fn is_selected_translation_payload_file(root: &Path, path: &Path) -> bool {
    if !is_translation_memory_payload_file(path) {
        return false;
    }
    path.strip_prefix(root).ok().is_some_and(|relative| {
        relative.components().any(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("selected")
        })
    })
}

fn is_translation_memory_payload_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "pck" | "zip" | "rar" | "7z"
            )
        })
        .unwrap_or(false)
}

fn cleanup_language_preview_extract(
    root: &Path,
    current_cache_dirs: &BTreeSet<String>,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if current_cache_dirs.contains(&name) {
            continue;
        }
        remove_cache_path(&path, removed_dirs, removed_files)?;
    }
    Ok(())
}

fn current_language_preview_scan_dirs(
    summary: &ScanSummary,
    config: &AppConfig,
) -> BTreeSet<String> {
    current_language_preview_cache_keys(summary, config)
        .into_iter()
        .filter_map(|cache_key| {
            language_preview_extract_dir(&cache_key)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect()
}

fn current_language_preview_cache_keys(
    summary: &ScanSummary,
    config: &AppConfig,
) -> BTreeSet<String> {
    summary
        .game_mods
        .iter()
        .chain(summary.vault_mods.iter())
        .chain(summary.external_manager_mods.iter())
        .map(|record| {
            let extraction_source = extraction_source_for_record(record);
            language_cache_key(record, &extraction_source, &config.vendor_dir)
        })
        .collect()
}

fn prune_language_preview_cache(config: &AppConfig, summary: &ScanSummary) -> Result<(), String> {
    let current_keys = current_language_preview_cache_keys(summary, config);
    let mut cache = read_language_preview_cache(config).map_err(|error| error.to_string())?;
    let before = cache.entries.len();
    cache.entries.retain(|key, _| current_keys.contains(key));
    if cache.entries.len() != before {
        cache.dirty = true;
        write_language_preview_cache(config, &cache).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_cache_path(
    path: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    let (dirs, files) = count_path_items(path);
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    } else if path.is_file() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    } else {
        return Ok(());
    }
    *removed_dirs += dirs;
    *removed_files += files;
    Ok(())
}

fn count_path_items(path: &Path) -> (usize, usize) {
    if path.is_file() {
        return (0, 1);
    }
    if !path.is_dir() {
        return (0, 0);
    }
    let mut dirs = 1;
    let mut files = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let (child_dirs, child_files) = count_path_items(&entry.path());
            dirs += child_dirs;
            files += child_files;
        }
    }
    (dirs, files)
}


