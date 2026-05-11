struct PckBuildResult {
    output_pck_path: Option<PathBuf>,
    installed_mod_path: Option<PathBuf>,
}

pub(crate) fn export_translation_patch_mod(
    sheet_path: String,
    output_dir: String,
) -> Result<TranslationPatchExportDto, String> {
    let sheet_path = PathBuf::from(sheet_path.trim());
    if sheet_path.as_os_str().is_empty() {
        return Err("내보낼 번역 시트 경로를 입력하세요.".to_string());
    }
    let output_root = PathBuf::from(output_dir.trim());
    if output_root.as_os_str().is_empty() {
        return Err("번역 모드를 내보낼 폴더를 선택하세요.".to_string());
    }

    let app = app();
    app.ensure_workspace_dirs()
        .map_err(|error| error.to_string())?;
    let config = app.config();
    let json_report =
        apply_sheet_to_target_language(&sheet_path).map_err(|error| error.to_string())?;
    if json_report.applied_entries == 0 {
        return Err("내보낼 번역값이 없습니다. translated_value를 먼저 입력하거나 매칭해 주세요.".to_string());
    }

    let sheet = read_sheet(&sheet_path).map_err(|error| error.to_string())?;
    let source_path = PathBuf::from(&sheet.source_path);
    let language_output_path =
        target_language_output_path(&sheet).unwrap_or_else(|| json_report.output_path.clone());
    let payload_root = translation_patch_payload_root(&source_path, &language_output_path)
        .ok_or_else(|| "번역 모드에 넣을 translated 폴더를 찾지 못했습니다.".to_string())?;
    if !payload_root.is_dir() {
        return Err(format!(
            "번역 모드에 넣을 translated 폴더가 없습니다: {}",
            payload_root.display()
        ));
    }
    let files = count_files_with_extension(&payload_root, "json");
    if files == 0 {
        return Err("번역 모드에 넣을 JSON 파일이 없습니다.".to_string());
    }

    let build_root = sheet_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pck_export")
        .join(timestamp_string());
    fs::create_dir_all(&build_root).map_err(|error| error.to_string())?;
    let _build_cleanup = TempBuildDir::new(build_root.clone());

    let vendor_dir = &config.vendor_dir;
    let pck_tool = embedded_pck_tool(vendor_dir)
        .ok_or_else(|| "GodotPCKExplorer.Console.exe를 찾지 못했습니다.".to_string())?;
    let context = read_translation_context(&source_path).unwrap_or_default();
    let input_pck = resolve_input_pck(&context, &source_path, &build_root, vendor_dir)?;
    let version = pck_version(&pck_tool, &input_pck).unwrap_or_else(|_| "2.4.3.0".to_string());
    let manifest = patch_source_manifest(&context, &input_pck, &build_root, &app, vendor_dir);
    let dependency_id = original_package_id(&manifest, &context, &input_pck);
    let package_id = format!("{}_tr", sanitize_package_id(&dependency_id));
    let export_dir = if output_root
        .file_name()
        .map(|value| value.to_string_lossy().eq_ignore_ascii_case(&package_id))
        .unwrap_or(false)
    {
        output_root
    } else {
        output_root.join(&package_id)
    };
    fs::create_dir_all(&export_dir).map_err(|error| error.to_string())?;

    let payload_build = build_root.join("patch_payload");
    copy_dir_all(&payload_root, &payload_build).map_err(|error| error.to_string())?;
    let temp_pck = build_root.join(format!("{package_id}.pck"));
    run_pck_tool(
        &pck_tool,
        &[
            "-p".into(),
            payload_build,
            temp_pck.clone(),
            PathBuf::from(&version),
        ],
    )?;

    let pck_path = export_dir.join(format!("{package_id}.pck"));
    fs::copy(&temp_pck, &pck_path).map_err(|error| error.to_string())?;
    let languages = translation_payload_languages(&payload_root);
    let dependency_version = manifest.version.clone().unwrap_or_else(|| "-".to_string());
    let package_version = if dependency_version == "-" {
        "1.0.0".to_string()
    } else {
        dependency_version.clone()
    };
    let original_name = manifest
        .name
        .clone()
        .unwrap_or_else(|| dependency_id.clone());
    let manifest_path = export_dir.join(format!("{package_id}.json"));
    let manifest_json = serde_json::json!({
        "id": package_id,
        "name": format!("{original_name} Korean Translation"),
        "author": "STS2 Mod Manager",
        "description": format!("Korean localization patch for {original_name}. Target mod version: {dependency_version}."),
        "is_translation_patch": true,
        "translation_mod": true,
        "version": package_version,
        "translation_patch_version": "1.0.0",
        "has_pck": true,
        "has_dll": false,
        "dependencies": [dependency_id],
        "dependency_versions": { dependency_id.clone(): dependency_version },
        "affects_gameplay": false,
        "target_mod_id": dependency_id,
        "target_mod_name": original_name,
        "target_mod_version": dependency_version,
        "target_languages": languages,
    });
    let manifest_content =
        serde_json::to_string_pretty(&manifest_json).map_err(|error| error.to_string())?;
    fs::write(&manifest_path, manifest_content).map_err(|error| error.to_string())?;
    record_translation_apply(
        &sheet_path,
        &PckPatchReport {
            language_output_path: language_output_path.clone(),
            packed_pck_path: Some(pck_path.clone()),
            installed_mod_path: None,
            applied_entries: json_report.applied_entries,
        },
        config,
    )?;

    Ok(TranslationPatchExportDto {
        output_dir: display_path(&export_dir),
        manifest_path: display_path(&manifest_path),
        pck_path: display_path(&pck_path),
        package_id,
        dependency_id,
        dependency_version,
        languages,
        files,
        applied_entries: json_report.applied_entries,
    })
}

