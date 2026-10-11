use crate::domain::{ModChange, ModChangeKind, ModFingerprint, ModRecord, ModSource, ScanSummary};
use crate::error::{AppError, AppResult};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DESIRED_ACTIVE_MODS_FILE: &str = "desired_active_mods.txt";
const KNOWN_WORKSHOP_MODS_FILE: &str = "known_workshop_mods.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStateEntry {
    pub key: String,
    pub source: ModSource,
    pub bytes: u64,
    pub modified_epoch: Option<u64>,
    pub path: PathBuf,
    pub registered_epoch: u64,
    pub updated_epoch: u64,
    /// Version recorded at the last acknowledged scan; `None` for older indexes.
    pub version: Option<String>,
}

pub fn detect_changes(summary: &ScanSummary, state_path: &Path) -> AppResult<Vec<ModChange>> {
    let previous = read_state(state_path)?;
    let previous_by_key = previous
        .into_iter()
        .map(|record| (state_key(record.source, &record.key), record))
        .collect::<HashMap<_, _>>();

    let mut changes = Vec::new();
    for record in summary.records() {
        let key = record_state_key(record);
        match previous_by_key.get(&key) {
            None => changes.push(ModChange {
                kind: ModChangeKind::New,
                record: record.clone(),
            }),
            Some(previous)
                if fingerprint_changed(&record.fingerprint, previous)
                    || version_change(record, previous).is_some() =>
            {
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

    let mut output = String::from(
        "source\tkey\tbytes\tmodified_epoch\tregistered_epoch\tupdated_epoch\tpath\tversion\n",
    );
    for record in summary.records() {
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
            Some(previous)
                if !fingerprint_changed(&record.fingerprint, previous)
                    && version_change(record, previous).is_none() =>
            {
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
        output.push('\t');
        output.push_str(&state_version(record).unwrap_or_default());
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
    let mut desired = if path.exists() {
        read_desired_active_mod_keys(&path)?
    } else {
        summary
            .game_mods
            .iter()
            .map(|record| record.stable_key())
            .collect()
    };

    // Workshop mods appear without passing through this app, so until the user
    // decides otherwise here, follow whatever the game itself has them set to.
    let known_workshop = read_key_list(&state_dir.join(KNOWN_WORKSHOP_MODS_FILE))?;
    for record in &summary.workshop_mods {
        let key = record.stable_key();
        if !known_workshop.contains(&key) && summary.workshop_enabled_in_game(&key) {
            desired.insert(key);
        }
    }
    Ok(desired)
}

pub fn read_desired_active_mod_keys(path: &Path) -> AppResult<BTreeSet<String>> {
    read_key_list(path)
}

fn read_key_list(path: &Path) -> AppResult<BTreeSet<String>> {
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

/// Persists the desired set. Every workshop mod in `summary` is also recorded as
/// known, so its desired state is no longer seeded from the game's settings.
pub fn write_desired_active_mod_keys(
    keys: &BTreeSet<String>,
    summary: &ScanSummary,
    state_dir: &Path,
) -> AppResult<()> {
    fs::create_dir_all(state_dir).map_err(|source| AppError::io(state_dir, source))?;
    write_key_list(
        &desired_active_mods_path(state_dir),
        "# Desired active mod keys. Applied when launching modded.",
        keys,
    )?;

    if summary.workshop_mods.is_empty() {
        return Ok(());
    }
    let known_path = state_dir.join(KNOWN_WORKSHOP_MODS_FILE);
    let mut known = read_key_list(&known_path)?;
    let before = known.len();
    known.extend(summary.workshop_mods.iter().map(ModRecord::stable_key));
    if known.len() == before {
        return Ok(());
    }
    write_key_list(
        &known_path,
        "# Workshop mod keys whose desired state is managed by this app.",
        &known,
    )
}

fn write_key_list(path: &Path, header: &str, keys: &BTreeSet<String>) -> AppResult<()> {
    let mut output = format!("{header}\n");
    for key in keys {
        output.push_str(key);
        output.push('\n');
    }
    fs::write(path, output).map_err(|source| AppError::io(path, source))
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
        // Seven columns predate the version column; keep reading those indexes.
        if parts.len() != 7 && parts.len() != 8 {
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
        let registered_epoch = parts[4].parse::<u64>().unwrap_or(fallback_epoch);
        let updated_epoch = parts[5].parse::<u64>().unwrap_or(fallback_epoch);

        let path = PathBuf::from(parts[6]);
        records.push(ModStateEntry {
            source: normalized_state_source(source, &path),
            key: parts[1].to_string(),
            bytes,
            modified_epoch,
            registered_epoch,
            updated_epoch,
            path,
            version: parts
                .get(7)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_string),
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
    if source == ModSource::Disabled && is_game_disabled_path(path) {
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
                name.eq_ignore_ascii_case("mods.disabled")
            })
            .unwrap_or(false)
    })
}

fn state_key(source: ModSource, key: &str) -> String {
    format!("{}:{key}", source.as_key())
}

/// `(previous, current)` when both scans know the version and it differs.
/// Workshop mods update silently through Steam, so this is how those updates surface.
pub fn version_change(record: &ModRecord, previous: &ModStateEntry) -> Option<(String, String)> {
    let previous_version = previous.version.as_deref()?;
    let current_version = state_version(record)?;
    (normalize_version(previous_version) != normalize_version(&current_version))
        .then(|| (previous_version.to_string(), current_version))
}

fn state_version(record: &ModRecord) -> Option<String> {
    record
        .version_hint
        .as_deref()
        .map(|value| value.replace(['\t', '\r', '\n'], " ").trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_version(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_ascii_lowercase()
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
            disabled_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 10)],
            ..ScanSummary::default()
        };
        write_state(&first, &state_path).expect("write first state");

        let second = ScanSummary {
            game_mods: Vec::new(),
            disabled_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 20), record("Beta-v1", 5)],
            ..ScanSummary::default()
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
            disabled_mods: Vec::new(),
            external_manager_mods: vec![record("Alpha-v1", 20)],
            ..ScanSummary::default()
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
            disabled_mods: Vec::new(),
            external_manager_mods: Vec::new(),
            ..ScanSummary::default()
        };
        write_state(&first, &state_path).expect("write first state");

        let second = ScanSummary {
            game_mods: Vec::new(),
            disabled_mods: vec![record_at(
                "Alpha-v1",
                10,
                ModSource::Disabled,
                "C:/game/mods.disabled/Alpha-v1.zip",
            )],
            external_manager_mods: Vec::new(),
            ..ScanSummary::default()
        };

        let changes = detect_changes(&second, &state_path).expect("detect changes");
        write_state(&second, &state_path).expect("write disabled state");
        let records = read_state(&state_path).expect("read disabled state");

        assert!(changes.is_empty());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source, ModSource::GameMods);
    }

    #[test]
    fn version_changes_are_detected_and_old_indexes_stay_quiet() {
        let state_path = test_state_path("version_changes_are_detected");
        let mut workshop = record_at(
            "STS2-RitsuLib",
            10,
            ModSource::Workshop,
            "G:/workshop/content/2868840/3747602295",
        );
        workshop.version_hint = Some("0.6.6".to_string());
        let first = ScanSummary {
            workshop_mods: vec![workshop.clone()],
            ..ScanSummary::default()
        };
        write_state(&first, &state_path).expect("write first state");

        // Same files, same version: nothing to report.
        assert!(
            detect_changes(&first, &state_path)
                .expect("detect")
                .is_empty()
        );

        workshop.version_hint = Some("v0.6.6".to_string());
        let same_version = ScanSummary {
            workshop_mods: vec![workshop.clone()],
            ..ScanSummary::default()
        };
        assert!(
            detect_changes(&same_version, &state_path)
                .expect("detect")
                .is_empty()
        );

        workshop.version_hint = Some("0.6.7".to_string());
        let updated = ScanSummary {
            workshop_mods: vec![workshop.clone()],
            ..ScanSummary::default()
        };
        let changes = detect_changes(&updated, &state_path).expect("detect");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ModChangeKind::Updated);
        let index = read_mod_state_index(&state_path).expect("index");
        let previous = index
            .get(&mod_record_state_key(&workshop))
            .expect("previous entry");
        assert_eq!(
            version_change(&workshop, previous),
            Some(("0.6.6".to_string(), "0.6.7".to_string()))
        );

        // A seven-column index from before versions were recorded still loads.
        let legacy_path = test_state_path("version_changes_legacy_index");
        fs::create_dir_all(legacy_path.parent().expect("parent")).expect("dir");
        fs::write(
            &legacy_path,
            "source\tkey\tbytes\tmodified_epoch\tregistered_epoch\tupdated_epoch\tpath\n\
             workshop\tworkshop-sts2-ritsulib\t10\t10\t10\t10\tG:/workshop/content/2868840/3747602295\n",
        )
        .expect("write legacy");
        assert!(
            detect_changes(&updated, &legacy_path)
                .expect("detect")
                .is_empty()
        );
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
