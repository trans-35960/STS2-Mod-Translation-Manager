use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PROFILE_NAMES: [&str; 3] = ["profile1", "profile2", "profile3"];
const STEAM_APP_ID: &str = "2868840";
const STEAM_ID64_ACCOUNT_BASE: u64 = 76_561_197_960_265_728;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveKind {
    Vanilla,
    Modded,
}

impl SaveKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Modded => "modded",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Vanilla => "바닐라",
            Self::Modded => "모드",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "vanilla" => Some(Self::Vanilla),
            "modded" => Some(Self::Modded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupEntry {
    pub id: String,
    pub kind: SaveKind,
    pub created_epoch: u64,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupReport {
    pub save_dir: Option<PathBuf>,
    pub created: Vec<SaveBackupEntry>,
    pub seeded_modded_profiles: usize,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRunCleanupReport {
    pub cleared_files: Vec<PathBuf>,
    pub cleaned_cloud_caches: Vec<PathBuf>,
    pub remaining_files: Vec<PathBuf>,
}

pub fn backup_before_launch(
    config: &AppConfig,
    ensure_modded: bool,
) -> AppResult<SaveBackupReport> {
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Ok(SaveBackupReport {
            save_dir: None,
            created: Vec::new(),
            seeded_modded_profiles: 0,
            skipped_reason: Some("세이브 폴더를 탐지하지 못했습니다.".to_string()),
        });
    };

    if !save_dir.exists() {
        return Ok(SaveBackupReport {
            save_dir: Some(save_dir.to_path_buf()),
            created: Vec::new(),
            seeded_modded_profiles: 0,
            skipped_reason: Some(format!(
                "세이브 폴더가 아직 없습니다: {}",
                save_dir.display()
            )),
        });
    }

    let seeded_modded_profiles = if ensure_modded {
        ensure_modded_profiles(save_dir)?
    } else {
        0
    };

    fs::create_dir_all(&config.save_backup_dir)
        .map_err(|source| AppError::io(&config.save_backup_dir, source))?;

    let mut created = Vec::new();
    if let Some(entry) = backup_vanilla_saves(save_dir, &config.save_backup_dir)? {
        created.push(entry);
    }
    if let Some(entry) = backup_modded_saves(save_dir, &config.save_backup_dir)? {
        created.push(entry);
    }
    prune_backups(config)?;

    Ok(SaveBackupReport {
        save_dir: Some(save_dir.to_path_buf()),
        created,
        seeded_modded_profiles,
        skipped_reason: None,
    })
}

pub fn list_backups(config: &AppConfig) -> AppResult<Vec<SaveBackupEntry>> {
    let mut entries = Vec::new();
    for kind in [SaveKind::Vanilla, SaveKind::Modded] {
        let dir = config.save_backup_dir.join(kind.as_str());
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|source| AppError::io(&dir, source))? {
            let entry = entry.map_err(|source| AppError::io(&dir, source))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(created_epoch) = path
                .file_name()
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse::<u64>().ok())
            else {
                continue;
            };
            entries.push(SaveBackupEntry {
                id: backup_id(&kind, created_epoch),
                kind: kind.clone(),
                created_epoch,
                bytes: path_size(&path)?,
                path,
            });
        }
    }
    entries.sort_by(|left, right| right.created_epoch.cmp(&left.created_epoch));
    Ok(entries)
}

pub fn restore_backup(config: &AppConfig, id: &str) -> AppResult<SaveBackupEntry> {
    let (kind, created_epoch) = parse_backup_id(id)?;
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Err(AppError::InvalidCommand(
            "세이브 폴더를 탐지하지 못해 복원할 수 없습니다.".to_string(),
        ));
    };
    let entry = list_backups(config)?
        .into_iter()
        .find(|entry| entry.kind == kind && entry.created_epoch == created_epoch)
        .ok_or_else(|| AppError::InvalidCommand(format!("백업을 찾을 수 없습니다: {id}")))?;

    match kind {
        SaveKind::Vanilla => {
            for name in PROFILE_NAMES {
                let source = entry.path.join(name);
                if source.exists() {
                    replace_path(&source, &save_dir.join(name))?;
                }
            }
        }
        SaveKind::Modded => {
            replace_path(&entry.path.join("modded"), &save_dir.join("modded"))?;
        }
    }

    Ok(entry)
}

