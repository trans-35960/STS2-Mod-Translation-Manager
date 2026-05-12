pub(crate) fn load_dashboard() -> Result<DashboardDto, String> {
    dashboard().map_err(|error| error.to_string())
}


pub(crate) fn scan_updates() -> Result<ActionDto, String> {
    let app = app();
    let report = app
        .scan_and_update_state()
        .map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!("업데이트 스캔 완료: {}개 변경 감지", report.changes.len()),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn repair_mod_installations() -> Result<ActionDto, String> {
    let app = app();
    let config = app.config().clone();
    let report = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?;
    let mut unpacked = 0usize;
    let mut flattened = 0usize;
    let mut skipped = Vec::new();

    for record in report.summary.game_mods {
        if record.kind == ModKind::Archive && is_supported_archive_path(&record.path) {
            match repair_archive_mod(&record.path, &config) {
                Ok(true) => unpacked += 1,
                Ok(false) => skipped.push(record.name.clone()),
                Err(error) => skipped.push(format!("{} ({error})", record.name)),
            }
            continue;
        }
        if record.kind == ModKind::Directory {
            if is_vortex_nested_mod_layout(&record.path) {
                continue;
            }
            match repair_nested_mod_folder(&record.path) {
                Ok(true) => flattened += 1,
                Ok(false) => {}
                Err(error) => skipped.push(format!("{} ({error})", record.name)),
            }
        }
    }

    let mut parts = vec![format!(
        "설치 정리 완료: 압축 해제 {}개, 중첩 폴더 보정 {}개",
        unpacked, flattened
    )];
    if !skipped.is_empty() {
        parts.push(format!(
            "건너뜀 {}개: {}",
            skipped.len(),
            skipped.join(", ")
        ));
    }
    Ok(ActionDto {
        message: parts.join(" / "),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn open_path(path: String) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    if !path.exists() {
        if let Some(parent) = nearest_existing_parent(&path) {
            open_path_in_system(&parent)?;
            return Ok(());
        }
        return Err(format!("경로를 찾을 수 없습니다: {}", path.display()));
    }

    open_path_in_system(&path)
}

pub(crate) fn preview_dropped_mods(paths: Vec<String>) -> Result<Vec<DroppedModPreviewDto>, String> {
    let mut previews = Vec::new();
    let mut skipped = Vec::new();
    let config = app().config().clone();

    for value in paths {
        let path = PathBuf::from(value.trim());
        match dropped_mod_candidates(&path, &config) {
            Ok(candidates) => previews.extend(
                candidates
                    .into_iter()
                    .map(dropped_mod_preview_dto),
            ),
            Err(error) => skipped.push(format!("{} ({error})", path.display())),
        }
    }

    if previews.is_empty() {
        if skipped.is_empty() {
            return Err("드롭한 항목을 찾지 못했습니다.".to_string());
        }
        return Err(format!(
            "모드 폴더나 지원 압축파일을 찾지 못했습니다: {}",
            skipped.join(", ")
        ));
    }

    Ok(previews)
}

pub(crate) fn import_dropped_mod(
    path: String,
    replace_path: Option<String>,
) -> Result<ActionDto, String> {
    let source = PathBuf::from(path.trim());
    let record = dropped_mod_record(&source)?;
    let app = app();
    let drop_import_root = drop_import_root_for_path(&source, app.config());

    if let Some(replace_path) = replace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let target = PathBuf::from(replace_path);
        replace_existing_mod_path(&source, &target, app.config())?;
        cleanup_drop_import_root(drop_import_root.as_deref());
        return Ok(ActionDto {
            message: format!("{} 덮어쓰기 완료", record.name),
            dashboard: dashboard().map_err(|error| error.to_string())?,
        });
    }

    let action = app
        .import_mod_as_new(&source)
        .map_err(|error| error.to_string())?;
    cleanup_drop_import_root(drop_import_root.as_deref());
    Ok(ActionDto {
        message: format!("{} 새 모드 등록 완료: {}", record.name, action.to.display()),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn toggle_mod(key: String, active: bool, force: bool) -> Result<ActionDto, String> {
    let started = Instant::now();
    let app = app();
    let changed = apply_mod_toggles(
        &app,
        vec![ModToggleDto {
            key: key.clone(),
            active,
            force: Some(force),
        }],
    )?;
    let apply_ms = started.elapsed().as_millis();
    let dashboard_started = Instant::now();
    let dashboard = dashboard().map_err(|error| error.to_string())?;
    let dashboard_ms = dashboard_started.elapsed().as_millis();

    Ok(ActionDto {
        message: if active {
            format!(
                "{key} 비활성화 선택 완료. 다음 모드 실행 때 적용됩니다 ({changed}개, 적용 {apply_ms}ms, 새로고침 {dashboard_ms}ms)"
            )
        } else {
            format!(
                "{key} 활성화 선택 완료. 다음 모드 실행 때 적용됩니다 ({changed}개, 적용 {apply_ms}ms, 새로고침 {dashboard_ms}ms)"
            )
        },
        dashboard,
    })
}

pub(crate) fn toggle_mods(changes: Vec<ModToggleDto>) -> Result<ActionDto, String> {
    let started = Instant::now();
    let app = app();
    let changed = apply_mod_toggles(&app, changes)?;
    let apply_ms = started.elapsed().as_millis();
    let dashboard_started = Instant::now();
    let dashboard = dashboard().map_err(|error| error.to_string())?;
    let dashboard_ms = dashboard_started.elapsed().as_millis();

    Ok(ActionDto {
        message: format!(
            "모드 전환 선택 완료: {changed}개. 다음 모드 실행 때 적용됩니다 (적용 {apply_ms}ms, 새로고침 {dashboard_ms}ms)"
        ),
        dashboard,
    })
}

fn apply_mod_toggles(app: &App, changes: Vec<ModToggleDto>) -> Result<usize, String> {
    let mut target_active_by_key = BTreeMap::<String, bool>::new();
    let mut force_by_key = BTreeMap::<String, bool>::new();
    for change in changes {
        target_active_by_key.insert(change.key.clone(), !change.active);
        force_by_key.insert(change.key, change.force.unwrap_or(false));
    }
    if target_active_by_key.is_empty() {
        return Ok(0);
    }

    let enable_keys = target_active_by_key
        .iter()
        .filter_map(|(key, active)| (*active).then_some(key.clone()))
        .collect::<BTreeSet<_>>();

    if !enable_keys.is_empty() {
        let report = app
            .scan_preview_report()
            .map_err(|error| error.to_string())?;
        forget_revived_deleted_mod_tombstones(app.config(), &report.summary)?;
        let deleted_keys = deleted_mod_keys_for_summary(app.config(), &report.summary);
        for key in &enable_keys {
            if deleted_keys.contains(key) {
                return Err(format!(
                    "{key} 모드는 삭제된 항목입니다. 설정에서 복원한 뒤 활성화해 주세요."
                ));
            }
        }

        let rows = mod_rows(
            &report,
            app.config(),
            &deleted_keys,
            game_updated_epoch(&app.launch_status(), app.config()),
        )
        .map_err(|error| error.to_string())?;

        for key in &enable_keys {
            let row = rows
                .iter()
                .find(|row| &row.key == key)
                .ok_or_else(|| format!("{key} 모드를 찾을 수 없습니다."))?;
            let force = force_by_key.get(key).copied().unwrap_or(false);
            for dependency in &row.dependencies {
                if !dependency.available {
                    return Err(format!("선행 모드가 없습니다: {}", dependency.id));
                }
                let dependency_will_be_active = dependency
                    .key
                    .as_ref()
                    .is_some_and(|dependency_key| enable_keys.contains(dependency_key));
                if !dependency.active && !dependency_will_be_active && !force {
                    return Err(format!(
                        "선행 모드가 비활성화되어 있습니다: {}",
                        dependency.name
                    ));
                }
            }
        }
    }

    let desired_changes = target_active_by_key.into_iter().collect::<Vec<_>>();
    let changed = desired_changes.len();
    app.set_mods_desired_active(&desired_changes)
        .map_err(|error| error.to_string())?;
    Ok(changed)
}

struct DroppedModCandidate {
    record: ModRecord,
    display_path: String,
}

fn dropped_mod_candidates(path: &Path, config: &AppConfig) -> Result<Vec<DroppedModCandidate>, String> {
    if !path.exists() {
        return Err(format!("경로를 찾을 수 없습니다: {}", path.display()));
    }

    if path.is_dir() {
        let nested = split_dropped_directory(path)?;
        if nested.len() > 1 {
            return Ok(nested
                .into_iter()
                .map(|record| DroppedModCandidate {
                    display_path: container_child_display_path(path, &record.path),
                    record,
                })
                .collect());
        }
    } else if is_supported_drop_archive_path(path) {
        if let Some(extract_dir) = extract_dropped_archive(path, config) {
            let nested = split_dropped_directory(&extract_dir)?;
            if nested.len() > 1 {
                return Ok(nested
                    .into_iter()
                    .map(|record| DroppedModCandidate {
                        display_path: container_child_display_path(path, &record.path),
                        record,
                    })
                    .collect());
            }
            if nested.len() == 1 {
                let record = dropped_mod_record(path)?;
                return Ok(vec![DroppedModCandidate {
                    display_path: display_path(&record.path),
                    record,
                }]);
            }
        }
    }

    let record = dropped_mod_record(path)?;
    Ok(vec![DroppedModCandidate {
        display_path: display_path(&record.path),
        record,
    }])
}

fn dropped_mod_record(path: &Path) -> Result<ModRecord, String> {
    if !path.exists() {
        return Err(format!("경로를 찾을 수 없습니다: {}", path.display()));
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("경로 정보를 읽을 수 없습니다: {} ({error})", path.display()))?;
    let kind = classify_dropped_mod_path(path, metadata.is_dir());
    if !is_supported_dropped_mod_kind(kind) {
        return Err(format!("지원하지 않는 파일 형식입니다: {}", path.display()));
    }
    let fingerprint = if metadata.is_dir() {
        fingerprint_dropped_directory(path)?
    } else {
        ModFingerprint {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        }
    };
    Ok(ModRecord {
        version_hint: infer_dropped_version_hint(&dropped_mod_display_name(path, kind)),
        name: dropped_mod_display_name(path, kind),
        path: path.to_path_buf(),
        source: ModSource::Vault,
        kind,
        fingerprint,
    })
}

fn classify_dropped_mod_path(path: &Path, is_dir: bool) -> ModKind {
    if is_dir {
        return ModKind::Directory;
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("zip" | "7z" | "rar") => ModKind::Archive,
        Some("jar" | "pck" | "pak") => ModKind::Package,
        _ => ModKind::UnknownFile,
    }
}

fn dropped_mod_display_name(path: &Path, kind: ModKind) -> String {
    let raw_name = if kind == ModKind::Directory {
        path.file_name()
    } else {
        path.file_stem().or_else(|| path.file_name())
    };
    raw_name
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown-mod".to_string())
}

fn fingerprint_dropped_directory(path: &Path) -> Result<ModFingerprint, String> {
    let mut bytes = 0;
    let mut modified: Option<SystemTime> = None;
    collect_dropped_directory_fingerprint(path, &mut bytes, &mut modified)?;
    Ok(ModFingerprint { bytes, modified })
}

fn collect_dropped_directory_fingerprint(
    path: &Path,
    bytes: &mut u64,
    modified: &mut Option<SystemTime>,
) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("폴더를 읽을 수 없습니다: {} ({error})", path.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let entry_path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_dropped_directory_fingerprint(&entry_path, bytes, modified)?;
        } else {
            *bytes += metadata.len();
            if let Ok(entry_modified) = metadata.modified() {
                if modified.is_none_or(|current| entry_modified > current) {
                    *modified = Some(entry_modified);
                }
            }
        }
    }
    Ok(())
}

fn infer_dropped_version_hint(name: &str) -> Option<String> {
    name.split(['-', '_', ' ', '[', ']'])
        .find(|part| {
            let lower = part.to_ascii_lowercase();
            lower.starts_with('v') && lower.chars().skip(1).any(|character| character.is_ascii_digit())
        })
        .map(|part| part.trim().to_string())
}

fn split_dropped_directory(path: &Path) -> Result<Vec<ModRecord>, String> {
    let mut current = path.to_path_buf();
    for _ in 0..3 {
        if dropped_directory_is_mod_root(&current) {
            return Ok(vec![dropped_mod_record(&current)?]);
        }
        let records = supported_records_in_directory(&current)?;
        if records.len() > 1 {
            return Ok(records);
        }
        let Some(only) = records.into_iter().next() else {
            return Ok(Vec::new());
        };
        if only.kind != ModKind::Directory {
            return Ok(vec![only]);
        }
        current = only.path;
    }

    supported_records_in_directory(&current)
}

fn dropped_directory_is_mod_root(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        if !path.is_file() {
            return false;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        if matches!(extension.as_deref(), Some("dll" | "pck" | "pak" | "jar")) {
            return true;
        }
        if extension.as_deref() != Some("json") {
            return false;
        }
        path.file_stem()
            .and_then(|value| value.to_str())
            .map(|name| {
                let lower = name.to_ascii_lowercase();
                matches!(lower.as_str(), "manifest" | "mod" | "plugin")
                    || path
                        .parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|value| value.to_str())
                        .is_some_and(|folder| folder.eq_ignore_ascii_case(name))
            })
            .unwrap_or(false)
    })
}

fn supported_records_in_directory(path: &Path) -> Result<Vec<ModRecord>, String> {
    Ok(scan_mod_directory(path, ModSource::Vault)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|record| is_supported_dropped_mod_kind(record.kind))
        .collect())
}

