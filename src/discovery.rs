use crate::domain::{ModFingerprint, ModKind, ModRecord, ModSource};
use crate::error::{AppError, AppResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn scan_mod_directory(path: &Path, source: ModSource) -> AppResult<Vec<ModRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(path).map_err(|source| AppError::io(path, source))?;
    let mut mods = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| AppError::io(&entry_path, source))?;

        if should_ignore(&entry_path) {
            continue;
        }

        let kind = classify_mod_path(&entry_path, metadata.is_dir());
        let name = mod_display_name(&entry_path, kind);
        let fingerprint = if metadata.is_dir() {
            fingerprint_directory(&entry_path)?
        } else {
            ModFingerprint {
                bytes: metadata.len(),
                modified: metadata.modified().ok(),
            }
        };

        let manifest_version = if kind == ModKind::Directory {
            find_mod_manifests(&entry_path, LOCAL_MANIFEST_MAX_DEPTH)
                .first()
                .and_then(|manifest| read_manifest_identity(manifest).version)
        } else {
            None
        };

        mods.push(ModRecord {
            version_hint: manifest_version.or_else(|| infer_version_hint(&name)),
            name,
            path: entry_path,
            source,
            kind,
            fingerprint,
        });
    }

    mods.sort_by_key(|record| record.stable_key());
    Ok(mods)
}

pub const MOD_MANIFEST_FILE: &str = "mod_manifest.json";
const WORKSHOP_MANIFEST_MAX_DEPTH: usize = 3;
const LOCAL_MANIFEST_MAX_DEPTH: usize = 2;

/// Scans a Steam Workshop content folder (`.../workshop/content/<appid>`).
///
/// Each subscribed item is a numbered folder; like the game, every
/// `mod_manifest.json` found inside it is treated as one mod, named by the
/// manifest `id` because that is what the game's settings refer to.
pub fn scan_workshop_directory(path: &Path) -> AppResult<Vec<ModRecord>> {
    if !path.is_dir() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(path).map_err(|source| AppError::io(path, source))?;
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        for manifest_path in find_mod_manifests(&entry.path(), WORKSHOP_MANIFEST_MAX_DEPTH) {
            let Some(mod_dir) = manifest_path.parent() else {
                continue;
            };
            let manifest = read_manifest_identity(&manifest_path);
            let name = manifest
                .id
                .or_else(|| {
                    mod_dir
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "unknown-mod".to_string());
            mods.push(ModRecord {
                version_hint: manifest.version.or_else(|| infer_version_hint(&name)),
                name,
                path: mod_dir.to_path_buf(),
                source: ModSource::Workshop,
                kind: ModKind::Directory,
                fingerprint: fingerprint_directory(mod_dir)?,
            });
        }
    }

    mods.sort_by_key(|record| record.stable_key());
    Ok(mods)
}

/// Finds `mod_manifest.json` files below `root`, without descending further
/// once a folder is known to be a mod root.
pub fn find_mod_manifests(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_mod_manifests(root, max_depth, &mut found);
    found.sort();
    found
}