pub fn delete_backup(config: &AppConfig, id: &str) -> AppResult<SaveBackupEntry> {
    let (kind, created_epoch) = parse_backup_id(id)?;
    let entry = list_backups(config)?
        .into_iter()
        .find(|entry| entry.kind == kind && entry.created_epoch == created_epoch)
        .ok_or_else(|| AppError::InvalidCommand(format!("백업을 찾을 수 없습니다: {id}")))?;
    remove_path(&entry.path)?;
    Ok(entry)
}

pub fn ensure_modded_profiles(save_dir: &Path) -> AppResult<usize> {
    let modded_dir = save_dir.join("modded");
    if modded_dir.exists() {
        return Ok(0);
    }
    fs::create_dir_all(&modded_dir).map_err(|source| AppError::io(&modded_dir, source))?;

    let mut copied = 0;
    for name in PROFILE_NAMES {
        let source = save_dir.join(name);
        if !source.exists() {
            continue;
        }
        copy_path(&source, &modded_dir.join(name))?;
        copied += 1;
    }
    Ok(copied)
}

pub fn quarantine_modded_current_runs_for_vanilla(config: &AppConfig) -> AppResult<Vec<PathBuf>> {
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Ok(Vec::new());
    };
    if !save_dir.exists() {
        return Ok(Vec::new());
    }

    let created_epoch = epoch_seconds();
    let mut quarantined = Vec::new();
    for profile in PROFILE_NAMES {
        for current_run in current_run_quarantine_candidates_for_profile(config, save_dir, profile)
        {
            if !current_run.exists() || !is_modded_current_run_save(&current_run)? {
                continue;
            }
            let target_dir = unique_snapshot_dir(
                &config
                    .save_backup_dir
                    .join("vanilla-current-run-quarantine"),
                created_epoch,
            )?
            .join(profile)
            .join("saves");
            fs::create_dir_all(&target_dir).map_err(|source| AppError::io(&target_dir, source))?;
            let target = unique_backup_file(&target_dir, "current_run.save");
            move_path_to_backup(&current_run, &target)?;
            quarantined.push(target);
        }
    }
    Ok(quarantined)
}

pub fn bridge_modded_current_runs_for_modded_launch(config: &AppConfig) -> AppResult<Vec<PathBuf>> {
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Ok(Vec::new());
    };
    if !save_dir.exists() {
        return Ok(Vec::new());
    }

    let mut bridged = Vec::new();
    for profile in PROFILE_NAMES {
        let source = save_dir
            .join("modded")
            .join(profile)
            .join("saves")
            .join("current_run.save");
        if !source.exists() || !is_modded_current_run_save(&source)? {
            continue;
        }

        for target in current_run_candidates_for_profile(config, save_dir, profile) {
            if target.exists() {
                continue;
            }
            let is_local_target = target.starts_with(save_dir);
            if !is_local_target && !target.parent().is_some_and(|parent| parent.is_dir()) {
                continue;
            }
            copy_path(&source, &target)?;
            bridged.push(target);
        }
    }
    Ok(bridged)
}

pub fn clear_current_runs_for_mode_switch(
    config: &AppConfig,
) -> AppResult<CurrentRunCleanupReport> {
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Ok(CurrentRunCleanupReport {
            cleared_files: Vec::new(),
            cleaned_cloud_caches: Vec::new(),
            remaining_files: Vec::new(),
        });
    };
    if !save_dir.exists() {
        return Ok(CurrentRunCleanupReport {
            cleared_files: Vec::new(),
            cleaned_cloud_caches: Vec::new(),
            remaining_files: Vec::new(),
        });
    }

    let created_epoch = epoch_seconds();
    let mut cleared = Vec::new();
    for profile in PROFILE_NAMES {
        for current_run in current_run_cleanup_candidates_for_profile(config, save_dir, profile) {
            if !current_run.exists() {
                continue;
            }
            let target_dir = unique_snapshot_dir(
                &config.save_backup_dir.join("current-run-cleanup"),
                created_epoch,
            )?
            .join(profile)
            .join("saves");
            fs::create_dir_all(&target_dir).map_err(|source| AppError::io(&target_dir, source))?;
            let file_name = current_run
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("current_run.save");
            let target = unique_backup_file(&target_dir, file_name);
            move_path_to_backup(&current_run, &target)?;
            cleared.push(target);
        }
    }
    let cleaned_cloud_caches = clear_current_run_steam_cloud_cache_entries(config, save_dir)?;
    let remaining_files = modded_current_run_paths_for_mode_switch(config)?;
    Ok(CurrentRunCleanupReport {
        cleared_files: cleared,
        cleaned_cloud_caches,
        remaining_files,
    })
}