fn should_require_pck_pack(sheet: &JsonTranslationSheet) -> bool {
    let source_path = PathBuf::from(&sheet.source_path);
    path_inside_pck_contents(&source_path)
        || mod_key_from_selected_source(&source_path).is_some()
        || read_translation_context(&source_path)
            .map(|context| {
                context.input_pck_path.is_some()
                    || context.pck_stem.is_some()
                    || context
                        .extraction_source_path
                        .as_deref()
                        .map(is_supported_extractable_path)
                        .unwrap_or(false)
            })
            .unwrap_or(false)
}

fn build_translated_pck(
    sheet_path: &Path,
    sheet: &JsonTranslationSheet,
    language_output_path: &Path,
    requested_output: Option<&Path>,
    requested_pck_target: Option<&Path>,
    config: &AppConfig,
) -> Result<PckBuildResult, String> {
    let vendor_dir = &config.vendor_dir;
    let pck_tool = embedded_pck_tool(vendor_dir)
        .ok_or_else(|| "GodotPCKExplorer.Console.exe를 찾지 못했습니다.".to_string())?;
    let source_path = PathBuf::from(&sheet.source_path);
    let context = read_translation_context(&source_path).unwrap_or_default();
    let build_root = sheet_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pck_build")
        .join(timestamp_string());
    fs::create_dir_all(&build_root).map_err(|error| error.to_string())?;
    let _build_cleanup = TempBuildDir::new(build_root.clone());

    let input_pck = resolve_input_pck(&context, &source_path, &build_root, vendor_dir)?;
    let version = pck_version(&pck_tool, &input_pck).unwrap_or_else(|_| "2.4.3.0".to_string());
    let full_extract = build_root.join("full_extract");
    run_pck_tool(
        &pck_tool,
        &["-e".into(), input_pck.clone(), full_extract.clone()],
    )?;

    let target_relative =
        pck_target_for_language_output(&source_path, language_output_path, requested_pck_target)?;
    let target_in_extract = full_extract.join(&target_relative);
    replace_dir_or_file(language_output_path, &target_in_extract)
        .map_err(|error| error.to_string())?;

    let temp_output = build_root.join(
        input_pck
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("translated.pck")),
    );
    run_pck_tool(
        &pck_tool,
        &[
            "-p".into(),
            full_extract,
            temp_output.clone(),
            PathBuf::from(&version),
        ],
    )?;
    let output_pck = if should_persist_pck_output(requested_output, config) {
        let output_pck = translated_pck_output_path(requested_output, sheet_path, &input_pck)?;
        if let Some(parent) = output_pck.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&temp_output, &output_pck).map_err(|error| error.to_string())?;
        Some(output_pck)
    } else {
        None
    };
    let install_source = output_pck.as_deref().unwrap_or(temp_output.as_path());
    let installed_mod_path = install_patched_archive_mod(
        &context,
        &build_root.join("archive"),
        &input_pck,
        install_source,
        config,
    )?;
    Ok(PckBuildResult {
        output_pck_path: output_pck,
        installed_mod_path,
    })
}

