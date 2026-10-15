//! Whether installed mods fit the installed game, checked before launch.
//!
//! Two checks: the manifest's `min_game_version` against `release_info.json`,
//! and the references each mod DLL compiles in against `sts2.dll` and the
//! library mods it depends on ([`crate::dll_refs`]). Reading `sts2.dll` takes
//! a moment, so DLL results are cached per DLL build (size and modified time)
//! and per game build (`main_assembly_hash`).

use crate::dll_refs::{self, Metadata, MissingReference, TargetAssembly};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::UNIX_EPOCH;

/// Bump when the check changes what it reports, so cached results are redone.
const CACHE_RULES: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameRelease {
    pub version: Option<String>,
    /// Changes with every game build, unlike the version on beta branches.
    pub main_assembly_hash: Option<String>,
}

/// `release_info.json` of the install at `game_dir`.
pub fn read_game_release(game_dir: &Path) -> GameRelease {
    let Some(value) = fs::read_to_string(game_dir.join("release_info.json"))
        .ok()
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()
        })
    else {
        return GameRelease::default();
    };
    let text = |key: &str| match value.get(key)? {
        serde_json::Value::String(text) => Some(text.trim().to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    };
    GameRelease {
        version: text("version"),
        main_assembly_hash: text("main_assembly_hash"),
    }
}

pub fn game_assembly_path(game_dir: &Path) -> PathBuf {
    game_dir
        .join("data_sts2_windows_x86_64")
        .join(format!("{}.dll", dll_refs::GAME_ASSEMBLY))
}

/// Compares dotted versions numerically (`v0.111.0` > `0.107.1`); `None`
/// when either has no number to compare.
pub fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = version_numbers(left)?;
    let right = version_numbers(right)?;
    let len = left.len().max(right.len());
    let at = |numbers: &[u64], index: usize| numbers.get(index).copied().unwrap_or(0);
    Some(
        (0..len)
            .map(|index| at(&left, index).cmp(&at(&right, index)))
            .find(|order| order.is_ne())
            .unwrap_or(Ordering::Equal),
    )
}

/// `current >= minimum`, `None` when either is not a version.
pub fn version_at_least(current: &str, minimum: &str) -> Option<bool> {
    compare_versions(current, minimum).map(Ordering::is_ge)
}

/// Leading `1.2.3` numbers; anything after (`-beta`, `+build`) is ignored.
fn version_numbers(version: &str) -> Option<Vec<u64>> {
    let version = version.trim().trim_start_matches(['v', 'V']);
    let core = version
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .next()
        .unwrap_or_default();
    let numbers = core
        .split('.')
        .map_while(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    (!numbers.is_empty()).then_some(numbers)
}

/// DLLs next to the manifest of the mod at `mod_dir`, the ones the game
/// loads. Libraries in subfolders (`lib/`, version variants) are left out.
pub fn mod_dll_paths(mod_dir: &Path) -> Vec<PathBuf> {
    let manifest_dir = crate::discovery::find_mod_manifests(mod_dir, 2)
        .into_iter()
        .next()
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| mod_dir.to_path_buf());
    let Ok(entries) = fs::read_dir(&manifest_dir) else {
        return Vec::new();
    };
    let mut dlls = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
        })
        .collect::<Vec<_>>();
    dlls.sort();
    dlls
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    #[serde(default)]
    rules: u32,
    #[serde(default)]
    entries: BTreeMap<String, CacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    modified_ms: u64,
    /// Which builds of the game and dependency DLLs this was checked against.
    targets: String,
    missing: Vec<MissingReference>,
}

/// Checks mod DLLs against the game, reading each assembly at most once and
/// only when a result is not cached.
pub struct ReferenceChecker {
    game_dll: PathBuf,
    game_fingerprint: Option<String>,
    cache_path: PathBuf,
    cache: CacheFile,
    checked: BTreeSet<String>,
    dirty: bool,
    targets: HashMap<PathBuf, Option<Rc<TargetAssembly>>>,
}

impl ReferenceChecker {
    pub fn new(game_dir: &Path, state_dir: &Path) -> Self {
        let game_dll = game_assembly_path(game_dir);
        let game_fingerprint = file_stamp(&game_dll).map(|(size, modified_ms)| {
            match read_game_release(game_dir).main_assembly_hash {
                Some(hash) => format!("sts2#{hash}:{size}"),
                None => format!("sts2:{size}:{modified_ms}"),
            }
        });
        let cache_path = state_dir.join("mod_compat_cache.json");
        let cache = fs::read_to_string(&cache_path)
            .ok()
            .and_then(|text| serde_json::from_str::<CacheFile>(&text).ok())
            .filter(|cache| cache.rules == CACHE_RULES)
            .unwrap_or_default();
        Self {
            game_dll,
            game_fingerprint,
            cache_path,
            cache,
            checked: BTreeSet::new(),
            dirty: false,
            targets: HashMap::new(),
        }
    }

    /// Whether the game assembly is there to check against.
    pub fn has_game(&self) -> bool {
        self.game_fingerprint.is_some()
    }

