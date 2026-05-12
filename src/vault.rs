use crate::discovery::scan_mod_directory;
use crate::domain::{ModKind, ModRecord, ModSource};
use crate::error::{AppError, AppResult};
use crate::process::hidden_command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultEntry {
    pub key: String,
    pub display_name: String,
    pub payload_path: PathBuf,
    pub kind: ModKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultAction {
    pub key: String,
    pub from: PathBuf,
    pub to: PathBuf,
}

pub fn import_mod(source_path: &Path, vault_dir: &Path) -> AppResult<VaultAction> {
    import_mod_with_key_mode(source_path, vault_dir, false)
}

pub fn import_mod_as_new(source_path: &Path, vault_dir: &Path) -> AppResult<VaultAction> {
    import_mod_with_key_mode(source_path, vault_dir, true)
}

pub fn import_mod_to_disabled(source_path: &Path, game_mods_dir: &Path) -> AppResult<VaultAction> {
    import_mod_to_disabled_with_mode(source_path, game_mods_dir, false)
}

pub fn import_mod_to_disabled_as_new(
    source_path: &Path,
    game_mods_dir: &Path,
) -> AppResult<VaultAction> {
    import_mod_to_disabled_with_mode(source_path, game_mods_dir, true)
}

fn import_mod_to_disabled_with_mode(
    source_path: &Path,
    game_mods_dir: &Path,
    force_unique_name: bool,
) -> AppResult<VaultAction> {
    if !source_path.exists() {
        return Err(AppError::InvalidCommand(format!(
            "mod path does not exist: {}",
            source_path.display()
        )));
    }

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;
    let record = record_for_path(source_path, ModSource::Vault)?;
    let file_name = source_path.file_name().ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "cannot import unnamed path: {}",
            source_path.display()
        ))
    })?;
    let mut target_path = disabled_dir.join(file_name);
    if force_unique_name || target_path.exists() {
        target_path = unique_child_path(&disabled_dir, &PathBuf::from(file_name));
    }
    copy_path(source_path, &target_path)?;
    Ok(VaultAction {
        key: record.stable_key(),
        from: source_path.to_path_buf(),
        to: target_path,
    })
}

fn import_mod_with_key_mode(
    source_path: &Path,
    vault_dir: &Path,
    force_unique_key: bool,
) -> AppResult<VaultAction> {
    if !source_path.exists() {
        return Err(AppError::InvalidCommand(format!(
            "mod path does not exist: {}",
            source_path.display()
        )));
    }

    fs::create_dir_all(vault_dir).map_err(|source| AppError::io(vault_dir, source))?;

    let record = record_for_path(source_path, ModSource::Vault)?;
    let key = record.stable_key();
    let mod_dir = if force_unique_key {
        unique_key_dir(vault_dir, &key)
    } else {
        vault_dir.join(&key)
    };
    fs::create_dir_all(&mod_dir).map_err(|source| AppError::io(&mod_dir, source))?;

    let file_name = source_path
        .file_name()
        .ok_or_else(|| {
            AppError::InvalidCommand(format!(
                "cannot import unnamed path: {}",
                source_path.display()
            ))
        })?
        .to_owned();
    let target_path = unique_child_path(&mod_dir, &PathBuf::from(file_name));

    copy_path(source_path, &target_path)?;

    Ok(VaultAction {
        key: mod_dir
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or(key),
        from: source_path.to_path_buf(),
        to: target_path,
    })
}

pub fn list_vault(vault_dir: &Path) -> AppResult<Vec<VaultEntry>> {
    if !vault_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(vault_dir).map_err(|source| AppError::io(vault_dir, source))? {
        let entry = entry.map_err(|source| AppError::io(vault_dir, source))?;
        let mod_dir = entry.path();
        if !mod_dir.is_dir() {
            continue;
        }

        if let Some(payload_path) = first_payload_path(&mod_dir)? {
            let display_name = payload_path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string());
            let kind = if payload_path.is_dir() {
                ModKind::Directory
            } else {
                classify_payload(&payload_path)
            };

            entries.push(VaultEntry {
                key: entry.file_name().to_string_lossy().to_string(),
                display_name,
                payload_path,
                kind,
            });
        }
    }

    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

pub fn list_vault_records(vault_dir: &Path) -> AppResult<Vec<ModRecord>> {
    let mut records = Vec::new();
    for entry in list_vault(vault_dir)? {
        records.push(vault_entry_record(entry)?);
    }
    Ok(records)
}

