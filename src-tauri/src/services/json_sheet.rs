pub(crate) fn create_json_translation_sheet(
    source_path: String,
    existing_sheet_path: Option<String>,
    output_path: Option<String>,
    target_language: Option<String>,
) -> Result<JsonSheetActionDto, String> {
    let app = app();
    app.ensure_workspace_dirs()
        .map_err(|error| error.to_string())?;
    let settings = read_ui_settings(app.config()).map_err(|error| error.to_string())?;
    let source = PathBuf::from(source_path.trim());
    if source.as_os_str().is_empty() {
        return Err("원본 JSON 경로를 입력하세요.".to_string());
    }

    let language = target_language
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(settings.target_language);
    let output = output_path
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_sheet_path(app.config(), &source, &language));
    let existing = existing_sheet_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    let report = create_or_update_sheet(&source, &language, existing.as_deref(), &output)
        .map_err(|error| error.to_string())?;
    let sheet = read_sheet(&report.sheet_path).map_err(|error| error.to_string())?;

    Ok(JsonSheetActionDto {
        message: format!(
            "{} 번역 시트 생성 완료: {}개 항목, 신규 {}개, 업데이트 {}개",
            if is_hardcoded_source_file(&source) { "DLL" } else { "JSON" },
            report.entries, report.new_entries, report.updated_entries
        ),
        report: json_report_dto(report),
        sheet: json_sheet_dto(sheet),
    })
}

pub(crate) fn load_json_translation_sheet(sheet_path: String) -> Result<JsonSheetDto, String> {
    let path = PathBuf::from(sheet_path.trim());
    let sheet = read_sheet(&path).map_err(|error| error.to_string())?;
    Ok(json_sheet_dto(sheet))
}

pub(crate) fn validate_json_translation_sheet(
    sheet_path: String,
) -> Result<JsonValidationDto, String> {
    let path = PathBuf::from(sheet_path.trim());
    let report = validate_sheet(&path).map_err(|error| error.to_string())?;
    Ok(json_validation_dto(report))
}

pub(crate) fn validate_json_translation_sheet_data(
    sheet: JsonSheetDto,
) -> Result<JsonValidationDto, String> {
    let domain = json_sheet_from_dto(sheet)?;
    Ok(json_validation_dto(validate_translation_sheet(&domain)))
}

pub(crate) fn save_json_translation_sheet(
    sheet_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    let path = PathBuf::from(sheet_path.trim());
    if path.as_os_str().is_empty() {
        return Err("저장할 번역 시트 JSON 경로를 입력하세요.".to_string());
    }
    let domain = json_sheet_from_dto(sheet)?;
    write_sheet(&path, &domain).map_err(|error| error.to_string())?;
    let saved = read_sheet(&path).map_err(|error| error.to_string())?;
    let report = create_report_for_saved_sheet(&path, &saved);
    Ok(JsonSheetActionDto {
        message: format!("번역 시트 저장 완료: {}", path.display()),
        report: json_report_dto(report),
        sheet: json_sheet_dto(saved),
    })
}

