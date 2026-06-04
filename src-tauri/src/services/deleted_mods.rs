pub(crate) fn delete_mod(key: String, path: String) -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    let report = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?;
    let deleted_epoch = epoch_seconds(Some(SystemTime::now())).unwrap_or(0);
    let deleted = delete_mod_entry(
        ModDeleteDto { key, path },
        report.summary,
        deleted_epoch,
        config,
    )?;

    Ok(ActionDto {
        message: format!(
            "{} 삭제 완료: 백업 위치 {}",
            deleted.name,
            deleted.backup_path.display()
        ),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

pub(crate) fn delete_mods(items: Vec<ModDeleteDto>) -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    if items.is_empty() {
        return Err("삭제할 모드를 선택하세요.".to_string());
    }
    let report = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?;
    let summary = report.summary;
    let deleted_epoch = epoch_seconds(Some(SystemTime::now())).unwrap_or(0);
    let mut deleted = Vec::new();
    let mut failed = Vec::new();

    for item in items {
        match delete_mod_entry(item.clone(), summary.clone(), deleted_epoch, config) {
            Ok(entry) => deleted.push(entry.name),
            Err(error) => failed.push(format!("{} ({error})", item.key)),
        }
    }

    if deleted.is_empty() {
        return Err(format!("선택한 모드 삭제 실패: {}", failed.join(", ")));
    }

    Ok(ActionDto {
        message: if failed.is_empty() {
            format!("선택 모드 삭제 완료: {}개 ({})", deleted.len(), deleted.join(", "))
        } else {
            format!(
                "선택 모드 일부 삭제 완료: 성공 {}개 ({}) / 실패 {}개 ({})",
                deleted.len(),
                deleted.join(", "),
                failed.len(),
                failed.join(", ")
            )
        },
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

fn delete_mod_entry(
    item: ModDeleteDto,
    summary: ScanSummary,
    deleted_epoch: u64,
    config: &AppConfig,
) -> Result<DeletedModEntry, String> {
    let key = item.key.trim();
    let requested_path = PathBuf::from(item.path.trim());
    if key.is_empty() {
        return Err("삭제할 모드 키가 없습니다.".to_string());
    }
    if requested_path.as_os_str().is_empty() {
        return Err("삭제할 모드 경로가 없습니다.".to_string());
    }
    let record = summary
        .game_mods
        .into_iter()
        .chain(summary.disabled_mods)
        .chain(summary.external_manager_mods)
        .find(|record| record.stable_key() == key && same_path(&record.path, &requested_path))
        .ok_or_else(|| format!("{key} 모드를 현재 목록에서 찾지 못했습니다."))?;
    ensure_existing_deletable_mod_path(&record.path, config)?;
    let backup_path = move_mod_to_deleted_backup(&record.path, key, deleted_epoch, config)?;
    let entry = DeletedModEntry {
        id: deleted_mod_id(deleted_epoch, key),
        key: key.to_string(),
        name: record.name.clone(),
        original_path: record.path.clone(),
        backup_path: backup_path.clone(),
        deleted_epoch,
        bytes: record.fingerprint.bytes,
    };
    upsert_deleted_mod_entry(config, entry.clone())?;
    Ok(entry)
}

pub(crate) fn restore_deleted_mod(id: String) -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    let id = id.trim();
    let entry = read_deleted_mod_entries(config)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("복원할 삭제 항목을 찾지 못했습니다: {id}"))?;
    if !entry.backup_path.exists() {
        return Err(format!(
            "백업 파일을 찾을 수 없습니다: {}",
            entry.backup_path.display()
        ));
    }
    ensure_existing_state_path(&entry.backup_path, config, "삭제 백업 경로")?;
    ensure_deletable_mod_path(&entry.original_path, config)?;
    let target = restore_deleted_mod_entry(&entry, config)?;
    cleanup_deleted_backup_parent(&entry.backup_path);
    remove_deleted_mod_entry(config, &entry.id)?;
    remove_deleted_mod_tombstone(config, &entry.key)?;

    Ok(ActionDto {
        message: format!("{} 복원 완료: {}", entry.name, target.display()),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}

fn restore_deleted_mod_entry(entry: &DeletedModEntry, config: &AppConfig) -> Result<PathBuf, String> {
    ensure_existing_state_path(&entry.backup_path, config, "삭제 백업 경로")?;
    ensure_deletable_mod_path(&entry.original_path, config)?;
    if should_expand_restored_archive(entry, config) {
        return restore_deleted_archive_entry(entry, config);
    }
    if let Some(parent) = entry.original_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let target = unique_restore_target(&entry.original_path)?;
    move_path_or_copy(&entry.backup_path, &target).map_err(|error| {
        format!(
            "삭제된 모드 복원 실패: {} -> {} ({error})",
            entry.backup_path.display(),
            target.display()
        )
    })?;
    Ok(target)
}

fn should_expand_restored_archive(entry: &DeletedModEntry, config: &AppConfig) -> bool {
    entry.original_path.starts_with(&config.game_mods_dir)
        && entry.backup_path.is_file()
        && is_supported_archive_path(&entry.backup_path)
}

fn restore_deleted_archive_entry(
    entry: &DeletedModEntry,
    config: &AppConfig,
) -> Result<PathBuf, String> {
    let target = restored_archive_install_dir(&entry.original_path, config)
        .ok_or_else(|| format!("복원 위치를 계산하지 못했습니다: {}", entry.original_path.display()))?;
    let target = unique_restore_target(&target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if !expand_archive(&entry.backup_path, &target, &config.vendor_dir) {
        let _ = remove_path_if_exists(&target);
        return Err(format!(
            "삭제된 압축 모드 복원 실패: {}",
            entry.backup_path.display()
        ));
    }
    if target.is_dir() {
        repair_nested_mod_folder(&target)?;
    }
    remove_path_if_exists(&entry.backup_path).map_err(|error| error.to_string())?;
    Ok(target)
}

fn restored_archive_install_dir(original_path: &Path, config: &AppConfig) -> Option<PathBuf> {
    let relative = original_path.strip_prefix(&config.game_mods_dir).ok()?;
    let mut components = relative.components();
    let first = components.next()?.as_os_str();
    if components.next().is_some() {
        return Some(config.game_mods_dir.join(first));
    }
    let stem = Path::new(first).file_stem()?.to_os_string();
    Some(config.game_mods_dir.join(stem))
}

fn unique_restore_target(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("복원 위치를 계산하지 못했습니다: {}", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("복원 이름을 계산하지 못했습니다: {}", path.display()))?;
    Ok(unique_backup_path(parent, file_name))
}

pub(crate) fn empty_deleted_mods() -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    let entries = read_deleted_mod_entries(config).map_err(|error| error.to_string())?;
    for entry in &entries {
        remember_deleted_mod_tombstone(config, &entry.key)?;
    }
    let deleted_root = deleted_mods_dir(config);
    if deleted_root.exists() {
        ensure_existing_state_path(&deleted_root, config, "삭제 모드 저장소")?;
        fs::remove_dir_all(&deleted_root).map_err(|error| error.to_string())?;
    }
    write_deleted_mod_entries(config, &[]).map_err(|error| error.to_string())?;

    Ok(ActionDto {
        message: format!("최근 삭제 항목 비우기 완료: {}개", entries.len()),
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}


fn active_mod_install_dir(source: &Path, config: &AppConfig) -> Option<PathBuf> {
    let relative = source.strip_prefix(&config.game_mods_dir).ok()?;
    let mut components = relative.components();
    let first = components.next()?.as_os_str().to_os_string();
    if components.next().is_some() {
        return Some(config.game_mods_dir.join(first));
    }
    if source.is_file() {
        let stem = Path::new(&first).file_stem()?.to_os_string();
        return Some(config.game_mods_dir.join(stem));
    }
    Some(config.game_mods_dir.join(first))
}

fn backup_existing_path(path: &Path, config: &AppConfig) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("백업할 경로 이름이 없습니다: {}", path.display()))?;
    let backup_dir = config
        .state_dir
        .join("applied_backups")
        .join(timestamp_string());
    fs::create_dir_all(&backup_dir).map_err(|error| error.to_string())?;
    let backup_path = unique_backup_path(&backup_dir, file_name);
    move_path_or_copy(path, &backup_path).map_err(|error| {
        format!(
            "기존 모드 백업 실패: {} -> {} ({error})",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(Some(backup_path))
}

fn unique_backup_path(parent: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let base = PathBuf::from(file_name);
    let candidate = parent.join(&base);
    if !candidate.exists() {
        return candidate;
    }

    let stem = base
        .file_stem()
        .unwrap_or(file_name)
        .to_string_lossy()
        .to_string();
    let extension = base
        .extension()
        .map(|value| value.to_string_lossy().to_string());
    for index in 1.. {
        let name = match &extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn move_mod_to_deleted_backup(
    path: &Path,
    key: &str,
    deleted_epoch: u64,
    config: &AppConfig,
) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!(
            "삭제할 모드 경로를 찾을 수 없습니다: {}",
            path.display()
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("삭제할 모드 이름을 계산하지 못했습니다: {}", path.display()))?;
    let backup_dir = config
        .state_dir
        .join("deleted_mods")
        .join(deleted_mod_id(deleted_epoch, key));
    ensure_state_path(&backup_dir, config, "삭제 백업 경로")?;
    fs::create_dir_all(&backup_dir).map_err(|error| error.to_string())?;
    let backup_path = unique_backup_path(&backup_dir, file_name);
    move_path_or_copy(path, &backup_path).map_err(|error| {
        format!(
            "모드 삭제 백업 이동 실패: {} -> {} ({error})",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(backup_path)
}

fn quarantine_reappeared_deleted_mods(config: &AppConfig) -> Result<usize, String> {
    let entries = read_deleted_mod_entries(config).map_err(|error| error.to_string())?;
    let mut moved = 0usize;
    for entry in entries {
        if !entry.original_path.starts_with(&config.game_mods_dir) || !entry.original_path.exists()
        {
            continue;
        }
        let parent = entry
            .backup_path
            .parent()
            .ok_or_else(|| format!("삭제 백업 위치를 계산하지 못했습니다: {}", entry.backup_path.display()))?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let file_name = entry
            .original_path
            .file_name()
            .ok_or_else(|| format!("격리할 모드 이름을 계산하지 못했습니다: {}", entry.original_path.display()))?;
        let target = unique_backup_path(parent, file_name);
        move_path_or_copy(&entry.original_path, &target).map_err(|error| {
            format!(
                "삭제된 모드 재등장 격리 실패: {} -> {} ({error})",
                entry.original_path.display(),
                target.display()
            )
        })?;
        moved += 1;
    }
    Ok(moved)
}

fn repair_archive_mod(path: &Path, config: &AppConfig) -> Result<bool, String> {
    let stem = path
        .file_stem()
        .ok_or_else(|| format!("압축 파일 이름을 계산하지 못했습니다: {}", path.display()))?;
    let target = config.game_mods_dir.join(stem);
    if target.exists() {
        remove_repaired_archive(path)?;
        return Ok(true);
    }
    if !expand_archive(path, &target, &config.vendor_dir) {
        let _ = remove_path_if_exists(&target);
        return Err(
            "압축 해제에 실패했습니다. 7-Zip 내장 도구와 파일 손상 여부를 확인하세요.".to_string(),
        );
    }
    if repair_multi_mod_archive_folder(&target, config)? {
        remove_repaired_archive(path)?;
        return Ok(true);
    }
    repair_nested_mod_folder(&target)?;

    remove_repaired_archive(path)?;
    Ok(true)
}

fn remove_repaired_archive(path: &Path) -> Result<(), String> {
    remove_path_if_exists(path)
        .map_err(|error| format!("원본 압축 제거 실패: {} ({error})", path.display()))?;
    Ok(())
}

fn repair_multi_mod_archive_folder(path: &Path, config: &AppConfig) -> Result<bool, String> {
    let records = split_dropped_directory(path)?;
    if records.len() <= 1 {
        return Ok(false);
    }

    let mut moves = Vec::<(PathBuf, PathBuf)>::new();
    for record in records {
        let file_name = record
            .path
            .file_name()
            .ok_or_else(|| format!("모드 폴더 이름을 계산하지 못했습니다: {}", record.path.display()))?;
        let target = config.game_mods_dir.join(file_name);
        if target.exists() {
            return Err(format!("압축 내부 모드 대상이 이미 존재합니다: {}", target.display()));
        }
        moves.push((record.path, target));
    }

    for (source, target) in moves {
        move_path_or_copy(&source, &target).map_err(|error| {
            format!(
                "압축 내부 모드 이동 실패: {} -> {} ({error})",
                source.display(),
                target.display()
            )
        })?;
    }

    fs::remove_dir_all(path)
        .map_err(|error| format!("압축 외부 폴더 제거 실패: {} ({error})", path.display()))?;
    Ok(true)
}

fn repair_nested_mod_folder(path: &Path) -> Result<bool, String> {
    let Some(inner) = nested_mod_payload_dir(path) else {
        return Ok(false);
    };
    let parent = path
        .parent()
        .ok_or_else(|| format!("상위 폴더를 찾지 못했습니다: {}", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("모드 폴더 이름을 계산하지 못했습니다: {}", path.display()))?;
    let temp = unique_repair_sibling_path(parent, file_name);
    move_path_or_copy(&inner, &temp).map_err(|error| {
        format!(
            "내부 모드 폴더 이동 실패: {} -> {} ({error})",
            inner.display(),
            temp.display()
        )
    })?;
    fs::remove_dir_all(path)
        .map_err(|error| format!("빈 외부 폴더 제거 실패: {} ({error})", path.display()))?;
    move_path_or_copy(&temp, path).map_err(|error| {
        format!(
            "보정된 모드 폴더 복원 실패: {} -> {} ({error})",
            temp.display(),
            path.display()
        )
    })?;
    Ok(true)
}

fn unique_repair_sibling_path(parent: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let stem = file_name.to_string_lossy();
    for index in 0.. {
        let name = if index == 0 {
            format!("{stem}.repairing")
        } else {
            format!("{stem}.repairing-{index}")
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn deleted_mod_id(epoch: u64, key: &str) -> String {
    format!("{epoch}-{}", safe_file_stem(key))
}

fn safe_file_stem(value: &str) -> String {
    let sanitized = value
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
    if sanitized.is_empty() {
        "mod".to_string()
    } else {
        sanitized
    }
}

fn deleted_mods_dir(config: &AppConfig) -> PathBuf {
    config.state_dir.join("deleted_mods")
}

fn deleted_mod_index_path(config: &AppConfig) -> PathBuf {
    config.state_dir.join("deleted_mods.tsv")
}

fn deleted_mod_tombstones_path(config: &AppConfig) -> PathBuf {
    config.state_dir.join("deleted_mod_tombstones.tsv")
}

fn read_deleted_mod_entries(config: &AppConfig) -> std::io::Result<Vec<DeletedModEntry>> {
    let path = deleted_mod_index_path(config);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if index == 0 && line.starts_with("id\t") {
            continue;
        }
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() != 7 {
            continue;
        }
        let Some(deleted_epoch) = parts[4].parse::<u64>().ok() else {
            continue;
        };
        let Some(bytes) = parts[6].parse::<u64>().ok() else {
            continue;
        };
        let entry = DeletedModEntry {
            id: unescape_cache_field(parts[0]),
            key: unescape_cache_field(parts[1]),
            name: unescape_cache_field(parts[2]),
            original_path: PathBuf::from(unescape_cache_field(parts[3])),
            deleted_epoch,
            backup_path: PathBuf::from(unescape_cache_field(parts[5])),
            bytes,
        };
        if entry.backup_path.exists() {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| right.deleted_epoch.cmp(&left.deleted_epoch));
    Ok(entries)
}

fn write_deleted_mod_entries(
    config: &AppConfig,
    entries: &[DeletedModEntry],
) -> std::io::Result<()> {
    let path = deleted_mod_index_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output =
        String::from("id\tkey\tname\toriginal_path\tdeleted_epoch\tbackup_path\tbytes\n");
    for entry in entries {
        output.push_str(
            &[
                escape_cache_field(&entry.id),
                escape_cache_field(&entry.key),
                escape_cache_field(&entry.name),
                escape_cache_field(&display_path(&entry.original_path)),
                entry.deleted_epoch.to_string(),
                escape_cache_field(&display_path(&entry.backup_path)),
                entry.bytes.to_string(),
            ]
            .join("\t"),
        );
        output.push('\n');
    }
    fs::write(path, output)
}

fn upsert_deleted_mod_entry(config: &AppConfig, entry: DeletedModEntry) -> Result<(), String> {
    let mut entries = read_deleted_mod_entries(config).map_err(|error| error.to_string())?;
    remove_deleted_mod_tombstone(config, &entry.key)?;
    entries.retain(|existing| {
        let replace = existing.id == entry.id || existing.key == entry.key;
        if replace
            && ensure_existing_state_path(&existing.backup_path, config, "삭제 백업 경로").is_ok()
        {
            let _ = remove_path_if_exists(&existing.backup_path);
            cleanup_deleted_backup_parent(&existing.backup_path);
        }
        !replace
    });
    entries.push(entry);
    write_deleted_mod_entries(config, &entries).map_err(|error| error.to_string())
}

fn remove_deleted_mod_entry(config: &AppConfig, id: &str) -> Result<(), String> {
    let mut entries = read_deleted_mod_entries(config).map_err(|error| error.to_string())?;
    entries.retain(|entry| entry.id != id);
    write_deleted_mod_entries(config, &entries).map_err(|error| error.to_string())
}

#[cfg(test)]
fn deleted_mod_keys(config: &AppConfig) -> BTreeSet<String> {
    let mut keys = read_deleted_mod_entries(config)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.key)
        .collect::<BTreeSet<_>>();
    keys.extend(read_deleted_mod_tombstones(config));
    keys
}

fn deleted_mod_keys_for_summary(config: &AppConfig, summary: &ScanSummary) -> BTreeSet<String> {
    let connected_keys = connected_mod_keys(summary);
    let reinstalled_local_keys = reinstalled_local_deleted_mod_keys(config, summary);
    let mut keys = read_deleted_mod_entries(config)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.key)
        .filter(|key| !reinstalled_local_keys.contains(key))
        .collect::<BTreeSet<_>>();
    keys.extend(
        read_deleted_mod_tombstones(config)
            .into_iter()
            .filter(|key| !connected_keys.contains(key)),
    );
    keys
}

fn reinstalled_local_deleted_mod_keys(
    config: &AppConfig,
    summary: &ScanSummary,
) -> BTreeSet<String> {
    let local_keys = summary
        .game_mods
        .iter()
        .chain(summary.disabled_mods.iter())
        .map(|record| record.stable_key())
        .collect::<BTreeSet<_>>();
    if local_keys.is_empty() {
        return BTreeSet::new();
    }
    read_deleted_mod_entries(config)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| local_keys.contains(&entry.key).then_some(entry.key))
        .collect()
}

fn prune_deleted_desired_mod_keys(
    config: &AppConfig,
    summary: &ScanSummary,
) -> Result<usize, String> {
    let deleted = deleted_mod_keys_for_summary(config, summary);
    if deleted.is_empty() {
        return Ok(0);
    }
    let mut desired = desired_active_mod_keys(summary, &config.state_dir)
        .map_err(|error| error.to_string())?;
    let before = desired.len();
    desired.retain(|key| !deleted.contains(key));
    let removed = before.saturating_sub(desired.len());
    if removed > 0 {
        write_desired_active_mod_keys(&desired, &config.state_dir)
            .map_err(|error| error.to_string())?;
    }
    Ok(removed)
}

fn forget_revived_deleted_mod_tombstones(
    config: &AppConfig,
    summary: &ScanSummary,
) -> Result<(), String> {
    let connected_keys = connected_mod_keys(summary);
    let mut tombstones = read_deleted_mod_tombstones(config);
    let original_len = tombstones.len();
    tombstones.retain(|key| !connected_keys.contains(key));
    if tombstones.len() != original_len {
        write_deleted_mod_tombstones(config, &tombstones).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn read_deleted_mod_tombstones(config: &AppConfig) -> BTreeSet<String> {
    let path = deleted_mod_tombstones_path(config);
    let Ok(content) = fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn write_deleted_mod_tombstones(
    config: &AppConfig,
    keys: &BTreeSet<String>,
) -> std::io::Result<()> {
    let path = deleted_mod_tombstones_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::new();
    for key in keys {
        output.push_str(key);
        output.push('\n');
    }
    fs::write(path, output)
}

fn remember_deleted_mod_tombstone(config: &AppConfig, key: &str) -> Result<(), String> {
    let mut keys = read_deleted_mod_tombstones(config);
    keys.insert(key.to_string());
    write_deleted_mod_tombstones(config, &keys).map_err(|error| error.to_string())
}

fn remove_deleted_mod_tombstone(config: &AppConfig, key: &str) -> Result<(), String> {
    let mut keys = read_deleted_mod_tombstones(config);
    keys.remove(key);
    write_deleted_mod_tombstones(config, &keys).map_err(|error| error.to_string())
}

fn prune_expired_deleted_mods(config: &AppConfig, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let now = epoch_seconds(Some(SystemTime::now())).unwrap_or(0);
    let retention_secs = u64::from(retention_days) * 86_400;
    let entries = read_deleted_mod_entries(config).map_err(|error| error.to_string())?;
    let mut kept = Vec::new();
    for entry in entries {
        if entry.deleted_epoch.saturating_add(retention_secs) <= now {
            remember_deleted_mod_tombstone(config, &entry.key)?;
            ensure_existing_state_path(&entry.backup_path, config, "삭제 백업 경로")?;
            remove_path_if_exists(&entry.backup_path).map_err(|error| error.to_string())?;
            cleanup_deleted_backup_parent(&entry.backup_path);
        } else {
            kept.push(entry);
        }
    }
    write_deleted_mod_entries(config, &kept).map_err(|error| error.to_string())
}

fn cleanup_deleted_backup_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = remove_dir_if_empty(parent);
    }
}

fn remove_dir_if_empty(path: &Path) -> std::io::Result<()> {
    match fs::read_dir(path)?.next() {
        None => fs::remove_dir(path),
        Some(_) => Ok(()),
    }
}

fn deleted_mod_dto(entry: DeletedModEntry, retention_days: u32) -> DeletedModDto {
    let expires_epoch = (retention_days > 0).then(|| {
        entry
            .deleted_epoch
            .saturating_add(u64::from(retention_days) * 86_400)
    });
    DeletedModDto {
        id: entry.id,
        key: entry.key,
        name: entry.name,
        original_path: display_path(&entry.original_path),
        backup_path: display_path(&entry.backup_path),
        deleted_epoch: entry.deleted_epoch,
        expires_epoch,
        bytes: entry.bytes,
    }
}