struct TempBuildDir {
    path: PathBuf,
}

impl TempBuildDir {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempBuildDir {
    fn drop(&mut self) {
        if self.path.is_dir() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn install_patched_archive_mod(
    context: &TranslationContext,
    archive_dir: &Path,
    input_pck: &Path,
    patched_pck: &Path,
    config: &AppConfig,
) -> Result<Option<PathBuf>, String> {
    let Some(source) = context.extraction_source_path.as_ref() else {
        return Ok(None);
    };
    if is_pck_path(source) && source.exists() {
        backup_existing_path(source, config)?;
        fs::copy(patched_pck, source).map_err(|error| error.to_string())?;
        return Ok(Some(source.clone()));
    }
    if !is_supported_archive_path(source)
        || !archive_dir.is_dir()
        || !input_pck.starts_with(archive_dir)
    {
        return Ok(None);
    }
    fs::copy(patched_pck, input_pck).map_err(|error| error.to_string())?;
    let target_dir = active_mod_install_dir(source, config).unwrap_or_else(|| {
        let target_name = source
            .file_stem()
            .or_else(|| source.file_name())
            .unwrap_or_else(|| std::ffi::OsStr::new("translated_mod"));
        config.game_mods_dir.join(target_name)
    });
    backup_existing_path(&target_dir, config)?;
    if source.parent() == Some(config.game_mods_dir.as_path()) {
        backup_existing_path(source, config)?;
    }
    copy_dir_all(archive_dir, &target_dir).map_err(|error| error.to_string())?;
    Ok(Some(target_dir))
}


fn embedded_pck_tool(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir
        .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
        .join("GodotPCKExplorer.Console.exe");
    path.is_file().then_some(path)
}

fn run_pck_tool(tool: &Path, args: &[PathBuf]) -> Result<String, String> {
    let output = hidden_command(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("PCK 도구 실행 실패: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\0', "");
    let stderr = String::from_utf8_lossy(&output.stderr).replace('\0', "");
    if output.status.success() {
        Ok(format!("{stdout}\n{stderr}"))
    } else {
        Err(format!("PCK 도구 실패: {stdout}\n{stderr}"))
    }
}

fn pck_version(tool: &Path, pck: &Path) -> Result<String, String> {
    let output = run_pck_tool(tool, &["-l".into(), pck.to_path_buf()])?;
    for line in output.lines() {
        let line = line.trim();
        for marker in ["Version string for this program:", "Version:"] {
            if let Some(rest) = line.split_once(marker).map(|(_, rest)| rest.trim()) {
                let version = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|character: char| {
                        !character.is_ascii_digit() && character != '.'
                    });
                if version.matches('.').count() == 3 {
                    return Ok(version.to_string());
                }
            }
        }
    }
    Err("PCK 버전을 읽지 못했습니다.".to_string())
}

fn resolve_input_pck(
    context: &TranslationContext,
    source_path: &Path,
    build_root: &Path,
    vendor_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(path) = context
        .input_pck_path
        .as_ref()
        .filter(|path| path.is_file())
    {
        return Ok(path.clone());
    }
    if let Some(path) = source_path_inside_pck(source_path).filter(|path| path.is_file()) {
        return Ok(path);
    }
    if let Some(source) = context
        .extraction_source_path
        .as_ref()
        .filter(|path| path.exists())
    {
        return pck_from_extractable_source(
            source,
            context.pck_stem.as_deref(),
            build_root,
            vendor_dir,
        );
    }
    if let Some(mod_key) = context
        .mod_key
        .clone()
        .or_else(|| mod_key_from_selected_source(source_path))
    {
        let vault_root = resolve_workspace_dir().join("vault").join(mod_key);
        if let Some(source) = preferred_vault_source(&vault_root) {
            return pck_from_extractable_source(
                &source,
                context.pck_stem.as_deref(),
                build_root,
                vendor_dir,
            );
        }
    }
    Err("원본 PCK를 찾지 못했습니다.".to_string())
}

fn pck_from_extractable_source(
    source: &Path,
    preferred_stem: Option<&str>,
    build_root: &Path,
    vendor_dir: &Path,
) -> Result<PathBuf, String> {
    if source
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pck"))
        .unwrap_or(false)
    {
        return Ok(source.to_path_buf());
    }
    let archive_dir = build_root.join("archive");
    extract_archive_for_pck(source, &archive_dir, vendor_dir)?;
    let mut pcks = Vec::new();
    collect_files_with_extension(&archive_dir, "pck", &mut pcks);
    pcks.sort();
    if let Some(stem) = preferred_stem {
        if let Some(path) = pcks.iter().find(|path| {
            path.file_stem()
                .map(|value| value.to_string_lossy().eq_ignore_ascii_case(stem))
                .unwrap_or(false)
        }) {
            return Ok(path.clone());
        }
    }
    pcks.into_iter()
        .next()
        .ok_or_else(|| format!("압축 내부에서 PCK를 찾지 못했습니다: {}", source.display()))
}

fn extract_archive_for_pck(
    source: &Path,
    destination: &Path,
    vendor_dir: &Path,
) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let seven_zip = vendor_dir.join("7zip").join("7z.exe");
    if seven_zip.is_file() {
        let status = hidden_command(&seven_zip)
            .arg("x")
            .arg("-y")
            .arg(format!("-o{}", destination.to_string_lossy()))
            .arg(source)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            return Ok(());
        }
    }
    Err(format!("압축 해제 실패: {}", source.display()))
}

fn collect_files_with_extension(root: &Path, extension: &str, output: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(root) else {
        return;
    };
    if metadata.is_file() {
        if root
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            output.push(root.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_files_with_extension(&entry.path(), extension, output);
    }
}

fn source_path_inside_pck(source_path: &Path) -> Option<PathBuf> {
    let root = pck_contents_root_for_path(source_path)?;
    let name = root.file_name()?.to_string_lossy();
    let stem = name.strip_suffix(".pck.contents")?;
    let pck = root.parent()?.join(format!("{stem}.pck"));
    pck.is_file().then_some(pck)
}

fn preferred_vault_source(vault_root: &Path) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_preferred_vault_sources(vault_root, &mut files);
    files.sort();
    files.into_iter().next()
}

fn collect_preferred_vault_sources(path: &Path, output: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        if path
            .file_name()
            .map(|value| value.to_string_lossy().eq_ignore_ascii_case("disabled"))
            .unwrap_or(false)
        {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            collect_preferred_vault_sources(&entry.path(), output);
        }
        return;
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "pck" | "rar" | "zip" | "7z"
            )
        })
        .unwrap_or(false)
    {
        output.push(path.to_path_buf());
    }
}