    /// What `mod_dlls` use from the game or from `dependency_dlls` that is
    /// not there. DLLs that are not .NET assemblies are skipped.
    pub fn check(
        &mut self,
        mod_dlls: &[PathBuf],
        dependency_dlls: &[PathBuf],
    ) -> Vec<MissingReference> {
        let Some(game_fingerprint) = self.game_fingerprint.clone() else {
            return Vec::new();
        };
        let mut dependencies = dependency_dlls
            .iter()
            .filter(|dll| !mod_dlls.contains(dll))
            .filter_map(|dll| {
                let (size, modified_ms) = file_stamp(dll)?;
                Some((
                    dll.clone(),
                    format!("{}:{size}:{modified_ms}", dll.display()),
                ))
            })
            .collect::<Vec<_>>();
        dependencies.sort();
        dependencies.dedup();
        let targets_key = std::iter::once(game_fingerprint)
            .chain(dependencies.iter().map(|(_, stamp)| stamp.clone()))
            .collect::<Vec<_>>()
            .join("|");

        let mut missing = Vec::new();
        for dll in mod_dlls {
            let Some((size, modified_ms)) = file_stamp(dll) else {
                continue;
            };
            let key = dll.display().to_string();
            self.checked.insert(key.clone());
            let cached = self.cache.entries.get(&key).filter(|entry| {
                entry.size == size
                    && entry.modified_ms == modified_ms
                    && entry.targets == targets_key
            });
            let found = match cached {
                Some(entry) => entry.missing.clone(),
                None => {
                    let found = self.check_dll(dll, &dependencies);
                    self.cache.entries.insert(
                        key,
                        CacheEntry {
                            size,
                            modified_ms,
                            targets: targets_key.clone(),
                            missing: found.clone(),
                        },
                    );
                    self.dirty = true;
                    found
                }
            };
            for item in found {
                if !missing.contains(&item) {
                    missing.push(item);
                }
            }
        }
        missing
    }

    fn check_dll(
        &mut self,
        dll: &Path,
        dependencies: &[(PathBuf, String)],
    ) -> Vec<MissingReference> {
        let Ok(references) = Metadata::read(dll).and_then(|metadata| metadata.references()) else {
            return Vec::new();
        };
        let mut targets = Vec::new();
        for path in std::iter::once(self.game_dll.clone())
            .chain(dependencies.iter().map(|(path, _)| path.clone()))
        {
            if let Some(target) = self.target(&path) {
                targets.push(target);
            }
        }
        // The game and its dependencies come first, so a mod bundling its own
        // copy of a library is still checked against the installed one.
        let targets = targets.iter().map(Rc::as_ref).collect::<Vec<_>>();
        dll_refs::find_missing(&references, &targets)
    }

    fn target(&mut self, path: &Path) -> Option<Rc<TargetAssembly>> {
        self.targets
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                Metadata::read(path)
                    .and_then(|metadata| metadata.target())
                    .ok()
                    .map(Rc::new)
            })
            .clone()
    }

    /// Writes new results and forgets DLLs no longer installed.
    pub fn save(mut self) -> std::io::Result<()> {
        let before = self.cache.entries.len();
        let checked = std::mem::take(&mut self.checked);
        self.cache.entries.retain(|key, _| checked.contains(key));
        if !self.dirty && self.cache.entries.len() == before {
            return Ok(());
        }
        self.cache.rules = CACHE_RULES;
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(&self.cache).map_err(std::io::Error::other)?;
        fs::write(&self.cache_path, text)
    }
}

fn file_stamp(path: &Path) -> Option<(u64, u64)> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    Some((metadata.len(), modified_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert_eq!(
            compare_versions("v0.111.0", "0.107.1"),
            Some(Ordering::Greater)
        );
        assert_eq!(compare_versions("0.99", "0.100.0"), Some(Ordering::Less));
        assert_eq!(compare_versions("v1.6.6", "1.6.6"), Some(Ordering::Equal));
        assert_eq!(compare_versions("3.3", "3.3.0"), Some(Ordering::Equal));
        assert_eq!(
            compare_versions("2.0.0-beta", "2.0.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(compare_versions("latest", "1.0"), None);
        assert_eq!(version_at_least("v3.4.7", "3.3.0"), Some(true));
        assert_eq!(version_at_least("v1.6.5", "v1.6.6"), Some(false));
    }

    #[test]
    fn reads_release_info() {
        let dir =
            std::env::temp_dir().join(format!("sts2-mod-compat-release-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release_info.json"),
            r#"{"version": "v0.111.0", "branch": "v0.111.0", "main_assembly_hash": 222455745}"#,
        )
        .unwrap();
        let release = read_game_release(&dir);
        fs::remove_dir_all(&dir).ok();
        assert_eq!(
            release,
            GameRelease {
                version: Some("v0.111.0".to_string()),
                main_assembly_hash: Some("222455745".to_string()),
            }
        );
    }

    /// Uses a DLL shipped with the installed game as the "mod"; skipped
    /// without the game.
    #[test]
    fn reuses_cached_results_for_the_same_builds() {
        let game_dir = std::env::var_os("STS2_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::config::AppConfig::from_workspace(".").game_dir);
        let dll = game_dir
            .join("data_sts2_windows_x86_64")
            .join("GodotSharp.dll");
        if !game_assembly_path(&game_dir).is_file() || !dll.is_file() {
            eprintln!("skipped: game not found");
            return;
        }
        let state =
            std::env::temp_dir().join(format!("sts2-mod-compat-cache-{}", std::process::id()));
        let mut first = ReferenceChecker::new(&game_dir, &state);
        assert!(first.check(std::slice::from_ref(&dll), &[]).is_empty());
        assert!(first.dirty);
        first.save().unwrap();

        let mut second = ReferenceChecker::new(&game_dir, &state);
        assert!(second.check(std::slice::from_ref(&dll), &[]).is_empty());
        assert!(!second.dirty && second.targets.is_empty());
        fs::remove_dir_all(&state).ok();
    }

    #[test]
    fn checker_without_game_reports_nothing() {
        let dir =
            std::env::temp_dir().join(format!("sts2-mod-compat-nogame-{}", std::process::id()));
        let mut checker = ReferenceChecker::new(&dir, &dir);
        assert!(!checker.has_game());
        assert!(checker.check(&[dir.join("Mod.dll")], &[]).is_empty());
    }
}
