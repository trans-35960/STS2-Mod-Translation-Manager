use crate::config::AppConfig;
use crate::discovery::{existing_paths, scan_mod_directory};
use crate::domain::{ModSource, ScanReport, ScanSummary};
use crate::error::{AppError, AppResult};
use crate::launcher::{self, LaunchReport, LaunchStatus};
use crate::preset::{self, Preset, PresetApplyReport, PresetExportReport, PresetImportReport};
use crate::save_backup;
use crate::state::{
    desired_active_mod_keys, detect_changes, write_desired_active_mod_keys, write_state,
};
use crate::translation::{
    self, TranslationExtractReport, TranslationMergeReport, TranslationWorkspace,
};
use crate::vault::{self, DisabledModAction, DisabledModEntry};
use crate::vendor_tools::{self, VendorTool};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub struct App {
    config: AppConfig,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn ensure_workspace_dirs(&self) -> AppResult<()> {
        for path in self.config.managed_dirs() {
            fs::create_dir_all(path).map_err(|source| AppError::io(path, source))?;
        }
        Ok(())
    }

    pub fn scan(&self) -> AppResult<ScanSummary> {
        let game_mods = scan_mod_directory(&self.config.game_mods_dir, ModSource::GameMods)?;
        let disabled_mods = vault::list_disabled_game_mods(&self.config.game_mods_dir)?;
        let mut external_manager_mods = Vec::new();

        for path in existing_paths(&self.config.external_manager_dirs) {
            external_manager_mods.extend(scan_mod_directory(&path, ModSource::ExternalManager)?);
        }

        Ok(ScanSummary {
            game_mods,
            disabled_mods,
            external_manager_mods,
        })
    }

    pub fn scan_and_update_state(&self) -> AppResult<ScanReport> {
        let summary = self.scan()?;
        let changes = detect_changes(&summary, &self.config.mod_index_path)?;
        write_state(&summary, &self.config.mod_index_path)?;
        Ok(ScanReport { summary, changes })
    }

    pub fn scan_preview_report(&self) -> AppResult<ScanReport> {
        let summary = self.scan()?;
        let changes = detect_changes(&summary, &self.config.mod_index_path)?;
        Ok(ScanReport { summary, changes })
    }

    pub fn import_mod(&self, path: &Path) -> AppResult<DisabledModAction> {
        self.ensure_workspace_dirs()?;
        vault::import_mod_to_disabled(path, &self.config.game_mods_dir)
    }

    pub fn import_mod_as_new(&self, path: &Path) -> AppResult<DisabledModAction> {
        self.ensure_workspace_dirs()?;
        vault::import_mod_to_disabled_as_new(path, &self.config.game_mods_dir)
    }

    pub fn list_vault(&self) -> AppResult<Vec<DisabledModEntry>> {
        vault::list_disabled_game_entries(&self.config.game_mods_dir)
    }

    pub fn enable_mod(&self, key: &str) -> AppResult<DisabledModAction> {
        self.ensure_workspace_dirs()?;
        vault::enable_mod(key, &self.config.game_mods_dir, &self.config.vendor_dir)
    }

    pub fn disable_mod(&self, key: &str) -> AppResult<DisabledModAction> {
        self.ensure_workspace_dirs()?;
        vault::disable_mod(key, &self.config.game_mods_dir)
    }

    pub fn set_mod_desired_active(&self, key: &str, active: bool) -> AppResult<()> {
        self.set_mods_desired_active(&[(key.to_string(), active)])
    }

    pub fn set_mods_desired_active(&self, changes: &[(String, bool)]) -> AppResult<()> {
        self.ensure_workspace_dirs()?;
        let summary = self.scan()?;
        let mut desired = desired_active_mod_keys(&summary, &self.config.state_dir)?;
        for (key, active) in changes {
            if *active {
                desired.insert(key.clone());
            } else {
                desired.remove(key);
            }
        }
        write_desired_active_mod_keys(&desired, &self.config.state_dir)
    }

    pub fn disable_all_mods(&self) -> AppResult<Vec<DisabledModAction>> {
        self.ensure_workspace_dirs()?;
        vault::disable_all(&self.config.game_mods_dir)
    }

    pub fn save_preset(&self, name: &str) -> AppResult<Preset> {
        self.ensure_workspace_dirs()?;
        preset::save_from_enabled(name, &self.config.presets_dir, &self.config.game_mods_dir)
    }

    pub fn list_presets(&self) -> AppResult<Vec<Preset>> {
        preset::list_presets(&self.config.presets_dir)
    }

    pub fn load_preset(&self, name: &str) -> AppResult<Preset> {
        preset::load_preset(name, &self.config.presets_dir)
    }

    pub fn apply_preset(&self, name: &str) -> AppResult<PresetApplyReport> {
        self.ensure_workspace_dirs()?;
        preset::apply_preset(
            name,
            &self.config.presets_dir,
            &self.config.game_mods_dir,
            &self.config.vendor_dir,
        )
    }

    pub fn export_preset(&self, name: &str, archive_path: &Path) -> AppResult<PresetExportReport> {
        self.ensure_workspace_dirs()?;
        preset::export_preset_archive(
            name,
            &self.config.presets_dir,
            &self.config.game_mods_dir,
            archive_path,
        )
    }

    pub fn import_preset_archive(&self, archive_path: &Path) -> AppResult<PresetImportReport> {
        self.ensure_workspace_dirs()?;
        preset::import_preset_archive(
            archive_path,
            &self.config.presets_dir,
            &self.config.game_mods_dir,
        )
    }

    pub fn extract_translation(&self, source: &Path) -> AppResult<TranslationExtractReport> {
        self.ensure_workspace_dirs()?;
        translation::extract_translation_work(
            source,
            &self.config.translation_work_dir,
            &self.config.vendor_dir,
        )
    }

    pub fn list_translation_workspaces(&self) -> AppResult<Vec<TranslationWorkspace>> {
        translation::list_translation_workspaces(&self.config.translation_work_dir)
    }

    pub fn merge_translation(
        &self,
        workspace_dir: &Path,
        target_root: &Path,
    ) -> AppResult<TranslationMergeReport> {
        translation::merge_translation_workspace(workspace_dir, target_root)
    }

    pub fn launch_status(&self) -> LaunchStatus {
        launcher::status(&self.config)
    }

    pub fn launch_current(&self) -> AppResult<LaunchReport> {
        self.ensure_workspace_dirs()?;
        self.ensure_launch_settings_ready()?;
        self.ensure_game_not_running()?;
        self.sync_desired_mods_for_launch()?;
        let save_backup = save_backup::backup_before_launch(&self.config, true)?;
        let bridged_current_runs =
            save_backup::bridge_modded_current_runs_for_modded_launch(&self.config)?;
        let mut report = launcher::launch(&self.config, false)?;
        report.save_backups_created = save_backup.created.len();
        report.seeded_modded_profiles = save_backup.seeded_modded_profiles;
        report.save_backup_warning =
            launch_modded_save_warning(save_backup.skipped_reason, bridged_current_runs.len());
        Ok(report)
    }

    pub fn launch_vanilla(&self) -> AppResult<LaunchReport> {
        self.ensure_workspace_dirs()?;
        self.ensure_launch_settings_ready()?;
        self.ensure_game_not_running()?;
        let save_backup = save_backup::backup_before_launch(&self.config, false)?;
        let quarantined_current_runs =
            save_backup::quarantine_modded_current_runs_for_vanilla(&self.config)?;
        vault::disable_all(&self.config.game_mods_dir)?;
        let mut report = launcher::launch(&self.config, true)?;
        report.save_backups_created = save_backup.created.len();
        report.seeded_modded_profiles = save_backup.seeded_modded_profiles;
        report.save_backup_warning =
            launch_vanilla_save_warning(save_backup.skipped_reason, quarantined_current_runs.len());
        Ok(report)
    }

    pub fn vendor_tools(&self) -> Vec<VendorTool> {
        vendor_tools::inspect(&self.config.vendor_dir)
    }

    fn ensure_launch_settings_ready(&self) -> AppResult<()> {
        let launch = launcher::status(&self.config);
        if !launch.ready {
            return Err(AppError::InvalidCommand(
                "게임 실행 경로를 자동 탐지하지 못했습니다. 설정에서 게임 실행 파일을 지정해 주세요."
                    .to_string(),
            ));
        }

        let Some(save_dir) = self.config.save_dir.as_deref() else {
            return Err(AppError::InvalidCommand(
                "세이브 폴더를 자동 탐지하지 못했습니다. 설정에서 세이브 폴더를 지정해 주세요."
                    .to_string(),
            ));
        };
        if !save_dir.is_dir() {
            return Err(AppError::InvalidCommand(format!(
                "세이브 폴더를 찾을 수 없습니다. 설정에서 올바른 폴더를 지정해 주세요: {}",
                save_dir.display()
            )));
        }
        if self.config.save_backup_dir.exists() && !self.config.save_backup_dir.is_dir() {
            return Err(AppError::InvalidCommand(format!(
                "세이브 백업 경로가 폴더가 아닙니다. 설정에서 올바른 폴더를 지정해 주세요: {}",
                self.config.save_backup_dir.display()
            )));
        }

        Ok(())
    }

    fn ensure_game_not_running(&self) -> AppResult<()> {
        if self.launch_status().running {
            return Err(AppError::InvalidCommand(
                "게임이 이미 실행 중입니다. 게임을 종료한 뒤 다시 실행하세요.".to_string(),
            ));
        }
        Ok(())
    }

    pub fn sync_desired_mods_for_launch(&self) -> AppResult<Vec<DisabledModAction>> {
        self.ensure_workspace_dirs()?;
        let mut summary = self.scan()?;
        let mut desired = desired_active_mod_keys(&summary, &self.config.state_dir)?;
        if prune_unavailable_desired_mod_keys(&mut desired, &summary) {
            write_desired_active_mod_keys(&desired, &self.config.state_dir)?;
        }
        let mut actions = Vec::new();

        for record in summary.game_mods.clone() {
            if !desired.contains(&record.stable_key()) {
                actions.push(vault::disable_mod(
                    &record.stable_key(),
                    &self.config.game_mods_dir,
                )?);
            }
        }

        summary = self.scan()?;
        let mut active_keys = summary
            .game_mods
            .iter()
            .map(|record| record.stable_key())
            .collect::<BTreeSet<_>>();

        for key in desired {
            if active_keys.contains(&key) {
                continue;
            }
            if !summary
                .disabled_mods
                .iter()
                .any(|record| record.stable_key() == key)
            {
                let Some(external_record) = summary
                    .external_manager_mods
                    .iter()
                    .find(|record| record.stable_key() == key)
                    .cloned()
                else {
                    return Err(AppError::InvalidCommand(format!(
                        "활성화할 모드를 찾을 수 없습니다: {key}"
                    )));
                };
                actions.push(vault::import_mod_to_disabled(
                    &external_record.path,
                    &self.config.game_mods_dir,
                )?);
            }

            let action =
                vault::enable_mod(&key, &self.config.game_mods_dir, &self.config.vendor_dir)?;
            actions.push(action);
            active_keys.insert(key);
            summary = self.scan()?;
        }

        actions.extend(vault::normalize_active_archives(
            &self.config.game_mods_dir,
            &self.config.vendor_dir,
        )?);
        Ok(actions)
    }
}