pub fn has_modded_current_run_for_mode_switch(config: &AppConfig) -> AppResult<bool> {
    Ok(!modded_current_run_paths_for_mode_switch(config)?.is_empty())
}

pub fn modded_current_run_paths_for_mode_switch(config: &AppConfig) -> AppResult<Vec<PathBuf>> {
    let Some(save_dir) = config.save_dir.as_deref() else {
        return Ok(Vec::new());
    };
    if !save_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for profile in PROFILE_NAMES {
        for current_run in current_run_cleanup_candidates_for_profile(config, save_dir, profile) {
            if current_run.exists() && is_modded_current_run_save(&current_run)? {
                paths.push(current_run);
            }
        }
    }
    Ok(dedupe_paths(paths))
}

fn current_run_cleanup_candidates_for_profile(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
) -> Vec<PathBuf> {
    let mut paths = current_run_quarantine_candidates_for_profile(config, save_dir, profile);
    paths.push(
        save_dir
            .join("modded")
            .join(profile)
            .join("saves")
            .join("current_run.save"),
    );
    paths.extend(steam_cloud_modded_current_run_paths(
        config, save_dir, profile,
    ));
    let backup_paths = paths
        .iter()
        .map(|path| path.with_file_name("current_run.save.backup"))
        .collect::<Vec<_>>();
    paths.extend(backup_paths);
    dedupe_paths(paths)
}

fn current_run_quarantine_candidates_for_profile(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
) -> Vec<PathBuf> {
    let mut paths = current_run_candidates_for_profile(config, save_dir, profile);
    let backup_paths = paths
        .iter()
        .map(|path| path.with_file_name("current_run.save.backup"))
        .collect::<Vec<_>>();
    paths.extend(backup_paths);
    dedupe_paths(paths)
}

fn current_run_candidates_for_profile(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
) -> Vec<PathBuf> {
    let mut paths = vec![
        save_dir
            .join(profile)
            .join("saves")
            .join("current_run.save"),
    ];
    paths.extend(steam_cloud_current_run_paths(config, save_dir, profile));
    dedupe_paths(paths)
}

fn steam_cloud_current_run_paths(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
) -> Vec<PathBuf> {
    steam_cloud_current_run_paths_with_prefix(config, save_dir, profile, None)
}

fn steam_cloud_modded_current_run_paths(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
) -> Vec<PathBuf> {
    steam_cloud_current_run_paths_with_prefix(config, save_dir, profile, Some("modded"))
}

fn steam_cloud_current_run_paths_with_prefix(
    config: &AppConfig,
    save_dir: &Path,
    profile: &str,
    prefix: Option<&str>,
) -> Vec<PathBuf> {
    let Some(account_id) = steam_account_id_from_save_dir(save_dir) else {
        return Vec::new();
    };
    steam_userdata_roots(config)
        .into_iter()
        .map(|root| {
            let mut path = root
                .join(account_id.to_string())
                .join(STEAM_APP_ID)
                .join("remote");
            if let Some(prefix) = prefix {
                path = path.join(prefix);
            }
            path.join(profile).join("saves").join("current_run.save")
        })
        .collect()
}

fn clear_current_run_steam_cloud_cache_entries(
    config: &AppConfig,
    save_dir: &Path,
) -> AppResult<Vec<PathBuf>> {
    let Some(account_id) = steam_account_id_from_save_dir(save_dir) else {
        return Ok(Vec::new());
    };
    let mut cleaned = Vec::new();
    for cache in steam_userdata_roots(config).into_iter().map(|root| {
        root.join(account_id.to_string())
            .join(STEAM_APP_ID)
            .join("remotecache.vdf")
    }) {
        if !cache.exists() {
            continue;
        }
        let content = fs::read_to_string(&cache).map_err(|source| AppError::io(&cache, source))?;
        let updated = remove_current_run_entries_from_vdf(&content);
        if updated == content {
            continue;
        }
        fs::write(&cache, updated).map_err(|source| AppError::io(&cache, source))?;
        cleaned.push(cache);
    }
    Ok(dedupe_paths(cleaned))
}

