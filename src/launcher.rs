use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::process::hidden_command;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const STEAM_APP_ID: &str = "2868840";
const STEAM_APP_ID_FILE: &str = "steam_appid.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchStatus {
    pub game_exe: Option<PathBuf>,
    pub steam_exe: Option<PathBuf>,
    pub ready: bool,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchReport {
    pub target: String,
    pub vanilla_mode: bool,
    pub process_id: u32,
    pub save_backups_created: usize,
    pub seeded_modded_profiles: usize,
    pub save_backup_warning: Option<String>,
}

pub fn status(config: &AppConfig) -> LaunchStatus {
    let game_exe = resolve_game_exe(config);
    let running = is_game_running(config, game_exe.as_deref());
    let steam_exe = if game_exe.is_none() {
        resolve_steam_exe()
    } else {
        None
    };
    LaunchStatus {
        ready: game_exe.is_some() || steam_exe.is_some(),
        game_exe,
        steam_exe,
        running,
    }
}

pub fn wait_for_game_exit(config: &AppConfig) {
    let deadline = Instant::now() + Duration::from_secs(600);
    while Instant::now() < deadline {
        if status(config).running {
            break;
        }
        thread::sleep(Duration::from_secs(2));
    }

    while status(config).running {
        thread::sleep(Duration::from_secs(5));
    }
}

pub fn launch(config: &AppConfig, vanilla_mode: bool) -> AppResult<LaunchReport> {
    if let Some(exe) = resolve_game_exe(config) {
        let working_dir = exe.parent().unwrap_or(config.game_dir.as_path());
        ensure_steam_app_id_file(working_dir)?;
        let child = Command::new(&exe)
            .current_dir(working_dir)
            .spawn()
            .map_err(|error| AppError::io(&exe, error))?;

        return Ok(LaunchReport {
            target: exe.display().to_string(),
            vanilla_mode,
            process_id: child.id(),
            save_backups_created: 0,
            seeded_modded_profiles: 0,
            save_backup_warning: None,
        });
    }

    let steam_exe = resolve_steam_exe().ok_or_else(|| {
        AppError::InvalidCommand(
            "game executable not found. Set STS2_GAME_EXE, choose the executable in Settings, or install/open Steam."
                .to_string(),
        )
    })?;
    let steam_uri = format!("steam://rungameid/{STEAM_APP_ID}");
    let child = Command::new(&steam_exe)
        .arg(&steam_uri)
        .spawn()
        .map_err(|error| AppError::io(&steam_exe, error))?;

    Ok(LaunchReport {
        target: steam_uri,
        vanilla_mode,
        process_id: child.id(),
        save_backups_created: 0,
        seeded_modded_profiles: 0,
        save_backup_warning: None,
    })
}

fn ensure_steam_app_id_file(working_dir: &Path) -> AppResult<()> {
    let app_id_path = working_dir.join(STEAM_APP_ID_FILE);
    if let Ok(existing) = fs::read_to_string(&app_id_path) {
        if existing.trim() == STEAM_APP_ID {
            return Ok(());
        }
    }

    fs::write(&app_id_path, format!("{STEAM_APP_ID}\n"))
        .map_err(|source| AppError::io(&app_id_path, source))
}

pub fn resolve_game_exe(config: &AppConfig) -> Option<PathBuf> {
    if let Some(path) = &config.game_exe_path {
        if path.exists() {
            return Some(path.clone());
        }
    }

    find_known_exe(&config.game_dir).or_else(|| find_steam_game_exe())
}

fn find_known_exe(game_dir: &Path) -> Option<PathBuf> {
    let names = [
        "SlayTheSpire2.exe",
        "Slay the Spire 2.exe",
        "SlayTheSpireII.exe",
        "Slay the Spire II.exe",
        "sts2.exe",
        "StS2.exe",
    ];

    find_known_exe_in_dir(game_dir, &names)
        .or_else(|| find_known_exe_recursive(game_dir, &names, 4))
}

fn is_game_running(config: &AppConfig, game_exe: Option<&Path>) -> bool {
    let mut names = Vec::new();
    if let Some(name) = game_exe
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
    {
        names.push(name.to_string());
    }
    if let Some(name) = config
        .game_exe_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
    {
        names.push(name.to_string());
    }
    names.extend(
        [
            "SlayTheSpire2.exe",
            "Slay the Spire 2.exe",
            "SlayTheSpireII.exe",
            "Slay the Spire II.exe",
            "sts2.exe",
            "StS2.exe",
        ]
        .into_iter()
        .map(str::to_string),
    );
    names = dedupe_case_insensitive(names);
    is_process_running(&names)
}

#[cfg(target_os = "windows")]
fn is_process_running(names: &[String]) -> bool {
    let output = hidden_command("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output();
    let Ok(output) = output else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().any(|line| {
        let image_name = line
            .trim()
            .trim_start_matches('"')
            .split("\",")
            .next()
            .unwrap_or_default()
            .trim_matches('"');
        names
            .iter()
            .any(|name| image_name.eq_ignore_ascii_case(name.as_str()))
    })
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(_names: &[String]) -> bool {
    false
}

fn find_known_exe_in_dir(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn find_known_exe_recursive(root: &Path, names: &[&str], max_depth: usize) -> Option<PathBuf> {
    fn visit(dir: &Path, names: &[&str], depth: usize, max_depth: usize) -> Option<PathBuf> {
        if depth > max_depth || should_skip_dir(dir) {
            return None;
        }
        if let Some(path) = find_known_exe_in_dir(dir, names) {
            return Some(path);
        }
        let entries = fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = visit(&path, names, depth + 1, max_depth) {
                    return Some(found);
                }
            }
        }
        None
    }

    visit(root, names, 0, max_depth)
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git"
            | ".venv"
            | "modmanager"
            | "mods"
            | "target"
            | "translation"
            | "translation_work"
            | "utils"
    )
}

