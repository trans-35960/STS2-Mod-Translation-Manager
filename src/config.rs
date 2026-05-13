use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub workspace_dir: PathBuf,
    pub game_dir: PathBuf,
    pub game_mods_dir: PathBuf,
    pub game_exe_path: Option<PathBuf>,
    pub save_dir: Option<PathBuf>,
    pub save_backup_dir: PathBuf,
    pub save_backup_retention_days: u32,
    pub save_backup_max_entries: usize,
    pub presets_dir: PathBuf,
    pub translation_work_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub state_dir: PathBuf,
    pub mod_index_path: PathBuf,
    pub vendor_dir: PathBuf,
    pub external_manager_dirs: Vec<PathBuf>,
}

impl AppConfig {
    pub fn from_workspace(workspace_dir: impl Into<PathBuf>) -> Self {
        let workspace_dir = workspace_dir.into();
        let game_dir = env::var_os("STS2_GAME_DIR")
            .map(PathBuf::from)
            .or_else(|| default_game_dir(&workspace_dir))
            .unwrap_or_else(|| workspace_dir.clone());
        let game_mods_dir = env::var_os("STS2_GAME_MODS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| game_dir.join("mods"));
        let game_exe_path = env::var_os("STS2_GAME_EXE").map(PathBuf::from);
        let save_dir = env::var_os("STS2_SAVE_DIR")
            .map(PathBuf::from)
            .or_else(default_save_dir);
        let save_backup_dir = env::var_os("STS2_SAVE_BACKUP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_dir.join("backups"));

        Self {
            workspace_dir: workspace_dir.clone(),
            game_dir,
            game_mods_dir,
            game_exe_path,
            save_dir,
            save_backup_dir,
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace_dir.join("presets"),
            translation_work_dir: workspace_dir.join("translation_work"),
            logs_dir: workspace_dir.join("logs"),
            state_dir: workspace_dir.join("state"),
            mod_index_path: workspace_dir.join("state").join("mod_index.tsv"),
            vendor_dir: workspace_dir.join("vendor"),
            external_manager_dirs: default_external_manager_dirs(),
        }
    }

    pub fn managed_dirs(&self) -> [&Path; 6] {
        [
            self.save_backup_dir.as_path(),
            self.presets_dir.as_path(),
            self.translation_work_dir.as_path(),
            self.logs_dir.as_path(),
            self.state_dir.as_path(),
            self.vendor_dir.as_path(),
        ]
    }
}

pub fn default_save_dir() -> Option<PathBuf> {
    let app_data = env::var_os("APPDATA").map(PathBuf::from)?;
    let steam_root = app_data.join("SlayTheSpire2").join("steam");
    let preferred = steam_root.join("76561198093641030");
    if preferred.exists() {
        return Some(preferred);
    }

    if let Ok(entries) = std::fs::read_dir(&steam_root) {
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        candidates.sort();
        if let Some(path) = candidates
            .iter()
            .find(|path| looks_like_save_dir(path))
            .cloned()
        {
            return Some(path);
        }
        if let Some(path) = candidates.into_iter().next() {
            return Some(path);
        }
    }

    Some(preferred)
}

fn looks_like_save_dir(path: &Path) -> bool {
    ["profile1", "profile2", "profile3", "modded"]
        .iter()
        .any(|name| path.join(name).exists())
}

fn default_game_dir(workspace_dir: &Path) -> Option<PathBuf> {
    let workspace_parent = workspace_dir.parent().map(Path::to_path_buf);
    if let Some(parent) = workspace_parent
        .as_ref()
        .filter(|path| looks_like_game_dir(path))
    {
        return Some(parent.clone());
    }
    steam_game_dir().or(workspace_parent)
}

fn looks_like_game_dir(path: &Path) -> bool {
    known_game_exe_names()
        .iter()
        .any(|name| path.join(name).is_file())
}

fn steam_game_dir() -> Option<PathBuf> {
    let steam_dir = steam_install_dir()?;
    for library in steam_library_dirs(&steam_dir) {
        let manifest = library.join("steamapps").join("appmanifest_2868840.acf");
        if !manifest.exists() {
            continue;
        }
        let manifest_text = std::fs::read_to_string(&manifest).ok()?;
        let install_dir = acf_value(&manifest_text, "installdir")?;
        let game_dir = library.join("steamapps").join("common").join(install_dir);
        if looks_like_game_dir(&game_dir) {
            return Some(game_dir);
        }
    }
    None
}

fn steam_install_dir() -> Option<PathBuf> {
    [
        env::var_os("STEAM_DIR").map(PathBuf::from),
        env::var_os("ProgramFiles(x86)").map(|path| PathBuf::from(path).join("Steam")),
        env::var_os("ProgramFiles").map(|path| PathBuf::from(path).join("Steam")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.join("steam.exe").is_file())
}

fn steam_library_dirs(steam_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![steam_dir.to_path_buf()];
    let library_file = steam_dir.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = std::fs::read_to_string(library_file) {
        for line in content.lines() {
            if let Some(path) = acf_value(line, "path") {
                dirs.push(PathBuf::from(path.replace("\\\\", "\\")));
            }
        }
    }
    dedupe_paths(dirs)
}

fn acf_value(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        let rest = trimmed.strip_prefix(&needle)?.trim();
        let value = rest.trim_matches('"');
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn known_game_exe_names() -> [&'static str; 6] {
    [
        "SlayTheSpire2.exe",
        "Slay the Spire 2.exe",
        "SlayTheSpireII.exe",
        "Slay the Spire II.exe",
        "sts2.exe",
        "StS2.exe",
    ]
}

fn default_external_manager_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(paths) = env::var_os("STS2_EXTERNAL_MOD_DIRS") {
        dirs.extend(split_env_paths(&paths));
    }

    if let Some(path) = env::var_os("STS2_NEXUS_DOWNLOADS") {
        dirs.push(PathBuf::from(path));
    }

    if let Some(path) = env::var_os("STS2_NMM_DOWNLOADS") {
        dirs.push(PathBuf::from(path));
    }

    if let Some(app_data) = env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        push_vortex_dirs(&mut dirs, &app_data);
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        dirs.push(
            local_app_data
                .join("Nexus Mod Manager")
                .join("SlayTheSpire2")
                .join("Mods"),
        );
        dirs.push(
            local_app_data
                .join("Nexus Mod Manager")
                .join("Slay the Spire 2")
                .join("Mods"),
        );
    }

    if let Some(user_profile) = env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);
        dirs.push(
            user_profile
                .join("Documents")
                .join("Nexus Mod Manager")
                .join("SlayTheSpire2")
                .join("Mods"),
        );
        dirs.push(
            user_profile
                .join("Documents")
                .join("Nexus Mod Manager")
                .join("Slay the Spire 2")
                .join("Mods"),
        );
    }