fn is_supported_dropped_mod_kind(kind: ModKind) -> bool {
    matches!(kind, ModKind::Directory | ModKind::Archive | ModKind::Package)
}

fn dropped_mod_preview_dto(candidate: DroppedModCandidate) -> DroppedModPreviewDto {
    let record = candidate.record;
    DroppedModPreviewDto {
        path: display_path(&record.path),
        display_path: candidate.display_path,
        key: record.stable_key(),
        name: record.name,
        kind: kind_label(record.kind).to_string(),
        version_hint: record.version_hint,
        bytes: record.fingerprint.bytes,
        modified_epoch: epoch_seconds(record.fingerprint.modified),
    }
}

fn is_supported_drop_archive_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "zip" | "7z" | "rar"))
        .unwrap_or(false)
}

fn extract_dropped_archive(path: &Path, config: &AppConfig) -> Option<PathBuf> {
    let extract_dir = drop_import_extract_dir(path, config);
    if extract_dir.exists() {
        return Some(extract_dir);
    }
    fs::create_dir_all(&extract_dir).ok()?;
    let expanded = if let Some(seven_zip) = drop_embedded_7z_path(&config.vendor_dir) {
        drop_expand_with_7z(&seven_zip, path, &extract_dir)
    } else if drop_is_zip_archive(path) {
        expand_zip_archive_for_preview(path, &extract_dir)
    } else {
        false
    };
    if expanded {
        Some(extract_dir)
    } else {
        let _ = fs::remove_dir_all(&extract_dir);
        None
    }
}