pub fn list_disabled_game_entries(game_mods_dir: &Path) -> AppResult<Vec<VaultEntry>> {
    let mut entries = list_disabled_game_mods(game_mods_dir)?
        .into_iter()
        .map(|record| VaultEntry {
            key: record.stable_key(),
            display_name: record.name,
            payload_path: record.path,
            kind: record.kind,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

pub fn migrate_legacy_disabled_mods(
    vault_dir: &Path,
    game_mods_dir: &Path,
) -> AppResult<Vec<VaultAction>> {
    let mut actions = migrate_nested_game_disabled_mods(game_mods_dir)?;
    if !vault_dir.exists() {
        return Ok(actions);
    }

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;

    for entry in fs::read_dir(vault_dir).map_err(|source| AppError::io(vault_dir, source))? {
        let entry = entry.map_err(|source| AppError::io(vault_dir, source))?;
        let mod_dir = entry.path();
        if !mod_dir.is_dir() {
            continue;
        }

        let legacy_disabled_dir = mod_dir.join("disabled");
        if !legacy_disabled_dir.is_dir() {
            continue;
        }

        for payload in fs::read_dir(&legacy_disabled_dir)
            .map_err(|source| AppError::io(&legacy_disabled_dir, source))?
        {
            let payload = payload.map_err(|source| AppError::io(&legacy_disabled_dir, source))?;
            let source_path = payload.path();
            let Some(file_name) = source_path.file_name() else {
                continue;
            };
            let target_path = unique_child_path(&disabled_dir, &PathBuf::from(file_name));
            move_path(&source_path, &target_path)?;
            let key = record_for_path(&target_path, ModSource::Vault)
                .map(|record| record.stable_key())
                .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());
            actions.push(VaultAction {
                key,
                from: source_path,
                to: target_path,
            });
        }

        remove_empty_dir(&legacy_disabled_dir)
            .map_err(|source| AppError::io(&legacy_disabled_dir, source))?;
        remove_empty_dir(&mod_dir).map_err(|source| AppError::io(&mod_dir, source))?;
    }

    actions.extend(migrate_vault_mods_to_disabled(vault_dir, game_mods_dir)?);
    Ok(actions)
}

pub fn migrate_vault_mods_to_disabled(
    vault_dir: &Path,
    game_mods_dir: &Path,
) -> AppResult<Vec<VaultAction>> {
    if !vault_dir.exists() {
        return Ok(Vec::new());
    }

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;
    let entries = list_vault(vault_dir)?;
    let mut actions = Vec::new();

    for entry in entries {
        if !entry.payload_path.exists() {
            continue;
        }
        let Some(file_name) = entry.payload_path.file_name() else {
            continue;
        };
        let target_path = unique_child_path(&disabled_dir, &PathBuf::from(file_name));
        move_path(&entry.payload_path, &target_path)?;
        actions.push(VaultAction {
            key: entry.key.clone(),
            from: entry.payload_path.clone(),
            to: target_path,
        });
        let legacy_root = vault_dir.join(entry.key);
        let _ = remove_empty_dir(&legacy_root);
    }
    let _ = remove_empty_dir(vault_dir);

    Ok(actions)
}

pub fn enable_mod(
    key: &str,
    vault_dir: &Path,
    game_mods_dir: &Path,
    vendor_dir: &Path,
) -> AppResult<VaultAction> {
    fs::create_dir_all(game_mods_dir).map_err(|source| AppError::io(game_mods_dir, source))?;
    migrate_vault_mods_to_disabled(vault_dir, game_mods_dir)?;
    if let Some(record) = find_disabled_game_record(key, game_mods_dir)? {
        let target_path = activation_target_path(&record.path, game_mods_dir)?;
        if target_path.exists() {
            return Err(AppError::InvalidCommand(format!(
                "enabled mod target already exists: {}",
                target_path.display()
            )));
        }
        if is_supported_archive(&record.path) {
            expand_mod_archive(&record.path, &target_path, vendor_dir)?;
        } else {
            move_path(&record.path, &target_path)?;
        }
        return Ok(VaultAction {
            key: record.stable_key(),
            from: record.path,
            to: target_path,
        });
    }

    Err(AppError::InvalidCommand(format!(
        "disabled mod not found: {key}"
    )))
}

pub fn normalize_active_archives(
    game_mods_dir: &Path,
    vendor_dir: &Path,
) -> AppResult<Vec<VaultAction>> {
    fs::create_dir_all(game_mods_dir).map_err(|source| AppError::io(game_mods_dir, source))?;
    let records = scan_mod_directory(game_mods_dir, ModSource::GameMods)?;
    let mut actions = Vec::new();

    for record in records
        .into_iter()
        .filter(|record| is_supported_archive(&record.path))
    {
        let target_path = activation_target_path(&record.path, game_mods_dir)?;
        if !target_path.exists() {
            expand_mod_archive(&record.path, &target_path, vendor_dir)?;
        }
        remove_path_if_exists(&record.path).map_err(|source| AppError::io(&record.path, source))?;
        actions.push(VaultAction {
            key: record.stable_key(),
            from: record.path,
            to: target_path,
        });
    }

    Ok(actions)
}

fn activation_target_path(source: &Path, game_mods_dir: &Path) -> AppResult<PathBuf> {
    if is_supported_archive(source) {
        let stem = source.file_stem().ok_or_else(|| {
            AppError::InvalidCommand(format!("archive mod has no filename: {}", source.display()))
        })?;
        return Ok(game_mods_dir.join(stem));
    }

    let file_name = source.file_name().ok_or_else(|| {
        AppError::InvalidCommand(format!("mod has no filename: {}", source.display()))
    })?;
    Ok(game_mods_dir.join(file_name))
}

pub fn disable_mod(key: &str, game_mods_dir: &Path, _vault_dir: &Path) -> AppResult<VaultAction> {
    let records = scan_mod_directory(game_mods_dir, ModSource::GameMods)?;
    let record = records
        .into_iter()
        .find(|record| record.stable_key() == key)
        .ok_or_else(|| AppError::InvalidCommand(format!("enabled mod not found: {key}")))?;

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;

    let file_name = record.path.file_name().ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "enabled mod has no filename: {}",
            record.path.display()
        ))
    })?;
    let target_path = disabled_dir.join(file_name);
    remove_path_if_exists(&target_path).map_err(|source| AppError::io(&target_path, source))?;
    move_path(&record.path, &target_path)?;

    Ok(VaultAction {
        key: record.stable_key(),
        from: record.path,
        to: target_path,
    })
}

