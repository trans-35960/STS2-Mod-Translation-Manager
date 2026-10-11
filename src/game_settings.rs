//! Reads and edits the per-mod enable list in the game's `settings.save`.
//!
//! Slay the Spire 2 keeps `mod_settings.mod_list = [{ id, is_enabled, source }]`
//! next to the save profiles. Workshop mods cannot be moved out of Steam's
//! download folder (Steam would restore them), so this list is the only way to
//! turn them off; local `mods/` entries share the same list.

use crate::error::{AppError, AppResult};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SOURCE_WORKSHOP: &str = "steam_workshop";
pub const SOURCE_MODS_DIRECTORY: &str = "mods_directory";

const SETTINGS_BACKUP_DIR: &str = "game-settings";
const SETTINGS_BACKUP_KEEP: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameModEntry {
    pub id: String,
    pub source: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameModChange {
    pub id: String,
    pub source: &'static str,
    pub enabled: bool,
}

/// Returns the game's mod list; a missing or unreadable file yields an empty list
/// so a fresh install still scans.
pub fn read_mod_entries(settings_path: &Path) -> Vec<GameModEntry> {
    let Some(root) = read_settings(settings_path).ok().flatten() else {
        return Vec::new();
    };
    root.get("mod_settings")
        .and_then(|settings| settings.get("mod_list"))
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(entry_from_value).collect())
        .unwrap_or_default()
}

/// Applies `changes` to `settings.save`, inserting entries the game has not
/// recorded yet. The original file is copied to `backup_root` before the first
/// write. Returns how many entries actually changed; nothing is written when the
/// file is missing (the game creates it on first launch) or already matches.
pub fn apply_mod_changes(
    settings_path: &Path,
    backup_root: &Path,
    changes: &[GameModChange],
) -> AppResult<usize> {
    if changes.is_empty() {
        return Ok(0);
    }
    let Some(mut root) = read_settings(settings_path)? else {
        return Ok(0);
    };
    let changed = apply_changes_to_value(&mut root, changes)?;
    if changed == 0 {
        return Ok(0);
    }

    backup_settings(settings_path, backup_root)?;
    let text = serde_json::to_string_pretty(&root).map_err(|error| {
        AppError::InvalidCommand(format!("게임 설정을 저장하지 못했습니다: {error}"))
    })?;
    write_atomically(settings_path, text.as_bytes())?;
    Ok(changed)
}

fn read_settings(settings_path: &Path) -> AppResult<Option<Value>> {
    if !settings_path.is_file() {
        return Ok(None);
    }
    let text =
        fs::read_to_string(settings_path).map_err(|source| AppError::io(settings_path, source))?;
    let value =
        serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).map_err(|error| {
            AppError::InvalidCommand(format!(
                "게임 설정 파일을 읽을 수 없습니다: {} ({error})",
                settings_path.display()
            ))
        })?;
    if !value.is_object() {
        return Err(AppError::InvalidCommand(format!(
            "게임 설정 파일 형식이 올바르지 않습니다: {}",
            settings_path.display()
        )));
    }
    Ok(Some(value))
}