fn language_output_relative_to_pck(
    source_path: &Path,
    language_output_path: &Path,
) -> Result<PathBuf, String> {
    if let Some(relative) = pck_resource_relative_path(language_output_path) {
        return Ok(relative);
    }
    if let Some(relative) = relative_after_named_component(language_output_path, "translated") {
        return Ok(relative);
    }
    if let Some(source_root) = selected_source_root(source_path) {
        return language_output_path
            .strip_prefix(source_root)
            .map(Path::to_path_buf)
            .map_err(|_| "PCK 내부 언어 경로를 계산하지 못했습니다.".to_string());
    }
    Err("PCK 내부 언어 경로를 계산하지 못했습니다.".to_string())
}

fn pck_target_for_language_output(
    source_path: &Path,
    language_output_path: &Path,
    requested_pck_target: Option<&Path>,
) -> Result<PathBuf, String> {
    let target_relative = if let Some(target) = requested_pck_target {
        normalize_pck_target_path(target)?
    } else {
        language_output_relative_to_pck(source_path, language_output_path)?
    };
    Ok(adjust_pck_target_for_language_output(
        language_output_path,
        target_relative,
    ))
}

fn adjust_pck_target_for_language_output(
    language_output_path: &Path,
    target_relative: PathBuf,
) -> PathBuf {
    let output_is_dir = language_output_path.is_dir() || language_output_path.extension().is_none();
    let target_looks_file = target_relative.extension().is_some();
    if output_is_dir && target_looks_file {
        return target_relative
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(target_relative);
    }
    if !output_is_dir && !target_looks_file {
        if let Some(file_name) = language_output_path.file_name() {
            return target_relative.join(file_name);
        }
    }
    target_relative
}