pub fn disable_all(game_mods_dir: &Path, vault_dir: &Path) -> AppResult<Vec<VaultAction>> {
    let records = scan_mod_directory(game_mods_dir, ModSource::GameMods)?;
    let mut actions = Vec::new();

    for record in records {
        actions.push(disable_mod(&record.stable_key(), game_mods_dir, vault_dir)?);
    }
    actions.extend(disable_game_mod_metadata(game_mods_dir)?);

    Ok(actions)
}

fn disable_game_mod_metadata(game_mods_dir: &Path) -> AppResult<Vec<VaultAction>> {
    if !game_mods_dir.exists() {
        return Ok(Vec::new());
    }

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;
    let mut actions = Vec::new();
    for entry in
        fs::read_dir(game_mods_dir).map_err(|source| AppError::io(game_mods_dir, source))?
    {
        let entry = entry.map_err(|source| AppError::io(game_mods_dir, source))?;
        let path = entry.path();
        if !is_vanilla_unsafe_metadata_path(&path) {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let target = disabled_dir.join(file_name);
        remove_path_if_exists(&target).map_err(|source| AppError::io(&target, source))?;
        move_path(&path, &target)?;
        actions.push(VaultAction {
            key: file_name.to_string_lossy().to_string(),
            from: path,
            to: target,
        });
    }
    Ok(actions)
}

fn is_vanilla_unsafe_metadata_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            let unprefixed = lower.trim_start_matches('_');
            lower.starts_with("vortex.deployment.") || unprefixed == "vortex_staging_folder"
        })
}

