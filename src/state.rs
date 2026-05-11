use crate::domain::{ModChange, ModChangeKind, ModFingerprint, ModRecord, ModSource, ScanSummary};
use crate::error::{AppError, AppResult};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DESIRED_ACTIVE_MODS_FILE: &str = "desired_active_mods.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStateEntry {
    pub key: String,
    pub source: ModSource,
    pub bytes: u64,
    pub modified_epoch: Option<u64>,
    pub path: PathBuf,
    pub registered_epoch: u64,
    pub updated_epoch: u64,
}

pub fn detect_changes(summary: &ScanSummary, state_path: &Path) -> AppResult<Vec<ModChange>> {
    let previous = read_state(state_path)?;
    let previous_by_key = previous
        .into_iter()
        .map(|record| (state_key(record.source, &record.key), record))
        .collect::<HashMap<_, _>>();

    let mut changes = Vec::new();
    for record in summary
        .game_mods
        .iter()
        .chain(summary.vault_mods.iter())
        .chain(summary.external_manager_mods.iter())
    {
        let key = record_state_key(record);
        match previous_by_key.get(&key) {
            None => changes.push(ModChange {
                kind: ModChangeKind::New,
                record: record.clone(),
            }),
            Some(previous) if fingerprint_changed(&record.fingerprint, previous) => {
                changes.push(ModChange {
                    kind: ModChangeKind::Updated,
                    record: record.clone(),
                });
            }
            Some(_) => {}
        }
    }

    changes.sort_by(|left, right| {
        left.record
            .source
            .as_key()
            .cmp(right.record.source.as_key())
            .then_with(|| left.record.stable_key().cmp(&right.record.stable_key()))
    });
    Ok(changes)
}

pub fn write_state(summary: &ScanSummary, state_path: &Path) -> AppResult<()> {
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }

    let now = epoch_now();
    let previous = read_state(state_path)?
        .into_iter()
        .map(|record| (state_key(record.source, &record.key), record))
        .collect::<HashMap<_, _>>();

    let mut output =
        String::from("source\tkey\tbytes\tmodified_epoch\tregistered_epoch\tupdated_epoch\tpath\n");
    for record in summary
        .game_mods
        .iter()
        .chain(summary.vault_mods.iter())
        .chain(summary.external_manager_mods.iter())
    {
        let key = record_state_key(record);
        let previous_record = previous.get(&key);
        let modified = epoch_seconds(record.fingerprint.modified)
            .map(|value| value.to_string())
            .unwrap_or_default();
        let fallback_epoch = epoch_seconds(record.fingerprint.modified).unwrap_or(now);
        let registered_epoch = previous_record
            .map(|record| record.registered_epoch)
            .unwrap_or(fallback_epoch);
        let updated_epoch = match previous_record {
            Some(previous) if !fingerprint_changed(&record.fingerprint, previous) => {
                previous.updated_epoch
            }
            Some(_) | None => now,
        };
        output.push_str(normalized_state_source(record.source, &record.path).as_key());
        output.push('\t');
        output.push_str(&record.stable_key());
        output.push('\t');
        output.push_str(&record.fingerprint.bytes.to_string());
        output.push('\t');
        output.push_str(&modified);
        output.push('\t');
        output.push_str(&registered_epoch.to_string());
        output.push('\t');
        output.push_str(&updated_epoch.to_string());
        output.push('\t');
        output.push_str(&record.path.to_string_lossy());
        output.push('\n');
    }

    fs::write(state_path, output).map_err(|source| AppError::io(state_path, source))
}

pub fn read_mod_state_index(state_path: &Path) -> AppResult<HashMap<String, ModStateEntry>> {
    Ok(read_state(state_path)?
        .into_iter()
        .map(|record| (state_key(record.source, &record.key), record))
        .collect())
}

pub fn mod_state_key(source: ModSource, key: &str) -> String {
    state_key(source, key)
}

