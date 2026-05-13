struct LanguagePreviewCache {
    entries: BTreeMap<String, Vec<LanguagePreviewDto>>,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct DeletedModEntry {
    id: String,
    key: String,
    name: String,
    original_path: PathBuf,
    backup_path: PathBuf,
    deleted_epoch: u64,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct PckPatchReport {
    language_output_path: PathBuf,
    packed_pck_path: Option<PathBuf>,
    installed_mod_path: Option<PathBuf>,
    applied_entries: usize,
}

#[derive(Debug, Clone)]
struct TranslationApplyRecord {
    mod_key: String,
    target_language: String,
    applied_epoch: u64,
    applied_entries: usize,
    output_path: PathBuf,
    installed_mod_path: Option<PathBuf>,
    packed_pck_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ModLifecycleSummary {
    registered_epoch: u64,
    updated_epoch: u64,
}

fn app() -> App {
    App::new(configured_config())
}

fn configured_config() -> AppConfig {
    let mut config = AppConfig::from_workspace(resolve_workspace_dir());
    if let Ok(settings) = read_ui_settings(&config) {
        if !settings.translation_work_dir.trim().is_empty() {
            config.translation_work_dir = PathBuf::from(settings.translation_work_dir);
        }
        if !settings.game_exe_path.trim().is_empty() {
            config.game_exe_path = Some(PathBuf::from(settings.game_exe_path));
        }
        if !settings.save_dir.trim().is_empty() {
            config.save_dir = Some(PathBuf::from(settings.save_dir));
        }
        if !settings.save_backup_dir.trim().is_empty() {
            config.save_backup_dir = PathBuf::from(settings.save_backup_dir);
        }
        config.save_backup_retention_days =
            sanitize_save_backup_retention_days(settings.save_backup_retention_days);
        config.save_backup_max_entries =
            sanitize_save_backup_max_entries(settings.save_backup_max_entries) as usize;
    }
    config
}

fn resolve_workspace_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("STS2_MOD_MANAGER_WORKSPACE") {
        return PathBuf::from(path);
    }

    if let Ok(exe_path) = env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
        && exe_dir.join(".sts2-mod-manager-portable").is_file()
    {
        return exe_dir.to_path_buf();
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn save_backup_dto(entry: SaveBackupEntry) -> SaveBackupDto {
    SaveBackupDto {
        id: entry.id,
        kind: entry.kind.as_str().to_string(),
        kind_label: entry.kind.label().to_string(),
        created_epoch: entry.created_epoch,
        path: display_path(&entry.path),
        bytes: entry.bytes,
    }
}

fn is_replaceable_mod_path(path: &Path, config: &AppConfig) -> bool {
    replaceable_mod_roots(config)
        .iter()
        .any(|root| path.starts_with(root))
}

fn managed_mod_roots(config: &AppConfig) -> Vec<PathBuf> {
    let mut roots = vec![
        config.game_mods_dir.clone(),
        game_disabled_dir(&config.game_mods_dir),
    ];
    push_unique_path(&mut roots, config.workspace_dir.join("mods.disabled"));
    roots
}

fn replaceable_mod_roots(config: &AppConfig) -> Vec<PathBuf> {
    let mut roots = managed_mod_roots(config);
    roots.extend(config.external_manager_dirs.iter().cloned());
    roots
}

fn deletable_mod_roots(config: &AppConfig) -> Vec<PathBuf> {
    replaceable_mod_roots(config)
}

fn ensure_existing_replaceable_mod_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    ensure_existing_path_in_roots(path, &replaceable_mod_roots(config), "교체할 모드 경로")
}

fn ensure_existing_deletable_mod_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    ensure_existing_path_in_roots(path, &deletable_mod_roots(config), "삭제할 모드 경로")
}

fn ensure_deletable_mod_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    ensure_path_in_roots(path, &deletable_mod_roots(config), "모드 경로")
}

fn ensure_state_path(path: &Path, config: &AppConfig, description: &str) -> Result<(), String> {
    ensure_path_in_roots(path, std::slice::from_ref(&config.state_dir), description)
}

fn ensure_existing_state_path(
    path: &Path,
    config: &AppConfig,
    description: &str,
) -> Result<(), String> {
    ensure_existing_path_in_roots(path, std::slice::from_ref(&config.state_dir), description)
}

fn ensure_translation_work_path(
    path: &Path,
    config: &AppConfig,
    description: &str,
) -> Result<(), String> {
    ensure_path_in_roots(
        path,
        std::slice::from_ref(&config.translation_work_dir),
        description,
    )
}

fn game_disabled_dir(game_mods_dir: &Path) -> PathBuf {
    game_mods_dir
        .parent()
        .map(|parent| parent.join("mods.disabled"))
        .unwrap_or_else(|| game_mods_dir.with_file_name("mods.disabled"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn connected_mod_keys(summary: &ScanSummary) -> BTreeSet<String> {
    summary
        .game_mods
        .iter()
        .filter(|record| !is_game_disabled_record(record))
        .chain(
            summary
                .disabled_mods
                .iter()
                .filter(|record| !is_game_disabled_record(record)),
        )
        .chain(summary.external_manager_mods.iter())
        .map(ModRecord::stable_key)
        .collect()
}


fn timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}


fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn default_sts2_game_log_path() -> Option<PathBuf> {
    roaming_dir().map(|dir| dir.join("SlayTheSpire2").join("logs").join("godot.log"))
}

fn roaming_dir() -> Option<PathBuf> {
    env::var_os("APPDATA").map(PathBuf::from)
}

fn kind_label(kind: ModKind) -> &'static str {
    match kind {
        ModKind::Directory => "folder",
        ModKind::Archive => "archive",
        ModKind::Package => "package",
        ModKind::UnknownFile => "file",
    }
}


fn epoch_seconds(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