pub(crate) fn export_json_translation_csv(
    output_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonCsvExportDto, String> {
    let path = PathBuf::from(output_path.trim());
    if path.as_os_str().is_empty() {
        return Err("CSV 출력 경로를 선택하세요.".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let domain = json_sheet_from_dto(sheet)?;
    let slots = translation_slot_entries(&domain);
    let mut csv = String::from("id,source_value,translated_value,status,file,key\n");
    for slot in &slots {
        let entry = slot.entry;
        let (file, key) = split_translation_key(&entry.key);
        csv.push_str(
            &[
                csv_field(&slot.id),
                csv_field(&entry.source_value),
                csv_field(&entry.translated_value),
                csv_field(status_label(entry.status)),
                csv_field(&file),
                csv_field(&key),
            ]
            .join(","),
        );
        csv.push('\n');
    }
    fs::write(&path, csv).map_err(|error| error.to_string())?;
    Ok(JsonCsvExportDto {
        output_path: display_path(&path),
        rows: slots.len(),
    })
}

pub(crate) fn export_json_translation_short_json(
    output_path: String,
    sheet: JsonSheetDto,
    only_empty: Option<bool>,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    let path = PathBuf::from(output_path.trim());
    if path.as_os_str().is_empty() {
        return Err("번역용 JSON 출력 경로를 선택하세요.".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let domain = json_sheet_from_dto(sheet)?;
    let include_keys = include_keys.map(|keys| keys.into_iter().collect::<BTreeSet<_>>());
    let output = compact_source_translation_map_with_keys(
        &domain,
        only_empty.unwrap_or(false),
        include_keys.as_ref(),
    );
    let rows = output.values().map(BTreeMap::len).sum();
    let content = serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?;
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(ShortJsonExportDto {
        output_path: display_path(&path),
        rows,
    })
}

pub(crate) fn export_json_translation_warning_json(
    output_path: String,
    sheet: JsonSheetDto,
    include_keys: Option<Vec<String>>,
) -> Result<ShortJsonExportDto, String> {
    let path = PathBuf::from(output_path.trim());
    if path.as_os_str().is_empty() {
        return Err("검증 오류 JSON 출력 경로를 선택하세요.".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let domain = json_sheet_from_dto(sheet)?;
    let include_keys = include_keys.map(|keys| keys.into_iter().collect::<BTreeSet<_>>());
    let validation = validate_translation_sheet(&domain);
    let output =
        compact_validation_issue_translation_map_with_keys(&domain, &validation, include_keys.as_ref());
    let rows = output.values().map(BTreeMap::len).sum();
    let content = serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?;
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(ShortJsonExportDto {
        output_path: display_path(&path),
        rows,
    })
}

pub(crate) fn import_json_translation_values(
    input_path: String,
    sheet: JsonSheetDto,
) -> Result<JsonSheetActionDto, String> {
    let path = PathBuf::from(input_path.trim());
    if path.as_os_str().is_empty() {
        return Err("불러올 CSV 또는 JSON 경로를 선택하세요.".to_string());
    }
    let domain = json_sheet_from_dto(sheet)?;
    let (imported, report) =
        import_translations(&domain, &path).map_err(|error| error.to_string())?;
    let action_report = create_report_for_saved_sheet(Path::new("imported"), &imported);
    let import = json_import_dto(report);
    Ok(JsonSheetActionDto {
        message: format!(
            "번역 파일 매칭 완료: {}개 입력 / {}개 미매칭 ({})",
            import.matched_entries, import.unmatched_entries, import.input_path
        ),
        report: json_report_dto(action_report),
        sheet: json_sheet_dto(imported),
    })
}

pub(crate) fn compare_translation_language(
    sheet_path: String,
    sample_path: String,
) -> Result<Vec<LanguageCompareValueDto>, String> {
    let sheet_path = PathBuf::from(sheet_path.trim());
    let sheet = read_sheet(&sheet_path).map_err(|error| error.to_string())?;
    let sample_relative = resource_path_to_relative(sample_path.trim())
        .ok_or_else(|| "비교 언어 경로를 읽지 못했습니다.".to_string())?;
    let compare_language = localization_language_component(&sample_relative)
        .ok_or_else(|| "비교 언어 폴더를 찾지 못했습니다.".to_string())?;
    let scan_root = compare_scan_root(&sheet)
        .ok_or_else(|| "비교할 원본 모드 파일을 찾지 못했습니다.".to_string())?;
    let mut json_cache = BTreeMap::<PathBuf, serde_json::Value>::new();
    let mut values = Vec::new();
    for entry in &sheet.entries {
        let (file, pointer) = split_translation_key(&entry.key);
        if file.is_empty() || pointer.is_empty() {
            continue;
        }
        let relative = replace_resource_language(Path::new(&file), &compare_language)
            .unwrap_or_else(|| PathBuf::from(&file));
        let file_path = scan_root.join(relative);
        if !json_cache.contains_key(&file_path) {
            let content = match fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let json = match serde_json::from_str::<serde_json::Value>(
                content.trim_start_matches('\u{feff}'),
            ) {
                Ok(json) => json,
                Err(_) => continue,
            };
            json_cache.insert(file_path.clone(), json);
        }
        if let Some(value) = json_cache
            .get(&file_path)
            .and_then(|json| json.pointer(&pointer))
            .and_then(serde_json::Value::as_str)
        {
            values.push(LanguageCompareValueDto {
                key: entry.key.clone(),
                value: value.to_string(),
            });
        }
    }
    Ok(values)
}

pub(crate) fn apply_json_translation_sheet(
    sheet_path: String,
    output_path: String,
    pck_target_path: Option<String>,
) -> Result<JsonApplyDto, String> {
    let sheet = PathBuf::from(sheet_path.trim());
    let output = output_path.trim();
    let pck_target = pck_target_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let report = apply_sheet_and_pack_pck(
        &sheet,
        (!output.is_empty()).then(|| PathBuf::from(output)),
        pck_target.as_deref(),
        app().config(),
    )
    .map_err(|error| error.to_string())?;
    if report.applied_entries > 0 {
        record_translation_apply(&sheet, &report, app().config())?;
    }
    Ok(json_apply_dto(report))
}

fn apply_sheet_and_pack_pck(
    sheet_path: &Path,
    requested_output: Option<PathBuf>,
    requested_pck_target: Option<&Path>,
    config: &AppConfig,
) -> Result<PckPatchReport, String> {
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    if is_hardcoded_source_file(Path::new(&sheet.source_path)) {
        let output_path = requested_output
            .clone()
            .unwrap_or_else(|| default_hardcoded_output_path(sheet_path, &sheet));
        let json_report = apply_sheet(sheet_path, &output_path).map_err(|error| error.to_string())?;
        let installed_mod_path = apply_hardcoded_translation_output(&sheet, &output_path, config)?;
        return Ok(PckPatchReport {
            language_output_path: json_report.output_path,
            packed_pck_path: None,
            installed_mod_path,
            applied_entries: json_report.applied_entries,
        });
    }
    let json_report =
        apply_sheet_to_target_language(sheet_path).map_err(|error| error.to_string())?;
    if json_report.applied_entries == 0 {
        return Err("적용할 번역값이 없습니다. translated_value를 입력하거나 CSV/JSON을 먼저 매칭해 주세요.".to_string());
    }
    let language_output_path =
        target_language_output_path(&sheet).unwrap_or_else(|| json_report.output_path.clone());
    if let Some(installed_mod_path) =
        apply_folder_translation_output(&sheet, &language_output_path, config)?
    {
        return Ok(PckPatchReport {
            language_output_path,
            packed_pck_path: None,
            installed_mod_path: Some(installed_mod_path),
            applied_entries: json_report.applied_entries,
        });
    }
    let build_result = match build_translated_pck(
        sheet_path,
        &sheet,
        &language_output_path,
        requested_output.as_deref(),
        requested_pck_target,
        config,
    ) {
        Ok(result) => Some(result),
        Err(error) if should_require_pck_pack(&sheet) => return Err(error),
        Err(error) => {
            eprintln!("PCK repack skipped: {error}");
            None
        }
    };
    if build_result.is_some() {
        if let Err(error) = remember_applied_language_preview(&sheet, &language_output_path, config)
        {
            eprintln!("language preview cache update skipped: {error}");
        }
    }

    Ok(PckPatchReport {
        language_output_path,
        packed_pck_path: build_result
            .as_ref()
            .and_then(|result| result.output_pck_path.clone()),
        installed_mod_path: build_result.and_then(|result| result.installed_mod_path),
        applied_entries: json_report.applied_entries,
    })
}

fn default_hardcoded_output_path(sheet_path: &Path, sheet: &JsonTranslationSheet) -> PathBuf {
    let source_path = PathBuf::from(&sheet.source_path);
    let file_name = source_path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("translated.dll"));
    sheet_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("patched")
        .join(file_name)
}

fn apply_hardcoded_translation_output(
    sheet: &JsonTranslationSheet,
    output_path: &Path,
    config: &AppConfig,
) -> Result<Option<PathBuf>, String> {
    let source_path = PathBuf::from(&sheet.source_path);
    let context = read_translation_context(&source_path).unwrap_or_default();
    let Some((install_root, target_path)) =
        hardcoded_translation_target_path(&context, &source_path, config)
    else {
        return Ok(None);
    };
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    backup_existing_path(&target_path, config)?;
    fs::copy(output_path, &target_path).map_err(|error| error.to_string())?;
    Ok(Some(install_root))
}

fn hardcoded_translation_target_path(
    context: &TranslationContext,
    source_path: &Path,
    config: &AppConfig,
) -> Option<(PathBuf, PathBuf)> {
    let source_root = selected_source_root(source_path)?;
    let source_relative = source_path.strip_prefix(source_root).ok()?;
    let install_root = folder_translation_install_root(context, source_path, config)?;
    Some((install_root.clone(), install_root.join(source_relative)))
}

fn apply_folder_translation_output(
    sheet: &JsonTranslationSheet,
    language_output_path: &Path,
    config: &AppConfig,
) -> Result<Option<PathBuf>, String> {
    let source_path = PathBuf::from(&sheet.source_path);
    let context = read_translation_context(&source_path).unwrap_or_default();
    let Some((install_root, target_path)) =
        folder_translation_target_path(&context, &source_path, sheet, config)
    else {
        return Ok(None);
    };
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    backup_existing_path(&target_path, config)?;
    replace_dir_or_file(language_output_path, &target_path).map_err(|error| error.to_string())?;
    Ok(Some(install_root))
}

fn folder_translation_target_path(
    context: &TranslationContext,
    source_path: &Path,
    sheet: &JsonTranslationSheet,
    config: &AppConfig,
) -> Option<(PathBuf, PathBuf)> {
    if path_inside_pck_contents(source_path)
        || context.input_pck_path.is_some()
        || context.pck_stem.is_some()
    {
        return None;
    }
    let source_root = selected_source_root(source_path)?;
    let source_relative = source_path.strip_prefix(source_root).ok()?;
    let target_relative = replace_resource_language(source_relative, &sheet.target_language)?;
    let install_root = folder_translation_install_root(context, source_path, config)?;
    Some((install_root.clone(), install_root.join(target_relative)))
}

fn folder_translation_install_root(
    context: &TranslationContext,
    source_path: &Path,
    config: &AppConfig,
) -> Option<PathBuf> {
    let mod_key = context
        .mod_key
        .clone()
        .or_else(|| mod_key_from_selected_source(source_path));
    if let Some(mod_key) = mod_key {
        if let Ok(record) = find_mod_record(&app(), &mod_key) {
            let extraction_source = extraction_source_for_record(&record);
            if extraction_source.is_dir() {
                return Some(extraction_source);
            }
            if record.path.is_dir() {
                return Some(record.path);
            }
        }
    }
    let source = context.extraction_source_path.as_ref()?;
    if source.is_dir() {
        return Some(source.clone());
    }
    if is_supported_archive_path(source) {
        return active_mod_install_dir(source, config).or_else(|| {
            source
                .file_stem()
                .or_else(|| source.file_name())
                .map(|name| config.game_mods_dir.join(name))
        });
    }
    None
}

fn record_translation_apply(
    sheet_path: &Path,
    report: &PckPatchReport,
    config: &AppConfig,
) -> Result<(), String> {
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    let Some(mod_key) = mod_key_for_translation_sheet(&sheet) else {
        return Ok(());
    };
    let record = TranslationApplyRecord {
        mod_key,
        target_language: sheet.target_language,
        applied_epoch: epoch_seconds(Some(SystemTime::now())).unwrap_or(0),
        applied_entries: report.applied_entries,
        output_path: report.language_output_path.clone(),
        installed_mod_path: report.installed_mod_path.clone(),
        packed_pck_path: report.packed_pck_path.clone(),
    };
    write_translation_apply_record(config, &record).map_err(|error| error.to_string())
}

fn mod_key_for_translation_sheet(sheet: &JsonTranslationSheet) -> Option<String> {
    let source_path = PathBuf::from(&sheet.source_path);
    read_translation_context(&source_path)
        .and_then(|context| context.mod_key)
        .or_else(|| mod_key_from_selected_source(&source_path))
}

fn read_translation_apply_index(
    config: &AppConfig,
) -> sts2_mod_manager::error::AppResult<BTreeMap<String, TranslationApplyRecord>> {
    let path = translation_apply_history_path(config);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let content = fs::read_to_string(&path)
        .map_err(|source| sts2_mod_manager::error::AppError::io(path.as_path(), source))?;
    let mut records = BTreeMap::<String, TranslationApplyRecord>::new();
    for (index, line) in content.lines().enumerate() {
        if index == 0 && line.starts_with("mod_key\t") {
            continue;
        }
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() != 7 {
            continue;
        }
        let Some(applied_epoch) = parts[2].parse::<u64>().ok() else {
            continue;
        };
        let Some(applied_entries) = parts[3].parse::<usize>().ok() else {
            continue;
        };
        let record = TranslationApplyRecord {
            mod_key: unescape_cache_field(parts[0]),
            target_language: unescape_cache_field(parts[1]),
            applied_epoch,
            applied_entries,
            output_path: PathBuf::from(unescape_cache_field(parts[4])),
            installed_mod_path: optional_unescaped_path(parts[5]),
            packed_pck_path: optional_unescaped_path(parts[6]),
        };
        match records.get(&record.mod_key) {
            Some(existing) if existing.applied_epoch >= record.applied_epoch => {}
            _ => {
                records.insert(record.mod_key.clone(), record);
            }
        }
    }
    Ok(records)
}

fn write_translation_apply_record(
    config: &AppConfig,
    record: &TranslationApplyRecord,
) -> std::io::Result<()> {
    let path = translation_apply_history_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::from(
            "mod_key\ttarget_language\tapplied_epoch\tapplied_entries\toutput_path\tinstalled_mod_path\tpacked_pck_path\n",
        )
    };
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(
        &[
            escape_cache_field(&record.mod_key),
            escape_cache_field(&record.target_language),
            record.applied_epoch.to_string(),
            record.applied_entries.to_string(),
            escape_cache_field(&display_path(&record.output_path)),
            record
                .installed_mod_path
                .as_deref()
                .map(display_path)
                .map(|value| escape_cache_field(&value))
                .unwrap_or_default(),
            record
                .packed_pck_path
                .as_deref()
                .map(display_path)
                .map(|value| escape_cache_field(&value))
                .unwrap_or_default(),
        ]
        .join("\t"),
    );
    output.push('\n');
    fs::write(path, output)
}

fn optional_unescaped_path(value: &str) -> Option<PathBuf> {
    (!value.is_empty()).then(|| PathBuf::from(unescape_cache_field(value)))
}

fn translation_apply_history_path(config: &AppConfig) -> PathBuf {
    config.state_dir.join("translation_apply_history.tsv")
}

fn remember_applied_language_preview(
    sheet: &JsonTranslationSheet,
    language_output_path: &Path,
    config: &AppConfig,
) -> Result<(), String> {
    let source_path = PathBuf::from(&sheet.source_path);
    let context = read_translation_context(&source_path).unwrap_or_default();
    let Some(mod_key) = context
        .mod_key
        .clone()
        .or_else(|| mod_key_from_selected_source(&source_path))
    else {
        return Ok(());
    };
    let summary = app()
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let Some(record) = summary
        .game_mods
        .into_iter()
        .chain(summary.vault_mods)
        .chain(summary.external_manager_mods)
        .find(|record| record.stable_key() == mod_key)
    else {
        return Ok(());
    };
    let extraction_source = extraction_source_for_record(&record);
    let cache_key = language_cache_key(&record, &extraction_source, &config.vendor_dir);
    let mut cache = read_language_preview_cache(config).map_err(|error| error.to_string())?;
    let mut detected = language_preview(&extraction_source, &cache_key, &config.vendor_dir);
    let files = count_files_with_extension(language_output_path, "json").max(1);
    let preview = LanguagePreviewDto {
        code: sheet.target_language.clone(),
        label: language_label(&sheet.target_language).to_string(),
        files,
        keys: count_json_translation_keys(language_output_path),
        sample_path: language_output_relative_to_pck(&source_path, language_output_path)
            .map(|relative| format!("res://{}", slash_path(&relative)))
            .unwrap_or_else(|_| display_path(language_output_path)),
    };
    if let Some(existing) = detected.iter_mut().find(|item| item.code == preview.code) {
        *existing = preview;
    } else {
        detected.push(preview);
    }
    sort_language_previews(&mut detected);
    detected.dedup_by(|left, right| left.code == right.code);
    cache.entries.insert(cache_key, detected);
    cache.dirty = true;
    write_language_preview_cache(config, &cache).map_err(|error| error.to_string())
}

fn count_files_with_extension(root: &Path, extension: &str) -> usize {
    let mut files = Vec::new();
    collect_files_with_extension(root, extension, &mut files);
    files.len()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn default_sheet_path(config: &AppConfig, source: &Path, target_language: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .or_else(|| source.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "translation".to_string())
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    config
        .translation_work_dir
        .join("json_sheets")
        .join(format!("{stem}.{target_language}.translation.json"))
}

fn json_report_dto(report: JsonSheetReport) -> JsonSheetReportDto {
    JsonSheetReportDto {
        sheet_path: display_path(&report.sheet_path),
        entries: report.entries,
        new_entries: report.new_entries,
        updated_entries: report.updated_entries,
        missing_entries: report.missing_entries,
        removed_entries: report.removed_entries,
    }
}

fn create_report_for_saved_sheet(path: &Path, sheet: &JsonTranslationSheet) -> JsonSheetReport {
    let mut new_entries = 0;
    let mut updated_entries = 0;
    let mut missing_entries = 0;
    let mut removed_entries = 0;
    for entry in &sheet.entries {
        let translatable = json_entry_is_translatable(entry);
        match entry.status {
            JsonTranslationStatus::New if translatable => new_entries += 1,
            JsonTranslationStatus::Updated if translatable => updated_entries += 1,
            JsonTranslationStatus::Missing if translatable => missing_entries += 1,
            JsonTranslationStatus::Removed => removed_entries += 1,
            _ => {}
        }
    }
    JsonSheetReport {
        sheet_path: path.to_path_buf(),
        entries: sheet
            .entries
            .iter()
            .filter(|entry| json_entry_is_translatable(entry))
            .count(),
        new_entries,
        updated_entries,
        missing_entries,
        removed_entries,
    }
}

fn json_entry_is_translatable(entry: &JsonTranslationEntry) -> bool {
    entry.status != JsonTranslationStatus::Removed && !entry.source_value.trim().is_empty()
}

fn json_sheet_from_dto(sheet: JsonSheetDto) -> Result<JsonTranslationSheet, String> {
    Ok(JsonTranslationSheet {
        source_path: sheet.source_path,
        target_language: sheet.target_language,
        updated_epoch: epoch_seconds(Some(SystemTime::now())).unwrap_or(sheet.updated_epoch),
        entries: sheet
            .entries
            .into_iter()
            .map(json_entry_from_dto)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn json_entry_from_dto(entry: JsonEntryDto) -> Result<JsonTranslationEntry, String> {
    Ok(JsonTranslationEntry {
        key: entry.key,
        slot_id: entry.slot_id,
        source_value: entry.source_value,
        translated_value: entry.translated_value,
        status: match entry.status.as_str() {
            "new" => JsonTranslationStatus::New,
            "ready" => JsonTranslationStatus::Ready,
            "updated" => JsonTranslationStatus::Updated,
            "missing" => JsonTranslationStatus::Missing,
            "removed" => JsonTranslationStatus::Removed,
            other => return Err(format!("알 수 없는 번역 상태: {other}")),
        },
    })
}

fn json_sheet_dto(sheet: sts2_mod_manager::json_translation::JsonTranslationSheet) -> JsonSheetDto {
    JsonSheetDto {
        source_path: sheet.source_path,
        target_language: sheet.target_language,
        updated_epoch: sheet.updated_epoch,
        entries: sheet.entries.into_iter().map(json_entry_dto).collect(),
    }
}

fn json_entry_dto(entry: JsonTranslationEntry) -> JsonEntryDto {
    JsonEntryDto {
        key: entry.key,
        slot_id: entry.slot_id,
        source_value: entry.source_value,
        translated_value: entry.translated_value,
        status: status_label(entry.status).to_string(),
    }
}

fn split_translation_key(key: &str) -> (String, String) {
    if let Some(rest) = key.strip_prefix("file://") {
        let mut parts = rest.splitn(2, '#');
        let file = parts.next().unwrap_or("source.json").to_string();
        let key = parts.next().unwrap_or("").to_string();
        return (file, key);
    }
    ("source.json".to_string(), key.to_string())
}

fn csv_field(value: &str) -> String {
    if value.contains(|character| matches!(character, ',' | '"' | '\n' | '\r')) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn status_label(status: JsonTranslationStatus) -> &'static str {
    match status {
        JsonTranslationStatus::New => "new",
        JsonTranslationStatus::Ready => "ready",
        JsonTranslationStatus::Updated => "updated",
        JsonTranslationStatus::Missing => "missing",
        JsonTranslationStatus::Removed => "removed",
    }
}

fn json_validation_dto(report: JsonValidationReport) -> JsonValidationDto {
    JsonValidationDto {
        valid: report.valid,
        total_entries: report.total_entries,
        missing_entries: report.missing_entries,
        updated_entries: report.updated_entries,
        removed_entries: report.removed_entries,
        format_issues: report
            .format_issues
            .into_iter()
            .map(|issue| JsonValidationIssueDto {
                key: issue.key,
                kind: issue.kind,
                message: issue.message,
            })
            .collect(),
    }
}

fn json_apply_dto(report: PckPatchReport) -> JsonApplyDto {
    JsonApplyDto {
        output_path: report
            .packed_pck_path
            .as_deref()
            .map(display_path)
            .unwrap_or_else(|| display_path(&report.language_output_path)),
        applied_entries: report.applied_entries,
        language_output_path: display_path(&report.language_output_path),
        packed_pck_path: report
            .packed_pck_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        installed_mod_path: report
            .installed_mod_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
    }
}

fn json_import_dto(report: JsonImportReport) -> JsonImportDto {
    JsonImportDto {
        input_path: display_path(&report.input_path),
        matched_entries: report.matched_entries,
        unmatched_entries: report.unmatched_entries,
    }
}