fn prune_unavailable_desired_mod_keys(
    desired: &mut BTreeSet<String>,
    summary: &ScanSummary,
) -> bool {
    let available = available_mod_keys(summary);
    let before = desired.len();
    desired.retain(|key| available.contains(key));
    desired.len() != before
}

fn available_mod_keys(summary: &ScanSummary) -> BTreeSet<String> {
    summary
        .game_mods
        .iter()
        .chain(summary.disabled_mods.iter())
        .chain(summary.external_manager_mods.iter())
        .map(|record| record.stable_key())
        .collect()
}

fn launch_vanilla_save_warning(
    backup_warning: Option<String>,
    quarantined_current_runs: usize,
) -> Option<String> {
    let quarantine_warning = (quarantined_current_runs > 0).then(|| {
        format!(
            "모드 진행 중 세이브 {quarantined_current_runs}개를 바닐라 실행 전에 백업으로 이동했습니다."
        )
    });
    match (backup_warning, quarantine_warning) {
        (Some(left), Some(right)) => Some(format!("{left} {right}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

fn launch_modded_save_warning(
    backup_warning: Option<String>,
    bridged_current_runs: usize,
) -> Option<String> {
    let bridge_warning = (bridged_current_runs > 0).then(|| {
        format!(
            "모드 진행 중 세이브 {bridged_current_runs}개를 모드 실행 전에 현재 런 위치로 복사했습니다."
        )
    });
    match (backup_warning, bridge_warning) {
        (Some(left), Some(right)) => Some(format!("{left} {right}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn empty_workspace_is_vanilla_safe() {
        let workspace = test_workspace("empty_workspace_is_vanilla_safe");
        let config = AppConfig {
            workspace_dir: workspace.clone(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(workspace.join("saves")),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state").join("mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: Vec::new(),
        };
        let app = App::new(config);

        app.ensure_workspace_dirs().expect("ensure dirs");
        let scan = app.scan().expect("scan");

        assert!(scan.is_vanilla_safe());
        assert_eq!(scan.total_mods(), 0);
    }

    #[test]
    fn disabled_game_mods_are_not_active_but_remain_available() {
        let workspace = test_workspace("disabled_game_mods_are_not_active_but_remain_available");
        let config = AppConfig {
            workspace_dir: workspace.clone(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(workspace.join("saves")),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state").join("mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: Vec::new(),
        };
        fs::create_dir_all(config.game_dir.join("mods.disabled")).expect("create disabled");
        fs::write(
            config.game_dir.join("mods.disabled").join("Example-v1.zip"),
            "mod",
        )
        .expect("write disabled mod");
        let app = App::new(config);

        let scan = app.scan().expect("scan");

        assert!(scan.is_vanilla_safe());
        assert!(scan.game_mods.is_empty());
        assert_eq!(scan.disabled_mods.len(), 1);
        assert_eq!(scan.disabled_mods[0].stable_key(), "example-v1");
    }

    #[test]
    fn desired_mod_state_is_applied_only_when_syncing_for_launch() {
        let workspace = test_workspace("desired_mod_state_is_applied_only_when_syncing_for_launch");
        let config = AppConfig {
            workspace_dir: workspace.clone(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(workspace.join("saves")),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state").join("mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: Vec::new(),
        };
        fs::create_dir_all(config.game_mods_dir.as_path()).expect("create game mods");
        fs::create_dir_all(config.game_dir.join("mods.disabled")).expect("create disabled mods");
        fs::write(config.game_mods_dir.join("Alpha-v1.jar"), "alpha").expect("write active");
        fs::write(
            config.game_dir.join("mods.disabled").join("Beta-v1.jar"),
            "beta",
        )
        .expect("write disabled");
        let app = App::new(config.clone());

        app.set_mod_desired_active("beta-v1", true)
            .expect("select beta");
        app.set_mod_desired_active("alpha-v1", false)
            .expect("deselect alpha");

        assert!(config.game_mods_dir.join("Alpha-v1.jar").exists());
        assert!(!config.game_mods_dir.join("Beta-v1.jar").exists());

        app.sync_desired_mods_for_launch().expect("sync desired");

        assert!(!config.game_mods_dir.join("Alpha-v1.jar").exists());
        assert!(
            config
                .game_dir
                .join("mods.disabled")
                .join("Alpha-v1.jar")
                .exists()
        );
        assert!(config.game_mods_dir.join("Beta-v1.jar").exists());
    }

    #[test]
    fn external_desired_mod_imports_to_disabled_only_when_syncing_for_launch() {
        let workspace =
            test_workspace("external_desired_mod_imports_to_disabled_only_when_syncing_for_launch");
        let external_dir = workspace.join("external");
        let config = AppConfig {
            workspace_dir: workspace.clone(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(workspace.join("saves")),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state").join("mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: vec![external_dir.clone()],
        };
        fs::create_dir_all(&external_dir).expect("create external");
        fs::write(external_dir.join("External-v1.jar"), "external").expect("write external mod");
        let app = App::new(config.clone());

        app.set_mod_desired_active("external-v1", true)
            .expect("select external");

        assert!(!config.game_mods_dir.join("External-v1.jar").exists());

        app.sync_desired_mods_for_launch().expect("sync desired");

        assert!(config.game_mods_dir.join("External-v1.jar").exists());
    }

    #[test]
    fn unavailable_desired_mods_are_pruned_when_syncing_for_launch() {
        let workspace =
            test_workspace("unavailable_desired_mods_are_pruned_when_syncing_for_launch");
        let config = AppConfig {
            workspace_dir: workspace.clone(),
            game_dir: workspace.join("game"),
            game_mods_dir: workspace.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(workspace.join("saves")),
            save_backup_dir: workspace.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: workspace.join("presets"),
            translation_work_dir: workspace.join("translation_work"),
            logs_dir: workspace.join("logs"),
            state_dir: workspace.join("state"),
            mod_index_path: workspace.join("state").join("mod_index.tsv"),
            vendor_dir: workspace.join("vendor"),
            external_manager_dirs: Vec::new(),
        };
        fs::create_dir_all(config.game_dir.join("mods.disabled")).expect("create disabled mods");
        fs::write(
            config.game_dir.join("mods.disabled").join("Beta-v1.jar"),
            "beta",
        )
        .expect("write disabled");
        let app = App::new(config.clone());

        app.set_mod_desired_active("missing-v1", true)
            .expect("select missing");
        app.set_mod_desired_active("beta-v1", true)
            .expect("select beta");

        app.sync_desired_mods_for_launch().expect("sync desired");

        let desired =
            crate::state::desired_active_mod_keys(&app.scan().expect("scan"), &config.state_dir)
                .expect("read desired");
        assert!(!desired.contains("missing-v1"));
        assert!(desired.contains("beta-v1"));
        assert!(config.game_mods_dir.join("Beta-v1.jar").exists());
    }

    fn test_workspace(name: &str) -> PathBuf {
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
        path
    }
}