fn remove_current_run_entries_from_vdf(content: &str) -> String {
    let had_trailing_newline = content.ends_with(['\n', '\r']);
    let lines = content.lines().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if is_current_run_vdf_key(trimmed) {
            index += 1;
            if index < lines.len() && lines[index].trim_start().starts_with('{') {
                let mut depth = 0_i32;
                while index < lines.len() {
                    let line = lines[index].trim();
                    depth += line.matches('{').count() as i32;
                    depth -= line.matches('}').count() as i32;
                    index += 1;
                    if depth <= 0 {
                        break;
                    }
                }
            }
            continue;
        }
        output.push(lines[index]);
        index += 1;
    }
    let mut result = output.join("\n");
    if had_trailing_newline {
        result.push('\n');
    }
    result
}

fn is_current_run_vdf_key(trimmed: &str) -> bool {
    if !trimmed.starts_with('"') {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase().replace('\\', "/");
    (lower.contains("/current_run.save") || lower.contains("\"current_run.save\""))
        && !lower.contains("\"sha\"")
        && !lower.contains("\"size\"")
        && !lower.contains("\"localtime\"")
        && !lower.contains("\"time\"")
        && !lower.contains("\"root\"")
}

fn steam_account_id_from_save_dir(save_dir: &Path) -> Option<u64> {
    let steam_id = save_dir
        .file_name()?
        .to_string_lossy()
        .parse::<u64>()
        .ok()?;
    steam_id.checked_sub(STEAM_ID64_ACCOUNT_BASE)
}

fn steam_userdata_roots(config: &AppConfig) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = env::var_os("STEAM_USERDATA_DIR") {
        roots.push(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("STEAM_DIR") {
        roots.push(PathBuf::from(path).join("userdata"));
    }
    if let Some(steam_root) = steam_root_from_game_dir(&config.game_dir) {
        roots.push(steam_root.join("userdata"));
    }
    if let Some(path) = env::var_os("PROGRAMFILES(X86)") {
        roots.push(PathBuf::from(path).join("Steam").join("userdata"));
    }
    if let Some(path) = env::var_os("PROGRAMFILES") {
        roots.push(PathBuf::from(path).join("Steam").join("userdata"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam\userdata"));
    dedupe_paths(roots)
}

fn steam_root_from_game_dir(game_dir: &Path) -> Option<PathBuf> {
    let common_dir = game_dir.parent()?;
    if !common_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("common"))
    {
        return None;
    }
    let steamapps_dir = common_dir.parent()?;
    if !steamapps_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
    {
        return None;
    }
    steamapps_dir.parent().map(Path::to_path_buf)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut output = Vec::new();
    for path in paths {
        if !output.iter().any(|existing| existing == &path) {
            output.push(path);
        }
    }
    output
}

fn is_modded_current_run_save(path: &Path) -> AppResult<bool> {
    let content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    Ok(current_run_character_ids(&content)
        .into_iter()
        .any(|id| !is_vanilla_character_id(id)))
}

fn current_run_character_ids(content: &str) -> Vec<&str> {
    let mut ids = Vec::new();
    let mut rest = content;
    while let Some(index) = rest.find("\"character_id\"") {
        rest = &rest[index + "\"character_id\"".len()..];
        let Some(colon_index) = rest.find(':') else {
            break;
        };
        rest = &rest[colon_index + 1..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('"') {
            continue;
        }
        let value = &trimmed[1..];
        let Some(end_index) = value.find('"') else {
            break;
        };
        ids.push(&value[..end_index]);
        rest = &value[end_index + 1..];
    }
    ids
}

fn is_vanilla_character_id(id: &str) -> bool {
    matches!(
        id,
        "CHARACTER.IRONCLAD"
            | "CHARACTER.SILENT"
            | "CHARACTER.DEFECT"
            | "CHARACTER.NECROBINDER"
            | "CHARACTER.REGENT"
            | "CHARACTER.DARV"
            | "CHARACTER.OROBAS"
    )
}

fn move_path_to_backup(source: &Path, target: &Path) -> AppResult<()> {
    copy_path(source, target)?;
    remove_path(source)
}

fn unique_backup_file(parent: &Path, file_name: &str) -> PathBuf {
    let target = parent.join(file_name);
    if !target.exists() {
        return target;
    }
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let extension = file_name.rsplit_once('.').map(|(_, extension)| extension);
    for index in 1..1000 {
        let name = match extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}", epoch_seconds()))
}

fn backup_vanilla_saves(save_dir: &Path, backup_root: &Path) -> AppResult<Option<SaveBackupEntry>> {
    let sources: Vec<PathBuf> = PROFILE_NAMES
        .iter()
        .map(|name| save_dir.join(name))
        .filter(|path| path.exists())
        .collect();
    if sources.is_empty() {
        return Ok(None);
    }

    let created_epoch = epoch_seconds();
    let target = unique_snapshot_dir(&backup_root.join("vanilla"), created_epoch)?;
    fs::create_dir_all(&target).map_err(|source| AppError::io(&target, source))?;
    for source in sources {
        let name = source.file_name().ok_or_else(|| {
            AppError::InvalidCommand("프로필 폴더 이름을 읽을 수 없습니다.".to_string())
        })?;
        copy_path(&source, &target.join(name))?;
    }

    Ok(Some(SaveBackupEntry {
        id: backup_id(&SaveKind::Vanilla, snapshot_epoch(&target)?),
        kind: SaveKind::Vanilla,
        created_epoch: snapshot_epoch(&target)?,
        bytes: path_size(&target)?,
        path: target,
    }))
}

fn backup_modded_saves(save_dir: &Path, backup_root: &Path) -> AppResult<Option<SaveBackupEntry>> {
    let source = save_dir.join("modded");
    if !source.exists() {
        return Ok(None);
    }

    let created_epoch = epoch_seconds();
    let target = unique_snapshot_dir(&backup_root.join("modded"), created_epoch)?;
    fs::create_dir_all(&target).map_err(|source| AppError::io(&target, source))?;
    copy_path(&source, &target.join("modded"))?;

    Ok(Some(SaveBackupEntry {
        id: backup_id(&SaveKind::Modded, snapshot_epoch(&target)?),
        kind: SaveKind::Modded,
        created_epoch: snapshot_epoch(&target)?,
        bytes: path_size(&target)?,
        path: target,
    }))
}

fn prune_backups(config: &AppConfig) -> AppResult<()> {
    let now = epoch_seconds();
    let retention_secs = u64::from(config.save_backup_retention_days) * 24 * 60 * 60;
    for kind in [SaveKind::Vanilla, SaveKind::Modded] {
        let mut entries: Vec<SaveBackupEntry> = list_backups(config)?
            .into_iter()
            .filter(|entry| entry.kind == kind)
            .collect();
        entries.sort_by(|left, right| right.created_epoch.cmp(&left.created_epoch));

        for (index, entry) in entries.iter().enumerate() {
            let expired_by_age =
                retention_secs > 0 && now.saturating_sub(entry.created_epoch) > retention_secs;
            let expired_by_count = index >= config.save_backup_max_entries;
            if expired_by_age || expired_by_count {
                remove_path(&entry.path)?;
            }
        }
    }
    Ok(())
}

fn unique_snapshot_dir(kind_dir: &Path, created_epoch: u64) -> AppResult<PathBuf> {
    fs::create_dir_all(kind_dir).map_err(|source| AppError::io(kind_dir, source))?;
    let mut candidate = kind_dir.join(created_epoch.to_string());
    let mut suffix = 1_u64;
    while candidate.exists() {
        candidate = kind_dir.join((created_epoch + suffix).to_string());
        suffix += 1;
    }
    Ok(candidate)
}

fn replace_path(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::InvalidCommand(format!(
            "복원 원본이 없습니다: {}",
            source.display()
        )));
    }
    if target.exists() {
        remove_path(target)?;
    }
    copy_path(source, target)
}

fn copy_path(source: &Path, target: &Path) -> AppResult<()> {
    if source.is_dir() {
        copy_dir_recursive(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
        }
        fs::copy(source, target)
            .map(|_| ())
            .map_err(|source_error| AppError::io(source, source_error))
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir_all(target).map_err(|source| AppError::io(target, source))?;
    for entry in fs::read_dir(source).map_err(|source_error| AppError::io(source, source_error))? {
        let entry = entry.map_err(|source_error| AppError::io(source, source_error))?;
        let path = entry.path();
        copy_path(&path, &target.join(entry.file_name()))?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> AppResult<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|source| AppError::io(path, source))
    } else {
        fs::remove_file(path).map_err(|source| AppError::io(path, source))
    }
}

fn path_size(path: &Path) -> AppResult<u64> {
    if path.is_file() {
        return fs::metadata(path)
            .map(|metadata| metadata.len())
            .map_err(|source| AppError::io(path, source));
    }
    let mut total = 0;
    for entry in fs::read_dir(path).map_err(|source| AppError::io(path, source))? {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        total += path_size(&entry.path())?;
    }
    Ok(total)
}

fn backup_id(kind: &SaveKind, created_epoch: u64) -> String {
    format!("{}:{created_epoch}", kind.as_str())
}

fn parse_backup_id(id: &str) -> AppResult<(SaveKind, u64)> {
    let (kind, epoch) = id
        .split_once(':')
        .ok_or_else(|| AppError::InvalidCommand(format!("백업 ID가 올바르지 않습니다: {id}")))?;
    let kind = SaveKind::from_str(kind)
        .ok_or_else(|| AppError::InvalidCommand(format!("백업 종류가 올바르지 않습니다: {id}")))?;
    let epoch = epoch
        .parse::<u64>()
        .map_err(|_| AppError::InvalidCommand(format!("백업 시간이 올바르지 않습니다: {id}")))?;
    Ok((kind, epoch))
}

fn snapshot_epoch(path: &Path) -> AppResult<u64> {
    path.file_name()
        .and_then(|value| value.to_str())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| {
            AppError::InvalidCommand(format!("백업 시간을 읽을 수 없습니다: {}", path.display()))
        })
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn seeds_modded_profiles_from_vanilla_profiles_once() {
        let root = test_dir("seeds_modded_profiles_from_vanilla_profiles_once");
        fs::create_dir_all(root.join("profile1")).expect("profile1");
        fs::write(root.join("profile1/save.txt"), "vanilla").expect("write save");

        let copied = ensure_modded_profiles(&root).expect("seed modded");
        let second = ensure_modded_profiles(&root).expect("seed modded again");

        assert_eq!(copied, 1);
        assert_eq!(second, 0);
        assert_eq!(
            fs::read_to_string(root.join("modded/profile1/save.txt")).expect("read modded"),
            "vanilla"
        );
    }

    #[test]
    fn backs_up_and_restores_vanilla_and_modded_saves() {
        let workspace = test_dir("backs_up_and_restores_vanilla_and_modded_saves");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("profile1")).expect("profile1");
        fs::create_dir_all(save_dir.join("modded/profile1")).expect("modded profile1");
        fs::write(save_dir.join("profile1/save.txt"), "vanilla").expect("write vanilla");
        fs::write(save_dir.join("modded/profile1/save.txt"), "modded").expect("write modded");
        let config = test_config(&workspace, &save_dir);

        let report = backup_before_launch(&config, true).expect("backup");
        fs::write(save_dir.join("profile1/save.txt"), "changed vanilla").expect("change vanilla");
        fs::write(save_dir.join("modded/profile1/save.txt"), "changed modded")
            .expect("change modded");

        for entry in report.created {
            restore_backup(&config, &entry.id).expect("restore");
        }

        assert_eq!(
            fs::read_to_string(save_dir.join("profile1/save.txt")).expect("read vanilla"),
            "vanilla"
        );
        assert_eq!(
            fs::read_to_string(save_dir.join("modded/profile1/save.txt")).expect("read modded"),
            "modded"
        );
    }

    #[test]
    fn deletes_selected_save_backup() {
        let workspace = test_dir("deletes_selected_save_backup");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("profile1")).expect("profile1");
        fs::write(save_dir.join("profile1/save.txt"), "vanilla").expect("write vanilla");
        let config = test_config(&workspace, &save_dir);

        let report = backup_before_launch(&config, true).expect("backup");
        let entry = report.created.first().expect("created backup").clone();
        let deleted = delete_backup(&config, &entry.id).expect("delete backup");

        assert_eq!(deleted.id, entry.id);
        assert!(!entry.path.exists());
    }

    #[test]
    fn quarantines_only_modded_current_runs_for_vanilla_launch() {
        let workspace = test_dir("quarantines_only_modded_current_runs_for_vanilla_launch");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("profile1/saves")).expect("profile1");
        fs::create_dir_all(save_dir.join("profile2/saves")).expect("profile2");
        fs::write(
            save_dir.join("profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write modded current run");
        fs::write(
            save_dir.join("profile1/saves/current_run.save.backup"),
            r#"{ "players": [{ "character_id": "CHARACTER.ONEMOD_HERTA" }] }"#,
        )
        .expect("write modded current run backup");
        fs::write(
            save_dir.join("profile2/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.IRONCLAD" }] }"#,
        )
        .expect("write vanilla current run");
        let config = test_config(&workspace, &save_dir);

        let quarantined =
            quarantine_modded_current_runs_for_vanilla(&config).expect("quarantine current runs");

        assert_eq!(quarantined.len(), 2);
        assert!(!save_dir.join("profile1/saves/current_run.save").exists());
        assert!(
            !save_dir
                .join("profile1/saves/current_run.save.backup")
                .exists()
        );
        assert!(save_dir.join("profile2/saves/current_run.save").exists());
        assert!(quarantined.iter().all(|path| path.exists()));
        assert!(quarantined.iter().any(|path| {
            fs::read_to_string(path)
                .expect("read quarantined")
                .contains("CHARACTER.MIYU_CHARACTER")
        }));
        assert!(quarantined.iter().any(|path| {
            fs::read_to_string(path)
                .expect("read quarantined backup")
                .contains("CHARACTER.ONEMOD_HERTA")
        }));
    }

    #[test]
    fn clears_current_runs_for_mode_switch_after_warning() {
        let workspace = test_dir("clears_current_runs_for_mode_switch_after_warning");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("profile1/saves")).expect("profile1");
        fs::write(
            save_dir.join("profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write current run");
        fs::write(
            save_dir.join("profile1/saves/current_run.save.backup"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write current run backup");
        fs::create_dir_all(save_dir.join("modded/profile1/saves")).expect("modded profile1");
        fs::write(
            save_dir.join("modded/profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write modded current run");
        let config = test_config(&workspace, &save_dir);

        let report =
            clear_current_runs_for_mode_switch(&config).expect("clear current runs for switch");

        assert_eq!(report.cleared_files.len(), 3);
        assert!(report.cleaned_cloud_caches.is_empty());
        assert!(report.remaining_files.is_empty());
        assert!(!save_dir.join("profile1/saves/current_run.save").exists());
        assert!(
            !save_dir
                .join("profile1/saves/current_run.save.backup")
                .exists()
        );
        assert!(
            !save_dir
                .join("modded/profile1/saves/current_run.save")
                .exists()
        );
        assert!(report.cleared_files.iter().all(|path| path.exists()));
        assert!(report.cleared_files.iter().any(|path| {
            fs::read_to_string(path)
                .expect("read cleared current run")
                .contains("CHARACTER.MIYU_CHARACTER")
        }));
    }

    #[test]
    fn clears_steam_remote_current_run_and_remotecache_entries() {
        let workspace = test_dir("clears_steam_remote_current_run_and_remotecache_entries");
        let steam_root = workspace.join("Steam");
        let game_dir = steam_root
            .join("steamapps")
            .join("common")
            .join("Slay the Spire 2");
        let account_id = 1_u64;
        let steam_id = STEAM_ID64_ACCOUNT_BASE + account_id;
        let save_dir = workspace.join(steam_id.to_string());
        let remote_dir = steam_root
            .join("userdata")
            .join(account_id.to_string())
            .join(STEAM_APP_ID);
        let remote_save = remote_dir
            .join("remote")
            .join("profile1")
            .join("saves")
            .join("current_run.save");
        let remote_backup = remote_save.with_file_name("current_run.save.backup");
        let remote_modded_save = remote_dir
            .join("remote")
            .join("modded")
            .join("profile1")
            .join("saves")
            .join("current_run.save");
        let remote_cache = remote_dir.join("remotecache.vdf");
        fs::create_dir_all(save_dir.join("profile1/saves")).expect("local profile");
        fs::create_dir_all(remote_save.parent().expect("remote parent")).expect("remote profile");
        fs::create_dir_all(remote_modded_save.parent().expect("remote modded parent"))
            .expect("remote modded profile");
        fs::create_dir_all(&game_dir).expect("game dir");
        fs::write(&remote_save, "remote current run").expect("write remote current run");
        fs::write(&remote_backup, "remote current run backup").expect("write remote backup");
        fs::write(&remote_modded_save, "remote modded current run")
            .expect("write remote modded current run");
        fs::write(
            &remote_cache,
            "\"2868840\"\n{\n\t\"profile1/saves/current_run.save\"\n\t{\n\t\t\"root\"\t\t\"0\"\n\t\t\"size\"\t\t\"42\"\n\t}\n\t\"modded/profile1/saves/current_run.save\"\n\t{\n\t\t\"root\"\t\t\"0\"\n\t\t\"size\"\t\t\"42\"\n\t}\n\t\"profile1/saves/slot.save\"\n\t{\n\t\t\"root\"\t\t\"0\"\n\t}\n}\n",
        )
        .expect("write remote cache");
        let mut config = test_config(&workspace, &save_dir);
        config.game_dir = game_dir;

        let report = clear_current_runs_for_mode_switch(&config).expect("clear current runs");

        assert_eq!(report.cleared_files.len(), 3);
        assert_eq!(report.cleaned_cloud_caches, vec![remote_cache.clone()]);
        assert!(report.remaining_files.is_empty());
        assert!(!remote_save.exists());
        assert!(!remote_backup.exists());
        assert!(!remote_modded_save.exists());
        let cache = fs::read_to_string(remote_cache).expect("read remote cache");
        assert!(!cache.contains("current_run.save"));
        assert!(cache.contains("slot.save"));
    }

    #[test]
    fn bridges_modded_current_run_when_current_profile_is_empty() {
        let workspace = test_dir("bridges_modded_current_run_when_current_profile_is_empty");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("modded/profile1/saves")).expect("modded profile1");
        fs::write(
            save_dir.join("modded/profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write modded current run");
        let config = test_config(&workspace, &save_dir);

        let bridged =
            bridge_modded_current_runs_for_modded_launch(&config).expect("bridge current runs");

        assert_eq!(
            bridged,
            vec![save_dir.join("profile1/saves/current_run.save")]
        );
        assert_eq!(
            fs::read_to_string(save_dir.join("profile1/saves/current_run.save"))
                .expect("read bridged"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#
        );
        assert!(
            save_dir
                .join("modded/profile1/saves/current_run.save")
                .exists()
        );
    }

    #[test]
    fn bridge_does_not_overwrite_existing_current_run() {
        let workspace = test_dir("bridge_does_not_overwrite_existing_current_run");
        let save_dir = workspace.join("saves");
        fs::create_dir_all(save_dir.join("profile1/saves")).expect("profile1");
        fs::create_dir_all(save_dir.join("modded/profile1/saves")).expect("modded profile1");
        fs::write(
            save_dir.join("profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.IRONCLAD" }] }"#,
        )
        .expect("write vanilla current run");
        fs::write(
            save_dir.join("modded/profile1/saves/current_run.save"),
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write modded current run");
        let config = test_config(&workspace, &save_dir);

        let bridged =
            bridge_modded_current_runs_for_modded_launch(&config).expect("bridge current runs");

        assert!(bridged.is_empty());
        assert_eq!(
            fs::read_to_string(save_dir.join("profile1/saves/current_run.save"))
                .expect("read current"),
            r#"{ "players": [{ "character_id": "CHARACTER.IRONCLAD" }] }"#
        );
    }

    #[test]
    fn infers_steam_userdata_root_from_custom_game_library() {
        let steam_root = PathBuf::from(r"G:\Game\Steam");
        let game_dir = steam_root
            .join("steamapps")
            .join("common")
            .join("Slay the Spire 2");

        assert_eq!(steam_root_from_game_dir(&game_dir), Some(steam_root));
    }

    fn test_config(workspace: &Path, save_dir: &Path) -> AppConfig {
        AppConfig {
            workspace_dir: workspace.to_path_buf(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game/mods"),
            game_exe_path: None,
            save_dir: Some(save_dir.to_path_buf()),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            vault_dir: workspace.join("vault"),
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state/mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: Vec::new(),
        }
    }

    fn test_dir(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-work");
        path.push(format!(
            "{}-{}",
            name,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create test dir");
        path
    }
}