fn collect_mod_manifests(dir: &Path, depth_left: usize, found: &mut Vec<PathBuf>) {
    let manifest = dir.join(MOD_MANIFEST_FILE);
    if manifest.is_file() {
        found.push(manifest);
        return;
    }
    if depth_left == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_mod_manifests(&entry.path(), depth_left - 1, found);
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ManifestIdentity {
    pub id: Option<String>,
    pub version: Option<String>,
}

pub fn read_manifest_identity(manifest_path: &Path) -> ManifestIdentity {
    let Ok(text) = fs::read_to_string(manifest_path) else {
        return ManifestIdentity::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
    else {
        return ManifestIdentity::default();
    };
    let field = |key: &str| {
        value
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    ManifestIdentity {
        id: field("id"),
        version: field("version"),
    }
}

fn mod_display_name(path: &Path, kind: ModKind) -> String {
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

fn should_ignore(path: &Path) -> bool {
    path.file_name()
        .map(|name| {
            let name = name.to_string_lossy();
            let lower = name.to_ascii_lowercase();
            let unprefixed = lower.trim_start_matches('_');
            name.starts_with('.')
                || name.eq_ignore_ascii_case("desktop.ini")
                || unprefixed == "vortex_staging_folder"
                || lower.starts_with("vortex.deployment.")
        })
        .unwrap_or(false)
}

fn classify_mod_path(path: &Path, is_dir: bool) -> ModKind {
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

fn fingerprint_directory(path: &Path) -> AppResult<ModFingerprint> {
    let mut bytes = 0;
    let mut modified: Option<SystemTime> = None;
    collect_directory_fingerprint(path, &mut bytes, &mut modified)?;
    Ok(ModFingerprint { bytes, modified })
}

fn collect_directory_fingerprint(
    path: &Path,
    bytes: &mut u64,
    modified: &mut Option<SystemTime>,
) -> AppResult<()> {
    let entries = fs::read_dir(path).map_err(|source| AppError::io(path, source))?;

    for entry in entries {
        let entry = entry.map_err(|source| AppError::io(path, source))?;
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| AppError::io(&entry_path, source))?;

        if metadata.is_dir() {
            collect_directory_fingerprint(&entry_path, bytes, modified)?;
        } else {
            *bytes += metadata.len();
            if let Ok(entry_modified) = metadata.modified()
                && modified.is_none_or(|current| entry_modified > current)
            {
                *modified = Some(entry_modified);
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

pub fn existing_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|path| path.exists()).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn missing_directory_scans_as_empty() {
        let result = scan_mod_directory(Path::new("missing-test-directory"), ModSource::GameMods)
            .expect("scan should not fail for a missing directory");

        assert!(result.is_empty());
    }

    #[test]
    fn scans_directory_mods() {
        let fixture = TestDir::create("scan_directory_mods");
        fixture.create_dir("CoolMod-v1.2.3");
        fixture.write_file("CoolMod-v1.2.3/config.json", "{}");

        let mods = scan_mod_directory(fixture.path(), ModSource::GameMods).expect("scan");

        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "CoolMod-v1.2.3");
        assert_eq!(mods[0].kind, ModKind::Directory);
        assert_eq!(mods[0].version_hint.as_deref(), Some("v1.2.3"));
        assert_eq!(mods[0].fingerprint.bytes, 2);
    }

    #[test]
    fn directory_mod_version_comes_from_manifest() {
        let fixture = TestDir::create("directory_mod_version_comes_from_manifest");
        fixture.write_file(
            "BaseLib-v1.0.0/BaseLib/mod_manifest.json",
            r#"{"id":"BaseLib","version":"1.2.0"}"#,
        );

        let mods = scan_mod_directory(fixture.path(), ModSource::GameMods).expect("scan");

        assert_eq!(mods[0].version_hint.as_deref(), Some("1.2.0"));
    }

    #[test]
    fn ignores_vortex_deployment_metadata() {
        let fixture = TestDir::create("ignores_vortex_deployment_metadata");
        fixture.write_file("vortex.deployment.slaythespire2-mod.json", "{}");
        fixture.create_dir("_vortex_staging_folder");
        fixture.write_file("_vortex_staging_folder/staged-mod.zip", "staged");
        fixture.create_dir("BaseLib");

        let mods = scan_mod_directory(fixture.path(), ModSource::GameMods).expect("scan");

        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "BaseLib");
    }

    #[test]
    fn scans_workshop_items_by_manifest_id() {
        let fixture = TestDir::create("scans_workshop_items_by_manifest_id");
        fixture.write_file(
            "3747602295/mod_manifest.json",
            r#"{"id":"STS2-RitsuLib","name":"RitsuLib","version":"0.6.7"}"#,
        );
        fixture.write_file("3747602295/STS2-RitsuLib.dll", "dll");
        fixture.write_file(
            "3780369635/LustTravel2/mod_manifest.json",
            r#"{"id":"LustTravel2"}"#,
        );
        fixture.create_dir("3799999999/still-downloading");

        let mods = scan_workshop_directory(fixture.path()).expect("scan");

        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "LustTravel2");
        assert_eq!(mods[0].stable_key(), "workshop-lusttravel2");
        assert_eq!(mods[0].workshop_item_id().as_deref(), Some("3780369635"));
        assert_eq!(mods[1].name, "STS2-RitsuLib");
        assert_eq!(mods[1].version_hint.as_deref(), Some("0.6.7"));
        assert_eq!(mods[1].workshop_item_id().as_deref(), Some("3747602295"));
        assert_eq!(mods[1].path, fixture.path().join("3747602295"));
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn create(name: &str) -> Self {
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
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn create_dir(&self, child: &str) {
            fs::create_dir_all(self.path.join(child)).expect("create child dir");
        }

        fn write_file(&self, child: &str, content: &str) {
            let path = self.path.join(child);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            let mut file = fs::File::create(path).expect("create file");
            file.write_all(content.as_bytes()).expect("write file");
        }
    }
}