fn find_steam_game_exe() -> Option<PathBuf> {
    let steam_dir = steam_install_dir()?;
    for library in steam_library_dirs(&steam_dir) {
        let manifest = library
            .join("steamapps")
            .join(format!("appmanifest_{STEAM_APP_ID}.acf"));
        if !manifest.exists() {
            continue;
        }
        let manifest_text = fs::read_to_string(&manifest).ok()?;
        let install_dir = acf_value(&manifest_text, "installdir")?;
        let game_dir = library.join("steamapps").join("common").join(install_dir);
        if let Some(exe) = find_known_exe(&game_dir) {
            return Some(exe);
        }
    }
    None
}

fn resolve_steam_exe() -> Option<PathBuf> {
    let path = steam_install_dir()?.join("steam.exe");
    path.is_file().then_some(path)
}

fn steam_install_dir() -> Option<PathBuf> {
    let candidates = [
        env::var_os("STEAM_DIR").map(PathBuf::from),
        env::var_os("ProgramFiles(x86)").map(|path| PathBuf::from(path).join("Steam")),
        env::var_os("ProgramFiles").map(|path| PathBuf::from(path).join("Steam")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.join("steam.exe").is_file())
}

fn steam_library_dirs(steam_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![steam_dir.to_path_buf()];
    let library_file = steam_dir.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(library_file) {
        for line in content.lines() {
            if let Some(path) = acf_value(line, "path") {
                dirs.push(PathBuf::from(path.replace("\\\\", "\\")));
            }
        }
    }
    dedupe(dirs)
}

fn acf_value(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&needle) {
            continue;
        }
        let rest = trimmed[needle.len()..].trim();
        let start = rest.find('"')?;
        let end = rest[start + 1..].find('"')?;
        return Some(rest[start + 1..start + 1 + end].to_string());
    }
    None
}

fn dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut output = Vec::new();
    for path in paths {
        if !output.iter().any(|existing| existing == &path) {
            output.push(path);
        }
    }
    output
}

fn dedupe_case_insensitive(values: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !output
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&value))
        {
            output.push(value);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_known_exe_from_game_dir() {
        let root = test_dir("resolves_known_exe_from_game_dir");
        let exe = root.join("SlayTheSpire2.exe");
        fs::write(&exe, "fake exe").expect("write fake exe");
        let config = AppConfig {
            workspace_dir: root.join("modmanager"),
            game_dir: root.clone(),
            game_mods_dir: root.join("mods"),
            game_exe_path: None,
            save_dir: Some(root.join("saves")),
            save_backup_dir: root.join("modmanager/backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            vault_dir: root.join("modmanager/vault"),
            presets_dir: root.join("modmanager/presets"),
            translation_work_dir: root.join("modmanager/translation_work"),
            logs_dir: root.join("modmanager/logs"),
            state_dir: root.join("modmanager/state"),
            mod_index_path: root.join("modmanager/state/mod_index.tsv"),
            vendor_dir: root.join("modmanager/vendor"),
            external_manager_dirs: Vec::new(),
        };

        assert_eq!(resolve_game_exe(&config), Some(exe));
    }

    #[test]
    fn resolves_nested_known_exe_from_game_dir() {
        let root = test_dir("resolves_nested_known_exe_from_game_dir");
        let exe = root.join("SlayTheSpire2").join("Slay the Spire 2.exe");
        fs::create_dir_all(exe.parent().expect("parent")).expect("create nested dir");
        fs::write(&exe, "fake exe").expect("write fake exe");
        let config = AppConfig {
            workspace_dir: root.join("modmanager"),
            game_dir: root.clone(),
            game_mods_dir: root.join("mods"),
            game_exe_path: None,
            save_dir: Some(root.join("saves")),
            save_backup_dir: root.join("modmanager/backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            vault_dir: root.join("modmanager/vault"),
            presets_dir: root.join("modmanager/presets"),
            translation_work_dir: root.join("modmanager/translation_work"),
            logs_dir: root.join("modmanager/logs"),
            state_dir: root.join("modmanager/state"),
            mod_index_path: root.join("modmanager/state/mod_index.tsv"),
            vendor_dir: root.join("modmanager/vendor"),
            external_manager_dirs: Vec::new(),
        };

        assert_eq!(resolve_game_exe(&config), Some(exe));
    }

    #[test]
    fn writes_steam_app_id_file_for_direct_launch() {
        let root = test_dir("writes_steam_app_id_file_for_direct_launch");
        ensure_steam_app_id_file(&root).expect("write steam app id");

        assert_eq!(
            fs::read_to_string(root.join(STEAM_APP_ID_FILE)).expect("read steam app id"),
            format!("{STEAM_APP_ID}\n")
        );
    }

    #[test]
    fn leaves_matching_steam_app_id_file_unchanged() {
        let root = test_dir("leaves_matching_steam_app_id_file_unchanged");
        let path = root.join(STEAM_APP_ID_FILE);
        fs::write(&path, format!("  {STEAM_APP_ID}\r\n")).expect("seed steam app id");

        ensure_steam_app_id_file(&root).expect("check steam app id");

        assert_eq!(
            fs::read_to_string(path).expect("read steam app id"),
            format!("  {STEAM_APP_ID}\r\n")
        );
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