fn drop_is_zip_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
}

fn drop_import_extract_dir(path: &Path, config: &AppConfig) -> PathBuf {
    let cache_key = fs::metadata(path)
        .ok()
        .map(|metadata| {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            format!("{}:{}:{modified}", path.to_string_lossy(), metadata.len())
        })
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    config
        .state_dir
        .join("drop_imports")
        .join(format!("drop-{:016x}", drop_stable_hash(&cache_key)))
}

fn drop_import_root_for_path(path: &Path, config: &AppConfig) -> Option<PathBuf> {
    let root = config.state_dir.join("drop_imports");
    let relative = path.strip_prefix(&root).ok()?;
    let first = relative.components().next()?;
    Some(root.join(first.as_os_str()))
}

fn cleanup_drop_import_root(path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    let _ = remove_path_if_exists(path);
}

fn drop_embedded_7z_path(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir.join("7zip").join("7z.exe");
    path.exists().then_some(path)
}

fn drop_expand_with_7z(seven_zip: &Path, source: &Path, destination: &Path) -> bool {
    hidden_command(seven_zip)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", destination.to_string_lossy()))
        .arg(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn expand_zip_archive_for_preview(source: &Path, destination: &Path) -> bool {
    let command = format!(
        "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        drop_powershell_quote(source),
        drop_powershell_quote(destination)
    );
    hidden_command("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn drop_powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn container_child_display_path(container: &Path, child: &Path) -> String {
    let container_name = container
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| display_path(container));
    let child_name = child
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| display_path(child));
    format!("{container_name} > {child_name}")
}

fn drop_stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn replace_existing_mod_path(
    source: &Path,
    target: &Path,
    config: &AppConfig,
) -> Result<(), String> {
    if !target.exists() {
        return Err(format!("덮어쓸 모드를 찾을 수 없습니다: {}", target.display()));
    }
    if same_path(source, target) {
        return Err("같은 경로로는 덮어쓸 수 없습니다.".to_string());
    }
    if source.starts_with(&config.vault_dir)
        || source.starts_with(&config.game_mods_dir)
        || source.starts_with(game_disabled_dir(&config.game_mods_dir))
    {
        return Err("이미 관리 중인 모드는 드래그 등록 대상으로 사용할 수 없습니다.".to_string());
    }
    if !is_managed_mod_path(target, config) {
        return Err(format!(
            "관리 중인 모드만 덮어쓸 수 있습니다: {}",
            target.display()
        ));
    }

    if let Some(vault_entry_dir) = vault_entry_root_for_path(target, &config.vault_dir) {
        let action = App::new(config.clone())
            .import_mod_as_new(source)
            .map_err(|error| error.to_string())?;
        if !action.to.exists() {
            return Err(format!("모드 등록 결과를 확인할 수 없습니다: {}", action.to.display()));
        }
        remove_path_if_exists(&vault_entry_dir).map_err(|error| {
            format!(
                "기존 vault 모드 제거 실패: {} ({error})",
                vault_entry_dir.display()
            )
        })?;
        return Ok(());
    }

    let target_parent = target
        .parent()
        .ok_or_else(|| format!("대상 상위 폴더를 읽을 수 없습니다: {}", target.display()))?;
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("파일 이름을 읽을 수 없습니다: {}", source.display()))?;
    let replacement_target = target_parent.join(file_name);
    if replacement_target.exists() && !same_path(&replacement_target, target) {
        return Err(format!(
            "새 버전 이름의 대상이 이미 존재합니다: {}",
            replacement_target.display()
        ));
    }
    if same_path(&replacement_target, target) {
        let staged_target = staged_replacement_path(target_parent, file_name);
        copy_dropped_mod(source, &staged_target).map_err(|error| {
            format!(
                "모드 임시 복사 실패: {} -> {} ({error})",
                source.display(),
                staged_target.display()
            )
        })?;
        remove_path_if_exists(target)
            .map_err(|error| format!("기존 모드 제거 실패: {} ({error})", target.display()))?;
        move_path_or_copy(&staged_target, &replacement_target).map_err(|error| {
            format!(
                "모드 교체 실패: {} -> {} ({error})",
                staged_target.display(),
                replacement_target.display()
            )
        })?;
        return Ok(());
    }

    copy_dropped_mod(source, &replacement_target).map_err(|error| {
        format!(
            "새 모드 복사 실패: {} -> {} ({error})",
            source.display(),
            replacement_target.display()
        )
    })?;
    remove_path_if_exists(target)
        .map_err(|error| format!("기존 모드 제거 실패: {} ({error})", target.display()))
}