fn move_path(source: &Path, target: &Path) -> AppResult<()> {
    let mut last_error = None;
    for attempt in 0..5 {
        match fs::rename(source, target) {
            Ok(()) => return Ok(()),
            Err(error) if error.raw_os_error() == Some(17) => {
                if let Err(copy_error) = copy_path(source, target) {
                    let _ = remove_path_if_exists(target);
                    return Err(copy_error);
                }
                return remove_path_if_exists(source).map_err(|error| AppError::io(source, error));
            }
            Err(error) if is_transient_windows_move_error(&error) => {
                last_error = Some(error);
                if attempt < 4 {
                    thread::sleep(Duration::from_millis(250 * (attempt + 1)));
                }
            }
            Err(error) => return Err(AppError::io(source, error)),
        }
    }

    if let Some(error) = last_error {
        Err(AppError::InvalidCommand(format!(
            "{}: 다른 프로그램이 모드 폴더를 사용 중이라 이동할 수 없습니다. 게임, Vortex/모드 매니저, 탐색기 미리보기를 닫고 다시 시도하세요. ({error})",
            source.display()
        )))
    } else {
        match fs::rename(source, target) {
            Ok(()) => Ok(()),
            Err(error) if error.raw_os_error() == Some(17) => {
                if let Err(copy_error) = copy_path(source, target) {
                    let _ = remove_path_if_exists(target);
                    return Err(copy_error);
                }
                remove_path_if_exists(source).map_err(|error| AppError::io(source, error))
            }
            Err(error) => Err(AppError::io(source, error)),
        }
    }
}

fn is_transient_windows_move_error(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5) | Some(32) | Some(33))
}

pub fn list_disabled_game_mods(game_mods_dir: &Path) -> AppResult<Vec<ModRecord>> {
    scan_mod_directory(&game_disabled_dir(game_mods_dir), ModSource::Vault)
}

fn find_disabled_game_record(key: &str, game_mods_dir: &Path) -> AppResult<Option<ModRecord>> {
    let mut matches = list_disabled_game_mods(game_mods_dir)?
        .into_iter()
        .filter(|record| record.stable_key() == key)
        .collect::<Vec<_>>();
    matches.sort_by_key(|record| match record.kind {
        ModKind::Directory => 0,
        _ => 1,
    });
    Ok(matches.into_iter().next())
}

fn game_disabled_dir(game_mods_dir: &Path) -> PathBuf {
    game_mods_dir
        .parent()
        .map(|parent| parent.join("mods.disabled"))
        .unwrap_or_else(|| game_mods_dir.with_file_name("mods.disabled"))
}

fn legacy_nested_disabled_dir(game_mods_dir: &Path) -> PathBuf {
    game_mods_dir.join(".disabled")
}

fn migrate_nested_game_disabled_mods(game_mods_dir: &Path) -> AppResult<Vec<VaultAction>> {
    let legacy_dir = legacy_nested_disabled_dir(game_mods_dir);
    if !legacy_dir.is_dir() {
        return Ok(Vec::new());
    }

    let disabled_dir = game_disabled_dir(game_mods_dir);
    fs::create_dir_all(&disabled_dir).map_err(|source| AppError::io(&disabled_dir, source))?;

    let mut actions = Vec::new();
    for entry in fs::read_dir(&legacy_dir).map_err(|source| AppError::io(&legacy_dir, source))? {
        let entry = entry.map_err(|source| AppError::io(&legacy_dir, source))?;
        let source_path = entry.path();
        let Some(file_name) = source_path.file_name() else {
            continue;
        };
        let target_path = unique_child_path(&disabled_dir, &PathBuf::from(file_name));
        move_path(&source_path, &target_path)?;
        let key = record_for_path(&target_path, ModSource::Vault)
            .map(|record| record.stable_key())
            .unwrap_or_else(|_| file_name.to_string_lossy().to_string());
        actions.push(VaultAction {
            key,
            from: source_path,
            to: target_path,
        });
    }

    remove_empty_dir(&legacy_dir).map_err(|source| AppError::io(&legacy_dir, source))?;
    Ok(actions)
}

fn remove_path_if_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn vault_entry_record(entry: VaultEntry) -> AppResult<ModRecord> {
    Ok(ModRecord {
        name: entry.key,
        version_hint: infer_version_hint(&entry.display_name),
        fingerprint: fingerprint_path(&entry.payload_path)?,
        path: entry.payload_path,
        source: ModSource::Vault,
        kind: entry.kind,
    })
}