fn entry_from_value(value: &Value) -> Option<GameModEntry> {
    Some(GameModEntry {
        id: value.get("id")?.as_str()?.to_string(),
        source: value
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_enabled: value
            .get("is_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn apply_changes_to_value(root: &mut Value, changes: &[GameModChange]) -> AppResult<usize> {
    let root = root
        .as_object_mut()
        .expect("settings root checked as object");
    let mod_settings = root
        .entry("mod_settings")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(mod_settings) = mod_settings.as_object_mut() else {
        return Err(AppError::InvalidCommand(
            "게임 설정의 mod_settings 형식이 올바르지 않습니다.".to_string(),
        ));
    };
    let list = mod_settings
        .entry("mod_list")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(list) = list.as_array_mut() else {
        return Err(AppError::InvalidCommand(
            "게임 설정의 mod_list 형식이 올바르지 않습니다.".to_string(),
        ));
    };

    let mut changed = 0;
    for change in changes {
        let existing = list.iter_mut().find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(change.id.as_str())
                && entry.get("source").and_then(Value::as_str) == Some(change.source)
        });
        match existing {
            Some(entry) => {
                if entry.get("is_enabled").and_then(Value::as_bool) != Some(change.enabled) {
                    entry["is_enabled"] = Value::Bool(change.enabled);
                    changed += 1;
                }
            }
            None => {
                list.push(json!({
                    "id": change.id,
                    "is_enabled": change.enabled,
                    "source": change.source,
                }));
                changed += 1;
            }
        }
    }
    Ok(changed)
}

fn backup_settings(settings_path: &Path, backup_root: &Path) -> AppResult<()> {
    let backup_dir = backup_root.join(SETTINGS_BACKUP_DIR);
    fs::create_dir_all(&backup_dir).map_err(|source| AppError::io(&backup_dir, source))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let target = backup_dir.join(format!("settings.save.{stamp}"));
    fs::copy(settings_path, &target).map_err(|source| AppError::io(settings_path, source))?;
    prune_backups(&backup_dir);
    Ok(())
}

fn prune_backups(backup_dir: &Path) {
    let Ok(entries) = fs::read_dir(backup_dir) else {
        return;
    };
    let mut backups = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("settings.save."))
        })
        .collect::<Vec<PathBuf>>();
    // Millisecond stamps share a width for centuries, so name order is age order.
    backups.sort();
    let excess = backups.len().saturating_sub(SETTINGS_BACKUP_KEEP);
    for path in backups.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let temp = path.with_extension("save.sts2mm-tmp");
    fs::write(&temp, bytes).map_err(|source| AppError::io(&temp, source))?;
    fs::rename(&temp, path).map_err(|source| {
        let _ = fs::remove_file(&temp);
        AppError::io(path, source)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
  "language": "kor",
  "mod_settings": {
    "mod_list": [
      { "id": "STS2-RitsuLib", "is_enabled": true, "source": "steam_workshop" },
      { "id": "LustTravel2", "is_enabled": false, "source": "steam_workshop" }
    ],
    "mods_enabled": true
  },
  "schema_version": 8
}"#;

    #[test]
    fn reads_mod_entries() {
        let dir = test_dir("reads_mod_entries");
        let path = dir.join("settings.save");
        fs::write(&path, SAMPLE).expect("write");

        let entries = read_mod_entries(&path);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].id, "LustTravel2");
        assert!(!entries[1].is_enabled);
        assert_eq!(entries[1].source, SOURCE_WORKSHOP);
    }

    #[test]
    fn missing_settings_reads_empty_and_skips_writes() {
        let dir = test_dir("missing_settings_reads_empty_and_skips_writes");
        let path = dir.join("settings.save");

        assert!(read_mod_entries(&path).is_empty());
        let changed = apply_mod_changes(
            &path,
            &dir,
            &[GameModChange {
                id: "Any".to_string(),
                source: SOURCE_WORKSHOP,
                enabled: false,
            }],
        )
        .expect("apply");
        assert_eq!(changed, 0);
        assert!(!path.exists());
    }

    #[test]
    fn applies_changes_keeps_other_settings_and_backs_up() {
        let dir = test_dir("applies_changes_keeps_other_settings_and_backs_up");
        let path = dir.join("settings.save");
        fs::write(&path, SAMPLE).expect("write");

        let changed = apply_mod_changes(
            &path,
            &dir.join("backups"),
            &[
                GameModChange {
                    id: "STS2-RitsuLib".to_string(),
                    source: SOURCE_WORKSHOP,
                    enabled: true,
                },
                GameModChange {
                    id: "LustTravel2".to_string(),
                    source: SOURCE_WORKSHOP,
                    enabled: true,
                },
                GameModChange {
                    id: "MpLib".to_string(),
                    source: SOURCE_WORKSHOP,
                    enabled: false,
                },
            ],
        )
        .expect("apply");

        assert_eq!(changed, 2);
        let entries = read_mod_entries(&path);
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .all(|entry| entry.id != "LustTravel2" || entry.is_enabled)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.id == "MpLib" && !entry.is_enabled)
        );
        let root: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(root["language"], "kor");
        assert_eq!(root["schema_version"], 8);
        assert_eq!(root["mod_settings"]["mods_enabled"], true);
        let backups = fs::read_dir(dir.join("backups").join(SETTINGS_BACKUP_DIR))
            .expect("backup dir")
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn unchanged_settings_are_not_rewritten() {
        let dir = test_dir("unchanged_settings_are_not_rewritten");
        let path = dir.join("settings.save");
        fs::write(&path, SAMPLE).expect("write");

        let changed = apply_mod_changes(
            &path,
            &dir.join("backups"),
            &[GameModChange {
                id: "STS2-RitsuLib".to_string(),
                source: SOURCE_WORKSHOP,
                enabled: true,
            }],
        )
        .expect("apply");

        assert_eq!(changed, 0);
        assert_eq!(fs::read_to_string(&path).expect("read"), SAMPLE);
        assert!(!dir.join("backups").exists());
    }

    fn test_dir(name: &str) -> PathBuf {
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
        fs::create_dir_all(&path).expect("create test dir");
        path
    }
}