fn vault_entry_root_for_path(path: &Path, vault_dir: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(vault_dir).ok()?;
    let first = relative.components().next()?;
    Some(vault_dir.join(first.as_os_str()))
}

fn copy_dropped_mod(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        copy_dir_all(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        Ok(())
    }
}

fn staged_replacement_path(parent: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let stamp = timestamp_string();
    for index in 1..1000 {
        let candidate = parent.join(format!(
            ".drop-import-{stamp}-{index}-{}",
            file_name.to_string_lossy()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!(".drop-import-{stamp}-{}", file_name.to_string_lossy()))
}


pub(crate) fn save_preset(name: String) -> Result<ActionDto, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("프리셋 이름을 입력하세요.".to_string());
    }

    let preset = app()
        .save_preset(trimmed)
        .map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!("프리셋 '{}' 저장 완료", preset.name),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn apply_preset(name: String) -> Result<ActionDto, String> {
    let report = app()
        .apply_preset(&name)
        .map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: preset_apply_message(&name, &report),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn export_preset(name: String, archive_path: String) -> Result<ActionDto, String> {
    let archive = PathBuf::from(archive_path.trim());
    if archive.as_os_str().is_empty() {
        return Err("내보낼 ZIP 경로를 입력하세요.".to_string());
    }

    let report = app()
        .export_preset(&name, &archive)
        .map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!(
            "'{}' 프리셋 내보내기 완료: {}개 모드 포함 ({})",
            name,
            report.included_mods,
            report.archive_path.display()
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn import_preset_archive(archive_path: String) -> Result<ActionDto, String> {
    let archive = PathBuf::from(archive_path.trim());
    if archive.as_os_str().is_empty() {
        return Err("불러올 ZIP 경로를 입력하세요.".to_string());
    }

    let report = app()
        .import_preset_archive(&archive)
        .map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!(
            "'{}' 프리셋 불러오기 완료: {}개 모드 등록",
            report.preset.name, report.imported_mods
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn launch_current() -> Result<ActionDto, String> {
    let report = app().launch_current().map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: launch_message("게임 실행 완료", &report),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn launch_vanilla() -> Result<ActionDto, String> {
    let report = app().launch_vanilla().map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!(
            "{}. 선택한 모드는 다음 모드 실행 때 다시 적용됩니다.",
            launch_message("바닐라 모드로 게임 실행 완료", &report)
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

fn launch_message(prefix: &str, report: &LaunchReport) -> String {
    let mut parts = vec![format!("{prefix}: pid {}", report.process_id)];
    if report.save_backups_created > 0 {
        parts.push(format!("세이브 백업 {}개", report.save_backups_created));
    }
    if report.seeded_modded_profiles > 0 {
        parts.push(format!(
            "modded 프로필 {}개 초기 복사",
            report.seeded_modded_profiles
        ));
    }
    if let Some(warning) = &report.save_backup_warning {
        parts.push(format!("백업 건너뜀: {warning}"));
    }
    parts.join(" · ")
}

pub(crate) fn create_save_backup() -> Result<ActionDto, String> {
    let config = configured_config();
    let report =
        save_backup::backup_before_launch(&config, true).map_err(|error| error.to_string())?;
    let message = if let Some(reason) = report.skipped_reason {
        reason
    } else {
        format!(
            "세이브 백업 완료: {}개 생성, modded 프로필 {}개 준비",
            report.created.len(),
            report.seeded_modded_profiles
        )
    };
    Ok(ActionDto {
        message,
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn clear_current_runs() -> Result<ActionDto, String> {
    let config = configured_config();
    let report = save_backup::clear_current_runs_for_mode_switch(&config)
        .map_err(|error| current_run_cleanup_error_message(&error.to_string()))?;
    let message = if !report.remaining_files.is_empty() {
        format!(
            "진행 중 런 정리를 시도했지만 아직 파일이 남아 있습니다. Steam Cloud 동기화가 다시 내려받았거나 파일이 잠겨 있을 수 있습니다. Steam 동기화가 끝난 뒤 다시 시도하고, 계속 남으면 Steam을 잠깐 종료해 주세요. 이후 Steam에 '동기화 불가'가 남으면 게임을 한 번 정상 종료하거나 Steam을 재시작한 뒤, 충돌 창에서는 로컬 파일을 선택하세요. 남은 경로: {}",
            report
                .remaining_files
                .iter()
                .take(3)
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(" / ")
        )
    } else if report.cleared_files.is_empty() && report.cleaned_cloud_caches.is_empty() {
        "정리할 진행 중 런(current_run.save)을 찾지 못했습니다.".to_string()
    } else {
        format!(
            "진행 중 런 정리 완료: 파일 {}개 백업 후 제거, Steam Cloud 캐시 {}개 정리. Steam에 '동기화 불가'가 남으면 게임을 한 번 정상 종료하거나 Steam을 재시작한 뒤, 충돌 창에서는 로컬 파일을 선택하세요.",
            report.cleared_files.len(),
            report.cleaned_cloud_caches.len()
        )
    };
    Ok(ActionDto {
        message,
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

fn current_run_cleanup_error_message(error: &str) -> String {
    format!(
        "진행 중 런 정리에 실패했습니다. Steam Cloud가 current_run.save 또는 remotecache.vdf를 동기화 중이거나 잠그고 있을 수 있습니다. Steam의 클라우드 동기화가 끝날 때까지 기다린 뒤 다시 시도해 주세요. 계속 실패하면 Steam을 잠깐 종료한 뒤 다시 정리해 주세요. 정리 후 Steam에 '동기화 불가'가 남으면 게임을 한 번 정상 종료하거나 Steam을 재시작하고, 충돌 창에서는 로컬 파일을 선택하세요.\n원인: {error}"
    )
}

pub(crate) fn restore_save_backup(id: String) -> Result<ActionDto, String> {
    let config = configured_config();
    let restored =
        save_backup::restore_backup(&config, id.trim()).map_err(|error| error.to_string())?;
    Ok(ActionDto {
        message: format!(
            "{} 세이브 복원 완료: {}",
            restored.kind.label(),
            restored.path.display()
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn delete_save_backups(ids: Vec<String>) -> Result<ActionDto, String> {
    let config = configured_config();
    let mut deleted = Vec::new();
    let mut seen = BTreeSet::new();
    for id in ids {
        let trimmed = id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        let entry =
            save_backup::delete_backup(&config, trimmed).map_err(|error| error.to_string())?;
        deleted.push(format!(
            "{} {}",
            entry.kind.label(),
            entry.created_epoch
        ));
    }
    if deleted.is_empty() {
        return Err("삭제할 세이브 백업을 선택하세요.".to_string());
    }
    Ok(ActionDto {
        message: format!("세이브 백업 삭제 완료: {}", deleted.join(", ")),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}


fn dashboard() -> sts2_mod_manager::error::AppResult<DashboardDto> {
    let app = app();
    app.ensure_workspace_dirs()?;
    let settings = read_ui_settings(app.config())?;
    prune_expired_deleted_mods(app.config(), settings.deleted_retention_days)
        .map_err(sts2_mod_manager::error::AppError::InvalidCommand)?;
    quarantine_reappeared_deleted_mods(app.config())
        .map_err(sts2_mod_manager::error::AppError::InvalidCommand)?;
    let mut report = app.scan_preview_report()?;
    if auto_repair_active_mod_installations(app.config(), &report.summary) {
        report = app.scan_preview_report()?;
    }
    let presets = app.list_presets()?;
    let translations = app.list_translation_workspaces()?;
    let tools = app.vendor_tools();
    let launch = app.launch_status();
    let setup_issues = setup_issues(app.config(), &settings, &launch);
    let game_updated_epoch = game_updated_epoch(&launch, app.config());
    forget_revived_deleted_mod_tombstones(app.config(), &report.summary)
        .map_err(sts2_mod_manager::error::AppError::InvalidCommand)?;
    prune_deleted_desired_mod_keys(app.config(), &report.summary)
        .map_err(sts2_mod_manager::error::AppError::InvalidCommand)?;
    let deleted_keys = deleted_mod_keys_for_summary(app.config(), &report.summary);
    let mods = mod_rows(&report, app.config(), &deleted_keys, game_updated_epoch)?;
    let diagnostics = troubleshoot_diagnostics(
        app.config(),
        &settings,
        &launch,
        &report.summary,
        &mods,
        &setup_issues,
    );

    let stats = StatsDto {
        active_mods: mods.iter().filter(|row| row.active).count(),
        inactive_mods: mods.iter().filter(|row| !row.active).count(),
        vault_mods: mods.iter().filter(|row| row.managed).count(),
        external_mods: mods.iter().filter(|row| row.external).count(),
        presets: presets.len(),
        translations: translations.len(),
        detected_changes: mods
            .iter()
            .filter(|row| row.update_state != "clean")
            .count(),
        vanilla_safe: !mods.iter().any(|row| row.active),
    };
    let deleted_mods = read_deleted_mod_entries(app.config())
        .map_err(|source| {
            sts2_mod_manager::error::AppError::io(
                deleted_mod_index_path(app.config()).as_path(),
                source,
            )
        })?
        .into_iter()
        .map(|entry| deleted_mod_dto(entry, settings.deleted_retention_days))
        .collect();
    let save_backups = save_backup::list_backups(app.config())?
        .into_iter()
        .map(save_backup_dto)
        .collect();
    let cache_usage = work_cache_usage(app.config());

    Ok(DashboardDto {
        paths: paths_dto(app.config()),
        settings,
        stats,
        setup_issues,
        diagnostics,
        mods,
        presets: presets.into_iter().map(preset_dto).collect(),
        translations: translations.into_iter().map(translation_dto).collect(),
        deleted_mods,
        save_backups,
        cache_usage,
        tools: tools.into_iter().map(tool_dto).collect(),
        launch: launch_dto(launch),
    })
}

fn auto_repair_active_mod_installations(config: &AppConfig, summary: &ScanSummary) -> bool {
    let mut changed = false;
    for record in &summary.game_mods {
        if record.kind == ModKind::Archive && is_supported_archive_path(&record.path) {
            match repair_archive_mod(&record.path, config) {
                Ok(true) => changed = true,
                Ok(false) => {}
                Err(error) => {
                    eprintln!("active archive repair skipped: {} ({error})", record.path.display())
                }
            }
            continue;
        }
        if record.kind == ModKind::Directory && !is_vortex_nested_mod_layout(&record.path) {
            match repair_nested_mod_folder(&record.path) {
                Ok(true) => changed = true,
                Ok(false) => {}
                Err(error) => {
                    eprintln!("nested mod repair skipped: {} ({error})", record.path.display())
                }
            }
        }
    }
    changed
}