fn normalize_pck_target_path(path: &Path) -> Result<PathBuf, String> {
    let text = path.to_string_lossy().replace('\\', "/");
    let trimmed = text
        .strip_prefix("res://")
        .unwrap_or(&text)
        .trim()
        .trim_start_matches('/');
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err("PCK 내부 삽입 경로가 올바르지 않습니다.".to_string());
    }
    Ok(PathBuf::from(trimmed))
}

fn relative_after_named_component(path: &Path, component_name: &str) -> Option<PathBuf> {
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case(component_name))?;
    if index + 1 >= parts.len() {
        return None;
    }
    let mut output = PathBuf::new();
    for part in &parts[index + 1..] {
        output.push(part);
    }
    Some(output)
}

fn selected_source_root(source_path: &Path) -> Option<&Path> {
    for ancestor in source_path.ancestors() {
        if ancestor
            .file_name()
            .map(|value| value.to_string_lossy().eq_ignore_ascii_case("source"))
            .unwrap_or(false)
        {
            return Some(ancestor);
        }
    }
    None
}

fn translated_pck_output_path(
    requested_output: Option<&Path>,
    sheet_path: &Path,
    input_pck: &Path,
) -> Result<PathBuf, String> {
    let file_name = input_pck
        .file_name()
        .ok_or_else(|| "PCK 파일명이 없습니다.".to_string())?;
    if let Some(path) = requested_output {
        if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("pck"))
            .unwrap_or(false)
        {
            return Ok(path.to_path_buf());
        }
        return Ok(path.join(file_name));
    }
    Ok(sheet_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("patched")
        .join(file_name))
}

fn should_persist_pck_output(requested_output: Option<&Path>, config: &AppConfig) -> bool {
    let Some(path) = requested_output else {
        return true;
    };
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pck"))
        .unwrap_or(false)
    {
        return true;
    }
    !path.starts_with(config.translation_work_dir.join("selected"))
}

fn translation_patch_payload_root(
    source_path: &Path,
    language_output_path: &Path,
) -> Option<PathBuf> {
    source_path
        .ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .map(|value| value.to_string_lossy().eq_ignore_ascii_case("source"))
                .unwrap_or(false)
        })
        .and_then(|source_root| source_root.parent().map(|parent| parent.join("translated")))
        .or_else(|| root_through_named_component(language_output_path, "translated"))
        .or_else(|| {
            if language_output_path.is_dir() {
                Some(language_output_path.to_path_buf())
            } else {
                language_output_path.parent().map(Path::to_path_buf)
            }
        })
}

fn root_through_named_component(path: &Path, component_name: &str) -> Option<PathBuf> {
    let mut output = PathBuf::new();
    for component in path.components() {
        output.push(component.as_os_str());
        if component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(component_name)
        {
            return Some(output);
        }
    }
    None
}

fn translation_payload_languages(payload_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_files_with_extension(payload_root, "json", &mut files);
    let mut languages = BTreeSet::new();
    for file in files {
        if let Some(language) = localization_language_component(&file) {
            languages.insert(language);
        }
    }
    languages.into_iter().collect()
}

