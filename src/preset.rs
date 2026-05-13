use crate::discovery::scan_mod_directory;
use crate::domain::{ModRecord, ModSource};
use crate::error::{AppError, AppResult};
use crate::process::{powershell_compress_directory_contents, powershell_expand_archive};
use crate::vault::{self, DisabledModAction};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub keys: Vec<String>,
    pub mods: Vec<PresetMod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetMod {
    pub key: String,
    pub version_hint: Option<String>,
    pub bytes: u64,
    pub modified_epoch: Option<u64>,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetApplyReport {
    pub disabled: Vec<DisabledModAction>,
    pub enabled: Vec<DisabledModAction>,
    pub missing: Vec<String>,
    pub version_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetExportReport {
    pub archive_path: PathBuf,
    pub included_mods: usize,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetImportReport {
    pub preset: Preset,
    pub imported_mods: usize,
}

pub fn save_from_enabled(
    name: &str,
    presets_dir: &Path,
    game_mods_dir: &Path,
) -> AppResult<Preset> {
    let name = normalize_preset_name(name)?;
    fs::create_dir_all(presets_dir).map_err(|source| AppError::io(presets_dir, source))?;

    let mut mods = scan_mod_directory(game_mods_dir, ModSource::GameMods)?
        .into_iter()
        .map(|record| preset_mod_from_record(&record))
        .collect::<Vec<_>>();
    mods.sort_by(|left, right| left.key.cmp(&right.key));
    mods.dedup_by(|left, right| left.key == right.key);
    let mut keys = mods.iter().map(|item| item.key.clone()).collect::<Vec<_>>();
    keys.sort();
    keys.dedup();

    let preset = Preset { name, keys, mods };
    write_preset(&preset, presets_dir)?;
    Ok(preset)
}

pub fn list_presets(presets_dir: &Path) -> AppResult<Vec<Preset>> {
    if !presets_dir.exists() {
        return Ok(Vec::new());
    }

    let mut presets = Vec::new();
    for entry in fs::read_dir(presets_dir).map_err(|source| AppError::io(presets_dir, source))? {
        let entry = entry.map_err(|source| AppError::io(presets_dir, source))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("txt") {
            presets.push(read_preset(&path)?);
        }
    }

    presets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(presets)
}

pub fn load_preset(name: &str, presets_dir: &Path) -> AppResult<Preset> {
    let name = normalize_preset_name(name)?;
    let path = preset_path(presets_dir, &name);
    if !path.exists() {
        return Err(AppError::InvalidCommand(format!(
            "preset not found: {name}"
        )));
    }
    read_preset(&path)
}

pub fn apply_preset(
    name: &str,
    presets_dir: &Path,
    game_mods_dir: &Path,
    vendor_dir: &Path,
) -> AppResult<PresetApplyReport> {
    let preset = load_preset(name, presets_dir)?;
    let disabled = vault::disable_all(game_mods_dir)?;
    let inactive_records = vault::list_disabled_game_mods(game_mods_dir)?;

    let mut enabled = Vec::new();
    let mut missing = Vec::new();
    let mut version_warnings = Vec::new();

    for key in preset.keys.clone() {
        if inactive_records
            .iter()
            .any(|record| record.stable_key() == key)
        {
            if let Some(expected) = preset.mods.iter().find(|item| item.key == key)
                && let Some(warning) = version_warning(expected, game_mods_dir)
            {
                version_warnings.push(warning);
            }
            enabled.push(vault::enable_mod(&key, game_mods_dir, vendor_dir)?);
        } else {
            missing.push(key);
        }
    }

    Ok(PresetApplyReport {
        disabled,
        enabled,
        missing,
        version_warnings,
    })
}

pub fn export_preset_archive(
    name: &str,
    presets_dir: &Path,
    game_mods_dir: &Path,
    archive_path: &Path,
) -> AppResult<PresetExportReport> {
    let preset = load_preset(name, presets_dir)?;
    let staging_dir = presets_dir.join(format!(".export-{}-{}", preset.name, timestamp_string()));
    let mods_dir = staging_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|source| AppError::io(&mods_dir, source))?;
    write_preset(&preset, &staging_dir)?;
    fs::copy(
        preset_path(&staging_dir, &preset.name),
        staging_dir.join("preset.txt"),
    )
    .map_err(|source| AppError::io(&staging_dir, source))?;

    let available_records = preset_available_records(game_mods_dir)?;
    let mut included_mods = 0;
    let mut missing = Vec::new();

    for key in &preset.keys {
        if let Some(record) = available_records
            .iter()
            .find(|record| &record.stable_key() == key)
        {
            let target = mods_dir.join(
                record
                    .path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new(&record.name)),
            );
            copy_path(&record.path, &target)?;
            included_mods += 1;
        } else {
            missing.push(key.clone());
        }
    }

    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    compress_directory(&staging_dir, archive_path)?;
    let _ = fs::remove_dir_all(&staging_dir);

    Ok(PresetExportReport {
        archive_path: archive_path.to_path_buf(),
        included_mods,
        missing,
    })
}

pub fn import_preset_archive(
    archive_path: &Path,
    presets_dir: &Path,
    game_mods_dir: &Path,
) -> AppResult<PresetImportReport> {
    if !archive_path.exists() {
        return Err(AppError::InvalidCommand(format!(
            "preset archive not found: {}",
            archive_path.display()
        )));
    }

    let staging_dir = presets_dir.join(format!(".import-{}", timestamp_string()));
    fs::create_dir_all(&staging_dir).map_err(|source| AppError::io(&staging_dir, source))?;
    expand_archive(archive_path, &staging_dir)?;

    let preset_path = staging_dir.join("preset.txt");
    let preset = read_preset(&preset_path)?;
    let mods_dir = staging_dir.join("mods");
    let mut imported_mods = 0;
    if mods_dir.exists() {
        for entry in fs::read_dir(&mods_dir).map_err(|source| AppError::io(&mods_dir, source))? {
            let entry = entry.map_err(|source| AppError::io(&mods_dir, source))?;
            vault::import_mod_to_disabled(&entry.path(), game_mods_dir)?;
            imported_mods += 1;
        }
    }
    fs::create_dir_all(presets_dir).map_err(|source| AppError::io(presets_dir, source))?;
    write_preset(&preset, presets_dir)?;
    let _ = fs::remove_dir_all(&staging_dir);

    Ok(PresetImportReport {
        preset,
        imported_mods,
    })
}

fn write_preset(preset: &Preset, presets_dir: &Path) -> AppResult<()> {
    let path = preset_path(presets_dir, &preset.name);
    let mut output = String::new();
    output.push_str("# Slay the Spire 2 Mod Manager preset\n");
    output.push_str(&format!("name={}\n", preset.name));
    output.push_str("[mods]\n");
    for item in preset_mods_for_write(preset) {
        output.push_str(&item.key);
        output.push('\t');
        output.push_str(item.version_hint.as_deref().unwrap_or(""));
        output.push('\t');
        output.push_str(&item.bytes.to_string());
        output.push('\t');
        output.push_str(
            &item
                .modified_epoch
                .map(|value| value.to_string())
                .unwrap_or_default(),
        );
        output.push('\t');
        output.push_str(&item.file_name);
        output.push('\n');
    }

    fs::write(&path, output).map_err(|source| AppError::io(path, source))
}

fn read_preset(path: &Path) -> AppResult<Preset> {
    let content = fs::read_to_string(path).map_err(|source| AppError::io(path, source))?;
    let fallback_name = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "preset".to_string());
    let mut name = fallback_name;
    let mut keys = Vec::new();
    let mut mods = Vec::new();
    let mut in_mods = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("name=") {
            name = value.trim().to_string();
            continue;
        }
        if line == "[mods]" {
            in_mods = true;
            continue;
        }
        if in_mods {
            let parts = line.split('\t').collect::<Vec<_>>();
            let key = parts.first().copied().unwrap_or("").trim().to_string();
            if key.is_empty() {
                continue;
            }
            keys.push(key.clone());
            if parts.len() >= 5 {
                mods.push(PresetMod {
                    key,
                    version_hint: optional_string(parts[1]),
                    bytes: parts[2].parse::<u64>().unwrap_or(0),
                    modified_epoch: if parts[3].is_empty() {
                        None
                    } else {
                        parts[3].parse::<u64>().ok()
                    },
                    file_name: parts[4].to_string(),
                });
            }
        }
    }

    keys.sort();
    keys.dedup();
    Ok(Preset { name, keys, mods })
}