fn fingerprint_path(path: &Path) -> AppResult<crate::domain::ModFingerprint> {
    let metadata = fs::metadata(path).map_err(|source| AppError::io(path, source))?;
    if metadata.is_dir() {
        fingerprint_directory(path)
    } else {
        Ok(crate::domain::ModFingerprint {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

fn fingerprint_directory(path: &Path) -> AppResult<crate::domain::ModFingerprint> {
    let mut bytes = 0;
    let mut modified = None;
    collect_directory_fingerprint(path, &mut bytes, &mut modified)?;
    Ok(crate::domain::ModFingerprint { bytes, modified })
}

fn collect_directory_fingerprint(
    path: &Path,
    bytes: &mut u64,
    modified: &mut Option<SystemTime>,
) -> AppResult<()> {
    for entry in fs::read_dir(path).map_err(|source| AppError::io(path, source))? {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| AppError::io(&entry_path, source))?;
        if metadata.is_dir() {
            collect_directory_fingerprint(&entry_path, bytes, modified)?;
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

fn infer_version_hint(name: &str) -> Option<String> {
    name.split(['-', '_', ' ', '[', ']'])
        .find(|part| {
            let lower = part.to_ascii_lowercase();
            lower.starts_with('v')
                && lower
                    .chars()
                    .skip(1)
                    .any(|character| character.is_ascii_digit())
        })
        .map(|part| part.trim().to_string())
}

fn remove_empty_dir(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => Err(error),
    }
}

fn first_payload_path(mod_dir: &Path) -> AppResult<Option<PathBuf>> {
    let mut children = fs::read_dir(mod_dir)
        .map_err(|source| AppError::io(mod_dir, source))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| !name.to_string_lossy().eq_ignore_ascii_case("disabled"))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    children.sort();
    Ok(children.into_iter().next())
}

fn record_for_path(path: &Path, source: ModSource) -> AppResult<ModRecord> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidCommand(format!("cannot read parent for {}", path.display()))
    })?;
    let name = path.file_name().ok_or_else(|| {
        AppError::InvalidCommand(format!("cannot read filename for {}", path.display()))
    })?;
    let records = scan_mod_directory(parent, source)?;
    records
        .into_iter()
        .find(|record| record.path.file_name() == Some(name))
        .ok_or_else(|| AppError::InvalidCommand(format!("cannot classify {}", path.display())))
}

fn classify_payload(path: &Path) -> ModKind {
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

fn copy_path(source: &Path, target: &Path) -> AppResult<()> {
    if source.is_dir() {
        if target.exists() {
            return Err(AppError::InvalidCommand(format!(
                "target already exists: {}",
                target.display()
            )));
        }
        copy_dir_recursive(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
        }
        fs::copy(source, target)
            .map(|_| ())
            .map_err(|error| AppError::io(source_path_for_error(source, target), error))
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir_all(target).map_err(|source| AppError::io(target, source))?;
    for entry in fs::read_dir(source).map_err(|error| AppError::io(source, error))? {
        let entry = entry.map_err(|error| AppError::io(source, error))?;
        let child_source = entry.path();
        let child_target = target.join(entry.file_name());
        if child_source.is_dir() {
            copy_dir_recursive(&child_source, &child_target)?;
        } else {
            fs::copy(&child_source, &child_target)
                .map(|_| ())
                .map_err(|source| AppError::io(&child_source, source))?;
        }
    }
    Ok(())
}

fn expand_zip_archive(source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() {
        return Err(AppError::InvalidCommand(format!(
            "enabled mod target already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    fs::create_dir_all(target).map_err(|source| AppError::io(target, source))?;
    let command = format!(
        "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        powershell_quote(source),
        powershell_quote(target)
    );
    let status = hidden_command("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source_error| AppError::io(source, source_error))?;

    if status.success() {
        Ok(())
    } else {
        let _ = fs::remove_dir_all(target);
        Err(AppError::InvalidCommand(format!(
            "zip mod extraction failed: {}",
            source.display()
        )))
    }
}

fn expand_mod_archive(source: &Path, target: &Path, vendor_dir: &Path) -> AppResult<()> {
    if is_zip_archive(source) {
        return expand_zip_archive(source, target);
    }

    let seven_zip = vendor_dir.join("7zip").join("7z.exe");
    if !seven_zip.is_file() {
        return Err(AppError::InvalidCommand(format!(
            "압축 모드를 풀 수 없습니다. 내장 7-Zip 도구를 찾지 못했습니다: {}",
            seven_zip.display()
        )));
    }
    expand_with_7z(&seven_zip, source, target)
}

fn expand_with_7z(seven_zip: &Path, source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() {
        return Err(AppError::InvalidCommand(format!(
            "enabled mod target already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    fs::create_dir_all(target).map_err(|source| AppError::io(target, source))?;

    let status = hidden_command(seven_zip)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", target.to_string_lossy()))
        .arg(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source_error| AppError::io(seven_zip, source_error))?;

    if status.success() {
        Ok(())
    } else {
        let _ = fs::remove_dir_all(target);
        Err(AppError::InvalidCommand(format!(
            "archive mod extraction failed: {}",
            source.display()
        )))
    }
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn is_supported_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "zip" | "7z" | "rar"))
}

fn is_zip_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
}

fn unique_child_path(parent: &Path, child: &Path) -> PathBuf {
    let target = parent.join(child);
    if !target.exists() {
        return target;
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let name = child
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "mod".to_string());
    parent.join(format!("{stamp}-{name}"))
}

fn unique_key_dir(vault_dir: &Path, key: &str) -> PathBuf {
    let target = vault_dir.join(key);
    if !target.exists() {
        return target;
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    for index in 1..1000 {
        let candidate = vault_dir.join(format!("{key}-{stamp}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    vault_dir.join(format!("{key}-{stamp}"))
}

fn source_path_for_error(source: &Path, target: &Path) -> PathBuf {
    if source.exists() {
        source.to_path_buf()
    } else {
        target.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn imports_and_enables_file_mod() {
        let fixture = TestWorkspace::create("imports_and_enables_file_mod");
        let source = fixture.write_file("downloads/Example-v1.jar", "mod bytes");

        let imported = import_mod(&source, &fixture.vault_dir()).expect("import");
        let entries = list_vault(&fixture.vault_dir()).expect("list vault");
        let enabled = enable_mod(
            &imported.key,
            &fixture.vault_dir(),
            &fixture.game_mods_dir(),
            &fixture.vendor_dir(),
        )
        .expect("enable");

        assert_eq!(entries.len(), 1);
        assert!(enabled.to.exists());
        assert_eq!(
            fs::read_to_string(enabled.to).expect("enabled content"),
            "mod bytes"
        );
    }

    #[test]
    fn imports_same_key_as_new_visible_vault_entry() {
        let fixture = TestWorkspace::create("imports_same_key_as_new_visible_vault_entry");
        let source = fixture.write_file("downloads/Example-v1.jar", "mod bytes");

        let first = import_mod_as_new(&source, &fixture.vault_dir()).expect("first import");
        let second = import_mod_as_new(&source, &fixture.vault_dir()).expect("second import");
        let entries = list_vault(&fixture.vault_dir()).expect("list vault");

        assert_ne!(first.key, second.key);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.key == first.key));
        assert!(entries.iter().any(|entry| entry.key == second.key));
    }

    #[test]
    fn disables_enabled_mod_by_moving_to_game_disabled_folder() {
        let fixture =
            TestWorkspace::create("disables_enabled_mod_by_moving_to_game_disabled_folder");
        fixture.write_file("game/mods/Example-v1.zip", "mod bytes");

        let disabled = disable_mod("example-v1", &fixture.game_mods_dir(), &fixture.vault_dir())
            .expect("disable");

        assert!(disabled.to.exists());
        assert_eq!(
            disabled.to,
            fixture
                .path
                .join("game/mods.disabled")
                .join("Example-v1.zip")
        );
        assert!(!fixture.path.join("game/mods/Example-v1.zip").exists());
    }

    #[test]
    fn enables_disabled_game_mod_by_renaming_back() {
        let fixture = TestWorkspace::create("enables_disabled_game_mod_by_renaming_back");
        fixture.write_file("game/mods.disabled/Example-v1.jar", "mod bytes");

        let enabled = enable_mod(
            "example-v1",
            &fixture.vault_dir(),
            &fixture.game_mods_dir(),
            &fixture.vendor_dir(),
        )
        .expect("enable");

        assert_eq!(enabled.to, fixture.path.join("game/mods/Example-v1.jar"));
        assert!(enabled.to.exists());
        assert!(
            !fixture
                .path
                .join("game/mods.disabled/Example-v1.jar")
                .exists()
        );
    }

    #[test]
    fn disabled_game_mods_are_listed_separately_from_vault() {
        let fixture = TestWorkspace::create("disabled_game_mods_are_listed_separately_from_vault");
        fixture.write_file("game/mods.disabled/Example-v1.zip", "mod bytes");

        let disabled = list_disabled_game_mods(&fixture.game_mods_dir()).expect("list disabled");

        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].stable_key(), "example-v1");
        assert!(
            list_vault(&fixture.vault_dir())
                .expect("list vault")
                .is_empty()
        );
    }

    #[test]
    fn vault_records_skip_legacy_disabled_only_shells() {
        let fixture = TestWorkspace::create("vault_records_skip_legacy_disabled_only_shells");
        fixture.write_file("vault/example/disabled/Example-v1.zip", "mod bytes");

        let records = list_vault_records(&fixture.vault_dir()).expect("list records");

        assert!(records.is_empty());
    }

    #[test]
    fn legacy_disabled_vault_mods_migrate_to_game_disabled_folder() {
        let fixture =
            TestWorkspace::create("legacy_disabled_vault_mods_migrate_to_game_disabled_folder");
        fixture.write_file("vault/example/disabled/Example-v1.zip", "mod bytes");

        let actions = migrate_legacy_disabled_mods(&fixture.vault_dir(), &fixture.game_mods_dir())
            .expect("migrate");

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].to,
            fixture.path.join("game/mods.disabled/Example-v1.zip")
        );
        assert!(actions[0].to.exists());
        assert!(!fixture.path.join("vault/example").exists());
    }

    #[test]
    fn nested_game_disabled_mods_migrate_outside_mods_folder() {
        let fixture =
            TestWorkspace::create("nested_game_disabled_mods_migrate_outside_mods_folder");
        fixture.write_file("game/mods/.disabled/Example-v1.zip", "mod bytes");

        let actions = migrate_legacy_disabled_mods(&fixture.vault_dir(), &fixture.game_mods_dir())
            .expect("migrate");

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].to,
            fixture.path.join("game/mods.disabled/Example-v1.zip")
        );
        assert!(actions[0].to.exists());
        assert!(!fixture.path.join("game/mods/.disabled").exists());
    }

    #[test]
    fn disable_all_moves_vortex_metadata_out_of_mods_folder() {
        let fixture = TestWorkspace::create("disable_all_moves_vortex_metadata_out_of_mods_folder");
        fixture.write_file("game/mods/vortex.deployment.slaythespire2-mod.json", "{}");
        fixture.write_file("game/mods/__vortex_staging_folder/staged.txt", "staged");

        let actions = disable_all(&fixture.game_mods_dir(), &fixture.vault_dir())
            .expect("disable all metadata");

        assert_eq!(actions.len(), 2);
        assert!(
            !fixture
                .path
                .join("game/mods/vortex.deployment.slaythespire2-mod.json")
                .exists()
        );
        assert!(
            !fixture
                .path
                .join("game/mods/__vortex_staging_folder")
                .exists()
        );
        assert!(
            fixture
                .path
                .join("game/mods.disabled/vortex.deployment.slaythespire2-mod.json")
                .exists()
        );
        assert!(
            fixture
                .path
                .join("game/mods.disabled/__vortex_staging_folder/staged.txt")
                .exists()
        );
    }

    #[test]
    fn rar_archives_activate_to_extracted_folder_name() {
        let fixture = TestWorkspace::create("rar_archives_activate_to_extracted_folder_name");
        let source = fixture.write_file("vault/akisister/AkiSister-654.rar", "archive bytes");

        let target =
            activation_target_path(&source, &fixture.game_mods_dir()).expect("target path");

        assert_eq!(target, fixture.path.join("game/mods/AkiSister-654"));
    }

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn create(name: &str) -> Self {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("target");
            path.push("test-work");
            path.push(format!(
                "{}-{}",
                name,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("create workspace");
            Self { path }
        }

        fn vault_dir(&self) -> PathBuf {
            self.path.join("vault")
        }

        fn game_mods_dir(&self) -> PathBuf {
            self.path.join("game").join("mods")
        }

        fn vendor_dir(&self) -> PathBuf {
            self.path.join("vendor")
        }

        fn write_file(&self, child: &str, content: &str) -> PathBuf {
            let path = self.path.join(child);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            let mut file = fs::File::create(&path).expect("create file");
            file.write_all(content.as_bytes()).expect("write file");
            path
        }
    }
}
