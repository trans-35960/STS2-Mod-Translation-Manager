struct LanguagePreviewCache {
    entries: BTreeMap<String, Vec<LanguagePreviewDto>>,
    dirty: bool,
}

const APP_WORKSPACE_DIR_NAME: &str = "STS2-Mod-Translation-Manager";

static RUNTIME_APP_DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static RUNTIME_RESOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub(crate) fn configure_runtime_paths(
    app_data_dir: Option<PathBuf>,
    resource_dir: Option<PathBuf>,
) {
    if let Some(path) = app_data_dir {
        let _ = RUNTIME_APP_DATA_DIR.set(path);
    }
    if let Some(path) = resource_dir {
        let _ = RUNTIME_RESOURCE_DIR.set(path);
    }
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
    if let Some(vendor_dir) = bundled_vendor_dir() {
        config.vendor_dir = vendor_dir;
    }
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
    if let Some(path) = env_path("STS2_MOD_MANAGER_WORKSPACE") {
        return path;
    }

    if let Ok(exe_path) = env::current_exe()
        && let Some(path) = portable_workspace_dir(&exe_path)
    {
        return path;
    }

    #[cfg(debug_assertions)]
    if let Some(path) = manifest_workspace_dir() {
        return path;
    }

    runtime_app_data_workspace_dir()
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|value| !value.as_os_str().is_empty())
        .map(PathBuf::from)
}

fn portable_workspace_dir(exe_path: &Path) -> Option<PathBuf> {
    let exe_dir = exe_path.parent()?;
    exe_dir
        .join(".sts2-mod-manager-portable")
        .is_file()
        .then(|| exe_dir.to_path_buf())
}

#[cfg(debug_assertions)]
fn manifest_workspace_dir() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .filter(|path| path.join("Cargo.toml").is_file())
}

fn runtime_app_data_workspace_dir() -> PathBuf {
    RUNTIME_APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(default_user_workspace_dir)
}

fn default_user_workspace_dir() -> PathBuf {
    if let Some(path) = env_path("LOCALAPPDATA") {
        return path.join(APP_WORKSPACE_DIR_NAME);
    }
    if let Some(path) = env_path("APPDATA") {
        return path.join(APP_WORKSPACE_DIR_NAME);
    }
    if let Some(path) = env_path("XDG_DATA_HOME") {
        return path.join(APP_WORKSPACE_DIR_NAME);
    }
    if let Some(path) = env_path("HOME") {
        return path
            .join(".local")
            .join("share")
            .join(APP_WORKSPACE_DIR_NAME);
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(APP_WORKSPACE_DIR_NAME)
}

fn bundled_vendor_dir() -> Option<PathBuf> {
    let resource_dir = RUNTIME_RESOURCE_DIR.get()?;
    bundled_vendor_candidates(resource_dir)
        .into_iter()
        .find(|path| looks_like_vendor_dir(path))
}

fn bundled_vendor_candidates(resource_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    push_unique_path(&mut candidates, resource_dir.join("vendor"));
    push_unique_path(&mut candidates, resource_dir.to_path_buf());
    push_unique_path(&mut candidates, resource_dir.join("_up_").join("vendor"));
    if let Some(parent) = resource_dir.parent() {
        push_unique_path(&mut candidates, parent.join("vendor"));
        push_unique_path(&mut candidates, parent.join("_up_").join("vendor"));
    }
    candidates
}

fn looks_like_vendor_dir(path: &Path) -> bool {
    path.join("7zip").join("7z.exe").is_file()
        || path
            .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
            .join("GodotPCKExplorer.Console.exe")
            .is_file()
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

fn append_performance_log(line: impl AsRef<str>) {
    let path = resolve_workspace_dir()
        .join("state")
        .join("logs")
        .join("performance.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::metadata(&path)
        .map(|metadata| metadata.len() > 5 * 1024 * 1024)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, path.with_extension("log.old"));
    }
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(file, "{}\t{}", timestamp_string(), line.as_ref());
}

struct PerfTrace {
    name: &'static str,
    started: Instant,
    last: Instant,
    parts: Vec<(&'static str, u128)>,
}

impl PerfTrace {
    fn new(name: &'static str) -> Self {
        let now = Instant::now();
        Self {
            name,
            started: now,
            last: now,
            parts: Vec::new(),
        }
    }

    fn mark(&mut self, label: &'static str) {
        let now = Instant::now();
        self.parts
            .push((label, now.duration_since(self.last).as_millis()));
        self.last = now;
    }

    fn finish(self, detail: impl AsRef<str>, threshold_ms: u128) {
        let total = self.started.elapsed().as_millis();
        if total < threshold_ms && env::var_os("STS2_PERF_VERBOSE").is_none() {
            return;
        }
        let parts = self
            .parts
            .into_iter()
            .map(|(label, millis)| format!("{label}={millis}ms"))
            .collect::<Vec<_>>()
            .join(" ");
        append_performance_log(format!(
            "{} total={}ms detail={} {}",
            self.name,
            total,
            detail.as_ref(),
            parts
        ));
    }
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

#[cfg(test)]
mod runtime_path_tests {
    use super::*;

    #[test]
    fn portable_workspace_uses_marker_next_to_exe() {
        let root = env::temp_dir().join(format!("sts2-portable-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create portable dir");
        fs::write(root.join(".sts2-mod-manager-portable"), "").expect("write marker");

        assert_eq!(
            portable_workspace_dir(&root.join("STS2 Mod Manager.exe")),
            Some(root.clone())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_vendor_dir_accepts_resource_vendor_folder() {
        let root = env::temp_dir().join(format!("sts2-resource-{}", timestamp_string()));
        let vendor = root.join("vendor").join("7zip");
        fs::create_dir_all(&vendor).expect("create vendor dir");
        fs::write(vendor.join("7z.exe"), "").expect("write tool");

        assert!(looks_like_vendor_dir(&root.join("vendor")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_vendor_candidates_include_up_vendor_folder() {
        let root = env::temp_dir().join(format!("sts2-resource-up-{}", timestamp_string()));
        let vendor = root.join("_up_").join("vendor").join("7zip");
        fs::create_dir_all(&vendor).expect("create vendor dir");
        fs::write(vendor.join("7z.exe"), "").expect("write tool");

        let up_vendor = root.join("_up_").join("vendor");
        let candidates = bundled_vendor_candidates(&root);

        assert!(candidates.contains(&up_vendor));
        assert!(looks_like_vendor_dir(&up_vendor));

        let _ = fs::remove_dir_all(root);
    }
}