pub fn desired_active_mods_path(state_dir: &Path) -> PathBuf {
    state_dir.join(DESIRED_ACTIVE_MODS_FILE)
}

pub fn desired_active_mod_keys(
    summary: &ScanSummary,
    state_dir: &Path,
) -> AppResult<BTreeSet<String>> {
    let path = desired_active_mods_path(state_dir);
    if path.exists() {
        return read_desired_active_mod_keys(&path);
    }

    Ok(summary
        .game_mods
        .iter()
        .map(|record| record.stable_key())
        .collect())
}

pub fn read_desired_active_mod_keys(path: &Path) -> AppResult<BTreeSet<String>> {
    if !path.exists() {
        return Ok(BTreeSet::new());
    }

    let content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

pub fn write_desired_active_mod_keys(keys: &BTreeSet<String>, state_dir: &Path) -> AppResult<()> {
    fs::create_dir_all(state_dir).map_err(|source| AppError::io(state_dir, source))?;
    let path = desired_active_mods_path(state_dir);
    let mut output = String::from("# Desired active mod keys. Applied when launching modded.\n");
    for key in keys {
        output.push_str(key);
        output.push('\n');
    }
    fs::write(&path, output).map_err(|source| AppError::io(&path, source))
}

pub fn mod_record_state_key(record: &ModRecord) -> String {
    record_state_key(record)
}

fn read_state(path: &Path) -> AppResult<Vec<ModStateEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    let mut records = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if index == 0 && line.starts_with("source\tkey\t") {
            continue;
        }

        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() != 5 && parts.len() != 7 {
            continue;
        }

        let Some(source) = ModSource::from_key(parts[0]) else {
            continue;
        };
        let Ok(bytes) = parts[2].parse::<u64>() else {
            continue;
        };
        let modified_epoch = if parts[3].is_empty() {
            None
        } else {
            parts[3].parse::<u64>().ok()
        };
        let fallback_epoch = modified_epoch.unwrap_or_else(epoch_now);
        let (registered_epoch, updated_epoch, path_index) = if parts.len() == 7 {
            (
                parts[4].parse::<u64>().unwrap_or(fallback_epoch),
                parts[5].parse::<u64>().unwrap_or(fallback_epoch),
                6,
            )
        } else {
            (fallback_epoch, fallback_epoch, 4)
        };

        let path = PathBuf::from(parts[path_index]);
        records.push(ModStateEntry {
            source: normalized_state_source(source, &path),
            key: parts[1].to_string(),
            bytes,
            modified_epoch,
            registered_epoch,
            updated_epoch,
            path,
        });
    }

    Ok(records)
}

fn record_state_key(record: &ModRecord) -> String {
    state_key(
        normalized_state_source(record.source, &record.path),
        &record.stable_key(),
    )
}

fn normalized_state_source(source: ModSource, path: &Path) -> ModSource {
    if source == ModSource::Vault && is_game_disabled_path(path) {
        ModSource::GameMods
    } else {
        source
    }
}

fn is_game_disabled_path(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor
            .file_name()
            .map(|name| {
                let name = name.to_string_lossy();
                name.eq_ignore_ascii_case("mods.disabled") || name.eq_ignore_ascii_case(".disabled")
            })
            .unwrap_or(false)
    })
}

fn state_key(source: ModSource, key: &str) -> String {
    format!("{}:{key}", source.as_key())
}

fn fingerprint_changed(current: &ModFingerprint, previous: &ModStateEntry) -> bool {
    current.bytes != previous.bytes || epoch_seconds(current.modified) != previous.modified_epoch
}

fn epoch_now() -> u64 {
    epoch_seconds(Some(SystemTime::now())).unwrap_or(0)
}