fn patch_source_manifest(
    context: &TranslationContext,
    input_pck: &Path,
    build_root: &Path,
    app: &App,
    vendor_dir: &Path,
) -> ModManifestInfo {
    if let Some(mod_key) = context.mod_key.as_deref() {
        if let Ok(record) = find_mod_record(app, mod_key) {
            let extraction_source = extraction_source_for_record(&record);
            let cache_key = language_cache_key(&record, &extraction_source, vendor_dir);
            let scan_root = extraction_scan_root(&extraction_source, &cache_key, vendor_dir)
                .unwrap_or_else(|| extraction_source.clone());
            let info = read_mod_manifest_for_record(&record.path, &scan_root);
            if manifest_has_identity(&info) {
                return info;
            }
        }
    }
    let mut roots = Vec::new();
    if let Some(source) = context.extraction_source_path.as_ref() {
        if source.is_dir() {
            roots.push(source.clone());
        } else if is_supported_archive_path(source) {
            roots.push(build_root.join("archive"));
        } else if let Some(parent) = source.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    if let Some(parent) = input_pck.parent() {
        roots.push(parent.to_path_buf());
    }
    roots.sort();
    roots.dedup();
    for root in roots {
        let info = read_mod_manifest_info(&root);
        if manifest_has_identity(&info) {
            return info;
        }
    }
    ModManifestInfo::default()
}

fn manifest_has_identity(info: &ModManifestInfo) -> bool {
    info.id.is_some()
        || info.name.is_some()
        || info.version.is_some()
        || !info.dependencies.is_empty()
}

fn original_package_id(
    manifest: &ModManifestInfo,
    context: &TranslationContext,
    input_pck: &Path,
) -> String {
    manifest
        .id
        .clone()
        .or_else(|| context.pck_stem.clone())
        .or_else(|| {
            input_pck
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
        })
        .or_else(|| context.mod_key.clone())
        .unwrap_or_else(|| "translation_patch".to_string())
}

fn sanitize_package_id(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            output.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator {
            output.push('_');
            previous_was_separator = true;
        }
    }
    let trimmed = output.trim_matches(['_', '-']).to_string();
    if trimmed.is_empty() {
        "translation_patch".to_string()
    } else {
        trimmed
    }
}

#[derive(Default)]
struct TranslationContext {
    mod_key: Option<String>,
    extraction_source_path: Option<PathBuf>,
    input_pck_path: Option<PathBuf>,
    pck_stem: Option<String>,
}

fn write_translation_context(
    work_dir: &Path,
    mod_key: &str,
    resource_path: &str,
    extraction_source: &Path,
    pck_contents_root: Option<&Path>,
    pck_stem: &str,
) -> std::io::Result<()> {
    let path = work_dir.join("translation_context.tsv");
    let mut content = String::new();
    content.push_str(&format!("mod_key\t{mod_key}\n"));
    content.push_str(&format!("resource_path\t{resource_path}\n"));
    content.push_str(&format!(
        "extraction_source_path\t{}\n",
        extraction_source.display()
    ));
    if let Some(root) = pck_contents_root {
        content.push_str(&format!("pck_contents_root\t{}\n", root.display()));
    }
    if !pck_stem.is_empty() {
        content.push_str(&format!("pck_stem\t{pck_stem}\n"));
    }
    fs::write(path, content)
}

fn read_translation_context(source_path: &Path) -> Option<TranslationContext> {
    let source_root = selected_source_root(source_path)?;
    let path = source_root.parent()?.join("translation_context.tsv");
    let content = fs::read_to_string(path).ok()?;
    let mut context = TranslationContext::default();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        let value = value.trim();
        match key {
            "mod_key" if !value.is_empty() => context.mod_key = Some(value.to_string()),
            "extraction_source_path" if !value.is_empty() => {
                context.extraction_source_path = Some(PathBuf::from(value))
            }
            "input_pck_path" if !value.is_empty() => {
                context.input_pck_path = Some(PathBuf::from(value))
            }
            "pck_stem" if !value.is_empty() => context.pck_stem = Some(value.to_string()),
            _ => {}
        }
    }
    Some(context)
}

fn mod_key_from_selected_source(source_path: &Path) -> Option<String> {
    let components = source_path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    components
        .windows(2)
        .find_map(|window| (window[0].eq_ignore_ascii_case("selected")).then(|| window[1].clone()))
}


