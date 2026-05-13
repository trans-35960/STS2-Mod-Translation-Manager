pub(crate) fn extract_translation(
    key: String,
    output_dir: Option<String>,
    resource_path: Option<String>,
    force: bool,
) -> Result<ActionDto, String> {
    let app = app();
    let record = find_mod_record(&app, &key)?;
    let source = extraction_source_for_record(&record);
    app.ensure_workspace_dirs()
        .map_err(|error| error.to_string())?;
    if force {
        clear_translation_extract_cache_for_record(&record, &source, &app.config().vendor_dir)?;
    }
    let output_root = output_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| app.config().translation_work_dir.clone());
    let extracted = extract_mod_files(
        &record,
        &source,
        &output_root,
        resource_path.as_deref(),
        &app.config().vendor_dir,
    )?;
    Ok(ActionDto {
        message: format!(
            "{} 파일 추출 완료: {}개 ({})",
            record.stable_key(),
            extracted.files,
            extracted.output_dir.display()
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn clear_translation_extract_cache(key: String) -> Result<ActionDto, String> {
    let app = app();
    let record = find_mod_record(&app, &key)?;
    let source = extraction_source_for_record(&record);
    let removed =
        clear_translation_extract_cache_for_record(&record, &source, &app.config().vendor_dir)?;
    Ok(ActionDto {
        message: if removed == 0 {
            format!("{} 추출 캐시 없음", record.stable_key())
        } else {
            format!("{} 추출 캐시 삭제 완료: {}개", record.stable_key(), removed)
        },
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

fn clear_translation_extract_cache_for_record(
    record: &ModRecord,
    source: &Path,
    vendor_dir: &Path,
) -> Result<usize, String> {
    let cache_key = language_cache_key(record, source, vendor_dir);
    let paths = [
        language_preview_extract_dir(&cache_key),
        language_preview_extract_dir(&format!("{cache_key}-full")),
    ];
    let mut removed = 0usize;
    for path in paths {
        if !path.exists() {
            continue;
        }
        fs::remove_dir_all(&path).map_err(|error| {
            format!("추출 캐시를 삭제하지 못했습니다: {} ({error})", path.display())
        })?;
        removed += 1;
    }
    Ok(removed)
}

struct ModFileExtractReport {
    output_dir: PathBuf,
    files: usize,
}

fn extract_mod_files(
    record: &ModRecord,
    source: &Path,
    output_root: &Path,
    resource_path: Option<&str>,
    vendor_dir: &Path,
) -> Result<ModFileExtractReport, String> {
    let work_dir = output_root
        .join(record.stable_key())
        .join(format!("extract-{}", timestamp_string()));
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let scan_root = full_extract_scan_root(source, &work_dir, vendor_dir)?;
    let selected = selected_extract_paths(&scan_root, resource_path)?;
    if selected.is_empty() {
        return Err("선택한 항목을 찾지 못했습니다.".to_string());
    }
    let output_dir = work_dir.join("files");
    let mut files = 0usize;
    for path in selected {
        files += copy_extract_path(&scan_root, &path, &output_dir)?;
    }
    Ok(ModFileExtractReport { output_dir, files })
}

fn full_extract_scan_root(source: &Path, work_dir: &Path, vendor_dir: &Path) -> Result<PathBuf, String> {
    if source.is_dir() {
        let scan_root = work_dir.join("scan");
        copy_dir_all(source, &scan_root).map_err(|error| error.to_string())?;
        expand_nested_pcks(&scan_root, vendor_dir);
        return Ok(scan_root);
    }
    if is_supported_extractable_path(source) {
        let scan_root = work_dir.join("scan");
        if !expand_source(source, &scan_root, vendor_dir) {
            return Err(format!("추출할 수 없는 모드 파일입니다: {}", source.display()));
        }
        return Ok(scan_root);
    }
    Ok(source.to_path_buf())
}

#[derive(Clone)]
struct TranslationPatchApplyTarget {
    source_path: PathBuf,
    pck_stem: Option<String>,
}

#[derive(Default)]
struct ConnectedTranslationPatchCopy {
    files: usize,
    patch_label: Option<String>,
    apply_target: Option<TranslationPatchApplyTarget>,
}

struct ConnectedTranslationPatchRequest<'a> {
    app: &'a App,
    base_record: &'a ModRecord,
    base_manifest: &'a ModManifestInfo,
    source_scan_root: &'a Path,
    source_files: &'a [PathBuf],
    translated_root: &'a Path,
    target_language: &'a str,
    vendor_dir: &'a Path,
}

fn copy_connected_translation_patch_files(
    request: ConnectedTranslationPatchRequest<'_>,
) -> Result<ConnectedTranslationPatchCopy, String> {
    let ConnectedTranslationPatchRequest {
        app,
        base_record,
        base_manifest,
        source_scan_root,
        source_files,
        translated_root,
        target_language,
        vendor_dir,
    } = request;
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let mut report = ConnectedTranslationPatchCopy::default();
    for candidate in summary
        .game_mods
        .into_iter()
        .chain(summary.disabled_mods)
        .chain(summary.external_manager_mods)
    {
        if candidate.stable_key() == base_record.stable_key() {
            continue;
        }
        let patch_source = extraction_source_for_record(&candidate);
        let cache_key = language_cache_key(&candidate, &patch_source, vendor_dir);
        let Some(patch_scan_root) = extraction_scan_root(&patch_source, &cache_key, vendor_dir) else {
            continue;
        };
        let patch_manifest = read_mod_manifest_for_record(&candidate.path, &patch_scan_root);
        if !is_translation_patch_manifest(&patch_manifest) {
            continue;
        }
        if !translation_patch_matches_language(&patch_manifest, target_language) {
            continue;
        }
        if !translation_patch_targets_base(&candidate, &patch_manifest, base_record, base_manifest) {
            continue;
        }
        let mut target_relatives = Vec::new();
        for source_file in source_files {
            let Some(target_relative) =
                target_language_relative_path(source_scan_root, source_file, target_language)
            else {
                continue;
            };
            if find_resource_file_by_relative(&patch_scan_root, &target_relative).is_some() {
                target_relatives.push(target_relative);
            }
        }
        if target_relatives.is_empty() {
            continue;
        }
        let patch_label = translation_patch_label(&candidate, &patch_manifest);
        if report.files > 0 {
            return Err(format!(
                "연결된 번역 모드가 여러 개라 자동 적용 대상을 정할 수 없습니다: {}, {}. 하나만 남기거나 번역 모드로 내보내기를 사용해 주세요.",
                report
                    .patch_label
                    .as_deref()
                    .unwrap_or("이전 번역 모드"),
                patch_label
            ));
        }
        for target_relative in target_relatives {
            if copy_resource_relative_if_exists(&patch_scan_root, &target_relative, translated_root)? {
                report.files += 1;
                report.apply_target = Some(translation_patch_apply_target(
                    &patch_source,
                    &patch_scan_root,
                ));
            }
        }
        if report.files > 0 {
            report.patch_label = Some(patch_label);
        }
    }
    Ok(report)
}

fn translation_patch_label(record: &ModRecord, manifest: &ModManifestInfo) -> String {
    manifest
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| record.stable_key())
}

fn translation_patch_apply_target(
    patch_source: &Path,
    patch_scan_root: &Path,
) -> TranslationPatchApplyTarget {
    TranslationPatchApplyTarget {
        source_path: patch_source.to_path_buf(),
        pck_stem: pck_stem_from_source_or_scan_root(patch_source, patch_scan_root),
    }
}

fn pck_stem_from_source_or_scan_root(source: &Path, scan_root: &Path) -> Option<String> {
    if is_pck_path(source) {
        return source
            .file_stem()
            .map(|value| value.to_string_lossy().to_string());
    }
    pck_resource_roots(scan_root)
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .and_then(|name| name.strip_suffix(".pck.contents"))
                .map(str::to_string)
        })
        .next()
}

fn is_translation_patch_manifest(manifest: &ModManifestInfo) -> bool {
    manifest.is_translation_patch
        || manifest.target_mod_id.is_some()
        || manifest.target_mod_name.is_some()
        || !manifest.target_languages.is_empty()
}

fn translation_patch_matches_language(manifest: &ModManifestInfo, target_language: &str) -> bool {
    manifest.target_languages.is_empty()
        || manifest
            .target_languages
            .iter()
            .any(|language| normalize_language_code(language) == normalize_language_code(target_language))
}

fn translation_patch_targets_base(
    patch_record: &ModRecord,
    patch_manifest: &ModManifestInfo,
    base_record: &ModRecord,
    base_manifest: &ModManifestInfo,
) -> bool {
    let base_tokens = translation_match_tokens(base_record, base_manifest);
    let mut target_tokens = Vec::new();
    if let Some(target_id) = patch_manifest.target_mod_id.as_deref() {
        target_tokens.push(normalize_dependency_token(target_id));
    }
    if let Some(target_name) = patch_manifest.target_mod_name.as_deref() {
        target_tokens.push(normalize_dependency_token(target_name));
    }
    for dependency in &patch_manifest.dependencies {
        target_tokens.push(normalize_dependency_token(&dependency.id));
    }
    if let Some(stem) = patch_record.stable_key().strip_suffix("_tr") {
        target_tokens.push(normalize_dependency_token(stem));
    }
    target_tokens.retain(|token| !token.is_empty());
    target_tokens.sort();
    target_tokens.dedup();

    target_tokens.iter().any(|target| {
        base_tokens.iter().any(|base| {
            target == base || (!target.is_empty() && !base.is_empty() && base.starts_with(target))
        })
    })
}

fn translation_match_tokens(record: &ModRecord, manifest: &ModManifestInfo) -> Vec<String> {
    let mut tokens = vec![
        normalize_dependency_token(&record.stable_key()),
        normalize_dependency_token(&record.name),
    ];
    if let Some(id) = manifest.id.as_deref() {
        tokens.push(normalize_dependency_token(id));
    }
    if let Some(name) = manifest.name.as_deref() {
        tokens.push(normalize_dependency_token(name));
    }
    if let Some(prefix) = record.stable_key().split(['-', '_', ' ']).next() {
        tokens.push(normalize_dependency_token(prefix));
    }
    tokens.retain(|token| !token.is_empty());
    tokens.sort();
    tokens.dedup();
    tokens
}

fn target_language_relative_path(
    scan_root: &Path,
    source_file: &Path,
    target_language: &str,
) -> Option<PathBuf> {
    let relative = pck_resource_relative_path(source_file)
        .or_else(|| source_file.strip_prefix(scan_root).ok().map(Path::to_path_buf))
        .or_else(|| source_file.file_name().map(PathBuf::from))?;
    replace_resource_language(&relative, target_language)
}

fn copy_resource_relative_if_exists(
    scan_root: &Path,
    relative: &Path,
    translated_root: &Path,
) -> Result<bool, String> {
    let Some(source) = find_resource_file_by_relative(scan_root, relative) else {
        return Ok(false);
    };
    let target = translated_root.join(relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(source, target).map_err(|error| error.to_string())?;
    Ok(true)
}

fn find_resource_file_by_relative(scan_root: &Path, relative: &Path) -> Option<PathBuf> {
    let direct = scan_root.join(relative);
    if direct.is_file() {
        return Some(direct);
    }
    let wanted = normalize_resource_path(&format!("res://{}", slash_path(relative)));
    let wanted_suffix = wanted
        .strip_prefix("res://")
        .map(|value| format!("/{value}"))
        .unwrap_or_else(|| format!("/{wanted}"));
    let mut matches = scan_translation_candidates(scan_root)
        .ok()?
        .into_iter()
        .filter_map(|candidate| {
            let display = normalize_resource_path(&resource_display_path(&candidate.path));
            (display == wanted || display.ends_with(&wanted_suffix)).then_some(candidate.path)
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.into_iter().next()
}

fn selected_extract_paths(scan_root: &Path, resource_path: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let Some(resource_path) = resource_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(vec![scan_root.to_path_buf()]);
    };
    let normalized = normalize_resource_path(resource_path);
    if normalized == "res://" {
        return Ok(pck_resource_roots(scan_root));
    }
    let mut paths = Vec::new();
    collect_matching_extract_paths(scan_root, scan_root, &normalized, &mut paths);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn collect_matching_extract_paths(
    scan_root: &Path,
    path: &Path,
    normalized: &str,
    output: &mut Vec<PathBuf>,
) {
    let display = normalize_resource_path(&extract_display_path(scan_root, path));
    let absolute = normalize_resource_path(&display_path(path));
    if display == normalized || absolute == normalized {
        output.push(path.to_path_buf());
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if !metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_matching_extract_paths(scan_root, &entry.path(), normalized, output);
    }
}

fn copy_extract_path(scan_root: &Path, path: &Path, output_dir: &Path) -> Result<usize, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_file() {
        let relative = extract_relative_path(scan_root, path);
        let target = output_dir.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(path, target).map_err(|error| error.to_string())?;
        return Ok(1);
    }
    let mut files = 0usize;
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;
    for entry in entries.filter_map(Result::ok) {
        files += copy_extract_path(scan_root, &entry.path(), output_dir)?;
    }
    Ok(files)
}

fn extract_display_path(scan_root: &Path, path: &Path) -> String {
    pck_resource_relative_path(path)
        .map(|relative| format!("res://{}", slash_path(&relative)))
        .or_else(|| {
            path.strip_prefix(scan_root)
                .ok()
                .map(slash_path)
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| display_path(path))
}

fn extract_relative_path(scan_root: &Path, path: &Path) -> PathBuf {
    pck_resource_relative_path(path)
        .or_else(|| path.strip_prefix(scan_root).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| {
            path.file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("extracted"))
        })
}

pub(crate) fn prepare_translation_node(
    key: String,
    resource_path: String,
    output_dir: Option<String>,
    force: bool,
) -> Result<NodeTranslationDto, String> {
    let app = app();
    app.ensure_workspace_dirs()
        .map_err(|error| error.to_string())?;
    let record = find_mod_record(&app, &key)?;
    let extraction_source = extraction_source_for_record(&record);
    let vendor_dir = app.config().vendor_dir.clone();
    if force {
        clear_translation_extract_cache_for_record(&record, &extraction_source, &vendor_dir)?;
    }
    let cache_key = language_cache_key(&record, &extraction_source, &vendor_dir);
    let scan_root = extraction_scan_root(&extraction_source, &cache_key, &vendor_dir)
        .ok_or_else(|| "선택 항목을 분석할 수 없습니다.".to_string())?;
    let manifest = read_mod_manifest_for_record(&record.path, &scan_root);
    let available_languages = language_preview_from_scan_root(&scan_root);
    let settings = read_ui_settings(app.config()).map_err(|error| error.to_string())?;
    let target_language = settings.target_language;
    let mut selected_resource_path = if resource_path.trim().is_empty() {
        default_translation_resource_path(&scan_root, &target_language)
            .or_else(|| default_hardcoded_resource_path(&scan_root))
            .ok_or_else(|| "선택 항목 아래에서 localization 언어 파일이나 DLL 문자열 후보를 찾지 못했습니다.".to_string())?
    } else {
        resource_path.clone()
    };
    let mut selected = selected_translation_files(&scan_root, &selected_resource_path);
    if selected.is_empty()
        && !resource_path.trim().is_empty()
        && let Some(fallback_path) = default_translation_resource_path(&scan_root, &target_language)
    {
        selected = selected_translation_files(&scan_root, &fallback_path);
        if !selected.is_empty() {
            selected_resource_path = fallback_path;
        }
    }
    if selected.is_empty() {
        return Err("선택 항목 아래에서 localization 언어 파일이나 DLL 문자열 후보를 찾지 못했습니다.".to_string());
    }

    let selection_id = stable_resource_id(&selected_resource_path);
    let default_source_root = app
        .config()
        .translation_work_dir
        .join("selected")
        .join(record.stable_key())
        .join(&selection_id)
        .join("source");
    let source_root = output_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| {
            path.join(record.stable_key())
                .join(&selection_id)
                .join("source")
        })
        .unwrap_or(default_source_root);
    let translated_root = source_root
        .parent()
        .map(|parent| parent.join("translated"))
        .unwrap_or_else(|| source_root.join("translated"));
    let sheet_path = app
        .config()
        .translation_work_dir
        .join("translation_memory")
        .join(record.stable_key())
        .join(format!("{selection_id}.{target_language}.translation.json"));
    let memory_keys = translation_memory_candidate_keys(&app, &record, &manifest);
    let existing_sheet_path = select_translation_memory_sheet(
        &app.config().translation_work_dir,
        sheet_path.exists().then(|| sheet_path.clone()),
        &memory_keys,
        &target_language,
        &selected_resource_path,
    );

    if source_root.exists() {
        fs::remove_dir_all(&source_root).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&source_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(&translated_root).map_err(|error| error.to_string())?;

    let mut copied = Vec::new();
    for file in &selected {
        let relative = pck_resource_relative_path(file)
            .or_else(|| file.strip_prefix(&scan_root).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| {
                file.file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("translation.json"))
            });
        let target = source_root.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(file, &target).map_err(|error| error.to_string())?;
        copied.push(target);
    }
    copy_existing_target_language_files(&scan_root, &selected, &translated_root, &target_language)?;
    let connected_patch = copy_connected_translation_patch_files(
        ConnectedTranslationPatchRequest {
            app: &app,
            base_record: &record,
            base_manifest: &manifest,
            source_scan_root: &scan_root,
            source_files: &selected,
            translated_root: &translated_root,
            target_language: &target_language,
            vendor_dir: &vendor_dir,
        },
    )?;
    copied.sort();
    let pck_contents_root = copied
        .first()
        .and_then(|_| {
            selected_translation_files(&scan_root, &selected_resource_path)
                .first()
                .cloned()
        })
        .as_deref()
        .and_then(pck_contents_root_for_path);
    let pck_stem = pck_contents_root
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
        .and_then(|name| name.strip_suffix(".pck.contents"))
        .unwrap_or_default()
        .to_string();
    let can_export_patch_mod = !pck_stem.is_empty()
        || connected_patch
            .apply_target
            .as_ref()
            .and_then(|target| target.pck_stem.as_deref())
            .is_some_and(|target_pck_stem| !target_pck_stem.is_empty());
    let record_key = record.stable_key();
    write_translation_context(TranslationContextWriteRequest {
        work_dir: source_root.parent().unwrap_or(source_root.as_path()),
        mod_key: &record_key,
        resource_path: &selected_resource_path,
        extraction_source: &extraction_source,
        pck_contents_root: pck_contents_root.as_deref(),
        pck_stem: &pck_stem,
        translation_patch_source: connected_patch
            .apply_target
            .as_ref()
            .map(|target| target.source_path.as_path()),
        translation_patch_pck_stem: connected_patch
            .apply_target
            .as_ref()
            .and_then(|target| target.pck_stem.as_deref()),
    })
    .map_err(|error| error.to_string())?;
    let first_source = copied
        .first()
        .cloned()
        .ok_or_else(|| "복사된 언어 파일이 없습니다.".to_string())?;
    let tool_source = if copied.len() == 1 {
        first_source.clone()
    } else {
        source_root.clone()
    };
    let translated_output = if copied.len() == 1 {
        let output_relative = first_source
            .strip_prefix(&source_root)
            .ok()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("translated.json"));
        translated_root.join(output_relative)
    } else {
        translated_root.clone()
    };

    Ok(NodeTranslationDto {
        message: format!(
            "{} 항목 추출 완료: {}개 작업 파일 ({})",
            selected_resource_path,
            copied.len(),
            source_root.display()
        ),
        source_path: display_path(&tool_source),
        existing_sheet_path: existing_sheet_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        output_sheet_path: display_path(&sheet_path),
        translated_output_path: display_path(&translated_output),
        copied_files: copied.len(),
        mod_key: record.stable_key(),
        mod_path: display_path(&record.path),
        mod_name: manifest
            .name
            .unwrap_or_else(|| record.stable_key().to_string()),
        mod_version: manifest.version.unwrap_or_else(|| "-".to_string()),
        mod_author: manifest.author.unwrap_or_default(),
        mod_description: manifest.description.unwrap_or_default(),
        available_languages,
        can_export_patch_mod,
    })
}