    dedupe_paths(dirs)
}

fn push_vortex_dirs(dirs: &mut Vec<PathBuf>, app_data: &Path) {
    for game_id in vortex_game_ids() {
        dirs.push(app_data.join("Vortex").join("downloads").join(game_id));
        dirs.push(app_data.join("Vortex").join(game_id).join("mods"));
        dirs.push(app_data.join("Vortex").join("mods").join(game_id));
    }
}

fn vortex_game_ids() -> [&'static str; 3] {
    ["slaythespire2", "slay-the-spire-2", "slay the spire 2"]
}

fn split_env_paths(paths: &std::ffi::OsStr) -> Vec<PathBuf> {
    paths
        .to_string_lossy()
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_workspace_paths_under_workspace() {
        let config = AppConfig::from_workspace(PathBuf::from(r"Z:\game\sts2\modmanager"));

        assert_eq!(config.game_mods_dir, config.game_dir.join("mods"));
        assert_eq!(config.game_exe_path, None);
        assert_eq!(
            config.translation_work_dir,
            PathBuf::from(r"Z:\game\sts2\modmanager\translation_work")
        );
        assert_eq!(
            config.mod_index_path,
            PathBuf::from(r"Z:\game\sts2\modmanager\state\mod_index.tsv")
        );
    }

    #[test]
    fn vortex_defaults_cover_downloads_and_staging_layouts() {
        let mut dirs = Vec::new();
        push_vortex_dirs(&mut dirs, Path::new(r"C:\Users\player\AppData\Roaming"));

        assert!(dirs.contains(&PathBuf::from(
            r"C:\Users\player\AppData\Roaming\Vortex\downloads\slaythespire2"
        )));
        assert!(dirs.contains(&PathBuf::from(
            r"C:\Users\player\AppData\Roaming\Vortex\slaythespire2\mods"
        )));
        assert!(dirs.contains(&PathBuf::from(
            r"C:\Users\player\AppData\Roaming\Vortex\mods\slaythespire2"
        )));
    }
}
