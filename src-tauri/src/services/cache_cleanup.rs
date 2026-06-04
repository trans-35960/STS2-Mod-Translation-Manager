pub(crate) fn cleanup_orphan_caches() -> Result<ActionDto, String> {
    let app = app();
    let config = app.config().clone();
    let mut removed_dirs = 0usize;
    let mut removed_files = 0usize;

    cleanup_work_caches(&config, &mut removed_dirs, &mut removed_files)?;
    let mut dashboard = dashboard().map_err(|error| error.to_string())?;
    cleanup_work_caches(&config, &mut removed_dirs, &mut removed_files)?;
    dashboard.cache_usage = work_cache_usage(&config);

    Ok(ActionDto {
        message: format!("작업 캐시 정리 완료: 폴더 {removed_dirs}개, 파일 {removed_files}개"),
        dashboard,
    })
}

pub(crate) fn cleanup_dropped_mod_preview_cache() -> Result<(), String> {
    let app = app();
    let path = app.config().state_dir.join("drop_imports");
    if path.exists() {
        ensure_existing_state_path(&path, app.config(), "드롭 모드 미리보기 캐시")?;
    }
    remove_path_if_exists(&path).map_err(|error| error.to_string())
}

fn work_cache_usage(config: &AppConfig) -> CacheUsageDto {
    let mut usage = CacheUsageDto {
        bytes: 0,
        files: 0,
        dirs: 0,
    };
    add_cache_path_usage(
        &config.state_dir.join("language_preview_extract"),
        &mut usage,
    );
    add_cache_path_usage(&config.state_dir.join("drop_imports"), &mut usage);
    add_cache_path_usage(&config.state_dir.join("language_preview_cache.tsv"), &mut usage);
    add_translation_work_payload_usage(&config.translation_work_dir, &mut usage);
    usage
}

fn cached_work_cache_usage(config: &AppConfig) -> CacheUsageDto {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(Instant, String, CacheUsageDto)>>> =
        std::sync::OnceLock::new();
    let key = format!(
        "{}|{}",
        config.state_dir.display(),
        config.translation_work_dir.display()
    );
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock()
        && let Some((created, cached_key, usage)) = guard.as_ref()
        && cached_key == &key
        && created.elapsed().as_secs() < 120
    {
        return usage.clone();
    }
    let usage = work_cache_usage(config);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((Instant::now(), key, usage.clone()));
    }
    usage
}

fn add_translation_work_payload_usage(root: &Path, usage: &mut CacheUsageDto) {
    if !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if matches!(name.as_str(), "selected" | "translation_memory") {
                continue;
            }
            if matches!(name.as_str(), "expanded_archive" | "pck_build")
                || name.ends_with(".pck.contents")
            {
                add_cache_path_usage(&path, usage);
                continue;
            }
            add_translation_work_payload_usage(&path, usage);
            continue;
        }
        if is_translation_memory_payload_file(&path) {
            add_cache_path_usage(&path, usage);
        }
    }
}

fn add_cache_path_usage(path: &Path, usage: &mut CacheUsageDto) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        usage.dirs += 1;
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            add_cache_path_usage(&entry.path(), usage);
        }
    } else {
        usage.files += 1;
        usage.bytes = usage.bytes.saturating_add(metadata.len());
    }
}

fn cleanup_work_caches(
    config: &AppConfig,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
) -> Result<(), String> {
    let cache_roots = vec![config.state_dir.clone(), config.translation_work_dir.clone()];
    cleanup_translation_memory_payloads(
        &config.translation_work_dir.join("translation_memory"),
        removed_dirs,
        removed_files,
        &cache_roots,
    )?;
    cleanup_translation_work_payloads(
        &config.translation_work_dir,
        removed_dirs,
        removed_files,
        &cache_roots,
    )?;
    remove_cache_path_if_exists(
        &config.state_dir.join("language_preview_extract"),
        removed_dirs,
        removed_files,
        &cache_roots,
    )?;
    remove_cache_path_if_exists(
        &config.state_dir.join("drop_imports"),
        removed_dirs,
        removed_files,
        &cache_roots,
    )?;
    remove_cache_path_if_exists(
        &config.state_dir.join("language_preview_cache.tsv"),
        removed_dirs,
        removed_files,
        &cache_roots,
    )?;
    Ok(())
}

fn cleanup_translation_memory_payloads(
    root: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
    cache_roots: &[PathBuf],
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let mut payloads = Vec::new();
    collect_translation_memory_payloads(root, &mut payloads)?;
    for path in payloads {
        remove_cache_path(&path, removed_dirs, removed_files, cache_roots)?;
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
    cache_roots: &[PathBuf],
) -> Result<(), String> {
    cleanup_translation_work_payloads_inner(root, root, removed_dirs, removed_files, cache_roots)
}

fn cleanup_translation_work_payloads_inner(
    base: &Path,
    root: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
    cache_roots: &[PathBuf],
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
                remove_cache_path(&path, removed_dirs, removed_files, cache_roots)?;
                continue;
            }
            cleanup_translation_work_payloads_inner(
                base,
                &path,
                removed_dirs,
                removed_files,
                cache_roots,
            )?;
            continue;
        }
        if is_selected_translation_payload_file(base, &path) {
            remove_cache_path(&path, removed_dirs, removed_files, cache_roots)?;
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

fn remove_cache_path(
    path: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
    cache_roots: &[PathBuf],
) -> Result<(), String> {
    ensure_existing_path_in_roots(path, cache_roots, "캐시 경로")?;
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

fn remove_cache_path_if_exists(
    path: &Path,
    removed_dirs: &mut usize,
    removed_files: &mut usize,
    cache_roots: &[PathBuf],
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    remove_cache_path(path, removed_dirs, removed_files, cache_roots)
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