fn epoch_seconds(time: Option<SystemTime>) -> Option<u64> {
    let time = time?;
    time.duration_since(UNIX_EPOCH)
        .or_else(|_| Ok::<Duration, Duration>(Duration::from_secs(0)))
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ModKind, ModRecord, ScanSummary};
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn detects_new_and_updated_records() {
        let state_path = test_state_path("detects_new_and_updated_records");
        let first = ScanSummary {
            game_mods: Vec::new(),
            vault_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 10)],
        };
        write_state(&first, &state_path).expect("write first state");

        let second = ScanSummary {
            game_mods: Vec::new(),
            vault_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 20), record("Beta-v1", 5)],
        };

        let changes = detect_changes(&second, &state_path).expect("detect changes");

        assert_eq!(changes.len(), 2);
        assert!(
            changes
                .iter()
                .any(|change| change.kind == ModChangeKind::New)
        );
        assert!(
            changes
                .iter()
                .any(|change| change.kind == ModChangeKind::Updated)
        );
    }

    #[test]
    fn reads_legacy_state_without_lifecycle_columns() {
        let state_path = test_state_path("reads_legacy_state_without_lifecycle_columns");
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(
            &state_path,
            "source\tkey\tbytes\tmodified_epoch\tpath\nexternal\talpha\t10\t123\tC:/mods/alpha.zip\n",
        )
        .expect("write legacy state");

        let records = read_state(&state_path).expect("read state");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].registered_epoch, 123);
        assert_eq!(records[0].updated_epoch, 123);
    }

    #[test]
    fn preserves_registered_epoch_and_updates_changed_records() {
        let state_path = test_state_path("preserves_registered_epoch_and_updates_changed_records");
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(
            &state_path,
            "source\tkey\tbytes\tmodified_epoch\tregistered_epoch\tupdated_epoch\tpath\nexternal\talpha-v1\t10\t10\t100\t110\tC:/mods/Alpha-v1.zip\n",
        )
        .expect("write state");
        let summary = ScanSummary {
            game_mods: Vec::new(),
            vault_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 20)],
        };

        write_state(&summary, &state_path).expect("write updated state");
        let records = read_state(&state_path).expect("read updated state");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].registered_epoch, 100);
        assert!(records[0].updated_epoch >= 110);
    }

    #[test]
    fn moving_between_active_and_disabled_game_folders_is_not_a_content_change() {
        let state_path = test_state_path(
            "moving_between_active_and_disabled_game_folders_is_not_a_content_change",
        );
        let first = ScanSummary {
            game_mods: vec![record_at(
                "Alpha-v1",
                10,
                ModSource::GameMods,
                "C:/game/mods/Alpha-v1.zip",
            )],
            vault_mods: Vec::new(),
            external_manager_mods: Vec::new(),
        };
        write_state(&first, &state_path).expect("write first state");

        let second = ScanSummary {
            game_mods: Vec::new(),
            vault_mods: vec![record_at(
                "Alpha-v1",
                10,
                ModSource::Vault,
                "C:/game/mods.disabled/Alpha-v1.zip",
            )],
            external_manager_mods: Vec::new(),
        };

        let changes = detect_changes(&second, &state_path).expect("detect changes");
        write_state(&second, &state_path).expect("write disabled state");
        let records = read_state(&state_path).expect("read disabled state");

        assert!(changes.is_empty());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source, ModSource::GameMods);
    }

    fn record(name: &str, bytes: u64) -> ModRecord {
        record_at(
            name,
            bytes,
            ModSource::ExternalManager,
            &format!("C:/mods/{name}.zip"),
        )
    }

    fn record_at(name: &str, bytes: u64, source: ModSource, path: &str) -> ModRecord {
        ModRecord {
            name: name.to_string(),
            path: PathBuf::from(path),
            source,
            kind: ModKind::Archive,
            version_hint: None,
            fingerprint: ModFingerprint {
                bytes,
                modified: Some(UNIX_EPOCH + Duration::from_secs(bytes)),
            },
        }
    }

    fn test_state_path(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-work");
        path.push(format!(
            "{}-{}.tsv",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        path
    }
}