fn preset_path(presets_dir: &Path, name: &str) -> PathBuf {
    presets_dir.join(format!("{name}.txt"))
}

fn preset_mods_for_write(preset: &Preset) -> Vec<PresetMod> {
    if !preset.mods.is_empty() {
        return preset.mods.clone();
    }

    preset
        .keys
        .iter()
        .map(|key| PresetMod {
            key: key.clone(),
            version_hint: None,
            bytes: 0,
            modified_epoch: None,
            file_name: String::new(),
        })
        .collect()
}

fn preset_mod_from_record(record: &ModRecord) -> PresetMod {
    PresetMod {
        key: record.stable_key(),
        version_hint: record.version_hint.clone(),
        bytes: record.fingerprint.bytes,
        modified_epoch: record
            .fingerprint
            .modified
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
        file_name: record
            .path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

fn preset_available_records(game_mods_dir: &Path) -> AppResult<Vec<ModRecord>> {
    let mut records = scan_mod_directory(game_mods_dir, ModSource::GameMods)?;
    records.extend(vault::list_disabled_game_mods(game_mods_dir)?);
    Ok(records)
}

fn version_warning(expected: &PresetMod, game_mods_dir: &Path) -> Option<String> {
    let record = vault::list_disabled_game_mods(game_mods_dir)
        .ok()?
        .into_iter()
        .find(|record| record.stable_key() == expected.key)?;
    let metadata = fs::metadata(&record.path).ok()?;
    let bytes = if metadata.is_dir() {
        directory_bytes(&record.path).unwrap_or(0)
    } else {
        metadata.len()
    };
    let modified_epoch = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());

    if expected.bytes != 0 && (expected.bytes != bytes || expected.modified_epoch != modified_epoch)
    {
        Some(format!(
            "{} version differs from preset metadata",
            expected.key
        ))
    } else {
        None
    }
}

fn directory_bytes(path: &Path) -> AppResult<u64> {
    let mut total = 0;
    for entry in fs::read_dir(path).map_err(|source| AppError::io(path, source))? {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        let metadata = entry
            .metadata()
            .map_err(|source| AppError::io(entry.path(), source))?;
        if metadata.is_dir() {
            total += directory_bytes(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
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
            .map_err(|error| AppError::io(source, error))
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

fn compress_directory(source_dir: &Path, archive_path: &Path) -> AppResult<()> {
    run_powershell_archive_command(
        powershell_compress_directory_contents(source_dir, archive_path),
        archive_path,
    )
}

fn expand_archive(archive_path: &Path, destination: &Path) -> AppResult<()> {
    run_powershell_archive_command(
        powershell_expand_archive(archive_path, destination),
        archive_path,
    )
}

fn run_powershell_archive_command(
    status: std::io::Result<std::process::ExitStatus>,
    error_path: &Path,
) -> AppResult<()> {
    let status = status.map_err(|source| AppError::io(error_path, source))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::InvalidCommand(format!(
            "archive command failed for {}",
            error_path.display()
        )))
    }
}

fn optional_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn normalize_preset_name(name: &str) -> AppResult<String> {
    let normalized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if normalized.is_empty() {
        Err(AppError::InvalidCommand(
            "preset name must contain letters or numbers".to_string(),
        ))
    } else {
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn saves_current_enabled_mod_keys() {
        let fixture = TestWorkspace::create("saves_current_enabled_mod_keys");
        fixture.write_file("game/mods/Alpha-v1.zip", "alpha");
        fixture.write_file("game/mods/Beta-v2.zip", "beta");

        let preset = save_from_enabled(
            "daily run",
            &fixture.presets_dir(),
            &fixture.game_mods_dir(),
        )
        .expect("save preset");

        assert_eq!(preset.name, "daily-run");
        assert_eq!(preset.keys, vec!["alpha-v1", "beta-v2"]);
    }

    #[test]
    fn applies_preset_and_reports_missing_mods() {
        let fixture = TestWorkspace::create("applies_preset_and_reports_missing_mods");
        fixture.write_file("game/mods.disabled/Alpha-v1.jar", "alpha");
        fs::create_dir_all(fixture.presets_dir()).expect("preset dir");
        fs::write(
            fixture.presets_dir().join("daily.txt"),
            "name=daily\n[mods]\nalpha-v1\nmissing-v1\n",
        )
        .expect("write preset");

        let report = apply_preset(
            "daily",
            &fixture.presets_dir(),
            &fixture.game_mods_dir(),
            &fixture.vendor_dir(),
        )
        .expect("apply");

        assert_eq!(report.enabled.len(), 1);
        assert_eq!(report.missing, vec!["missing-v1"]);
        assert!(fixture.path.join("game/mods/Alpha-v1.jar").exists());
    }

    #[test]
    fn exports_and_imports_preset_archive() {
        let fixture = TestWorkspace::create("exports_and_imports_preset_archive");
        fixture.write_file("game/mods/Alpha-v1.zip", "alpha");
        let preset =
            save_from_enabled("portable", &fixture.presets_dir(), &fixture.game_mods_dir())
                .expect("save preset");
        let archive = fixture.path.join("portable.zip");

        let export_report = export_preset_archive(
            &preset.name,
            &fixture.presets_dir(),
            &fixture.game_mods_dir(),
            &archive,
        )
        .expect("export preset");
        let import_root = TestWorkspace::create("imports_preset_archive_target");
        let import_report = import_preset_archive(
            &archive,
            &import_root.presets_dir(),
            &import_root.game_mods_dir(),
        )
        .expect("import preset");

        assert!(export_report.archive_path.exists());
        assert_eq!(export_report.included_mods, 1);
        assert_eq!(import_report.preset.name, "portable");
        assert_eq!(import_report.imported_mods, 1);
        assert!(import_root.path.join("presets/portable.txt").exists());
        assert!(
            import_root
                .path
                .join("game/mods.disabled/Alpha-v1.zip")
                .exists()
        );
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

        fn presets_dir(&self) -> PathBuf {
            self.path.join("presets")
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
