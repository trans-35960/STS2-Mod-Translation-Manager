use crate::app::App;
use crate::config::AppConfig;
use crate::domain::{ModChange, ModRecord, ScanReport, ScanSummary};
use crate::error::{AppError, AppResult};
use crate::launcher::{LaunchReport, LaunchStatus};
use crate::preset::{Preset, PresetApplyReport, PresetExportReport, PresetImportReport};
use crate::text_ui;
use crate::translation::{
    TranslationExtractReport, TranslationMergeReport, TranslationWorkspace,
    scan_translation_candidates,
};
use crate::vault::{VaultAction, VaultEntry};
use crate::vendor_tools::VendorTool;
use std::env;
use std::path::{Path, PathBuf};

pub fn run_from_env() -> AppResult<()> {
    let current_dir = env::current_dir().map_err(|source| AppError::io(".", source))?;
    run(env::args().skip(1), resolve_workspace_dir(current_dir))
}

pub fn run(args: impl IntoIterator<Item = String>, current_dir: PathBuf) -> AppResult<()> {
    let args = args.into_iter().collect::<Vec<_>>();
    let config = AppConfig::from_workspace(current_dir.clone());
    let app = App::new(config);

    match args.first().map(String::as_str) {
        None => {
            print_help();
        }
        Some("scan") => {
            app.ensure_workspace_dirs()?;
            print_scan_report(&app.scan_and_update_state()?);
        }
        Some("init") => {
            app.ensure_workspace_dirs()?;
            println!("Workspace initialized.");
            print_paths(app.config());
        }
        Some("paths") => {
            print_paths(app.config());
        }
        Some("vault") => {
            run_vault_command(&app, &args[1..])?;
        }
        Some("preset") => {
            run_preset_command(&app, &args[1..])?;
        }
        Some("ui") => {
            text_ui::run(&app)?;
        }
        Some("launch") => {
            run_launch_command(&app, &args[1..])?;
        }
        Some("tools") => {
            run_tools_command(&app, &args[1..])?;
        }
        Some("translation-scan") => {
            let path = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| app.config().game_mods_dir.clone());
            print_translation_candidates(&path)?;
        }
        Some("translation") => {
            run_translation_command(&app, &args[1..])?;
        }
        Some("help" | "--help" | "-h") => {
            print_help();
        }
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn print_help() {
    println!("Slay the Spire 2 Mod Manager");
    println!();
    println!("Commands:");
    println!("  scan                  scan game mods, vault, and Nexus/Vortex paths");
    println!("  init                  create managed folders");
    println!("  paths                 print resolved workspace paths");
    println!("  vault list            list managed vault mods");
    println!("  vault import PATH     copy a mod into the managed vault");
    println!("  vault enable KEY      copy a vault mod into the game mods folder");
    println!("  vault disable KEY     move an enabled game mod into the vault disabled area");
    println!("  vault vanilla         disable all game mods for a clean launch");
    println!("  preset list           list saved presets");
    println!("  preset save NAME      save currently enabled mods as a preset");
    println!("  preset show NAME      show preset mod keys");
    println!("  preset apply NAME     apply a preset from the vault");
    println!("  preset export NAME ZIP export preset metadata and mod files");
    println!("  preset import ZIP     import a preset archive and register bundled mods");
    println!("  ui                    open the interactive Rust text UI");
    println!("  launch status         show game executable detection status");
    println!("  launch current        launch game with current enabled mods");
    println!("  launch vanilla        disable mods and launch clean game");
    println!("  tools status          show embedded helper tool availability");
    println!("  translation-scan PATH scan a mod or folder for language-like files");
    println!("  translation extract PATH      extract language files into translation_work");
    println!("  translation list              list translation workspaces");
    println!("  translation merge WORK TARGET merge translated files into a target folder");
    println!("  help                  show this help");
}

fn resolve_workspace_dir(current_dir: PathBuf) -> PathBuf {
    if current_dir.join("Cargo.toml").exists() {
        return current_dir;
    }

    if let Ok(exe_path) = env::current_exe() {
        for ancestor in exe_path.ancestors() {
            if ancestor.join("Cargo.toml").exists() {
                return ancestor.to_path_buf();
            }
        }
    }

    current_dir
}

fn run_vault_command(app: &App, args: &[String]) -> AppResult<()> {
    match args.first().map(String::as_str) {
        None | Some("list") => print_vault_entries(&app.list_vault()?),
        Some("import") => {
            let path = required_arg(args, 1, "vault import PATH")?;
            let action = app.import_mod(Path::new(path))?;
            print_action("Imported", &action);
        }
        Some("enable") => {
            let key = required_arg(args, 1, "vault enable KEY")?;
            let action = app.enable_mod(key)?;
            print_action("Enabled", &action);
        }
        Some("disable") => {
            let key = required_arg(args, 1, "vault disable KEY")?;
            let action = app.disable_mod(key)?;
            print_action("Disabled", &action);
        }
        Some("vanilla") => {
            let actions = app.disable_all_mods()?;
            if actions.is_empty() {
                println!("No enabled mods found. Vanilla-safe startup is ready.");
            } else {
                println!(
                    "Disabled {} mod(s). Vanilla-safe startup is ready.",
                    actions.len()
                );
                for action in actions {
                    print_action("Moved", &action);
                }
            }
        }
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown vault command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn run_preset_command(app: &App, args: &[String]) -> AppResult<()> {
    match args.first().map(String::as_str) {
        None | Some("list") => print_presets(&app.list_presets()?),
        Some("save") => {
            let name = required_arg(args, 1, "preset save NAME")?;
            let preset = app.save_preset(name)?;
            println!("Saved preset '{}'.", preset.name);
            print_preset(&preset);
        }
        Some("show") => {
            let name = required_arg(args, 1, "preset show NAME")?;
            print_preset(&app.load_preset(name)?);
        }
        Some("apply") => {
            let name = required_arg(args, 1, "preset apply NAME")?;
            print_preset_apply_report(&app.apply_preset(name)?);
        }
        Some("export") => {
            let name = required_arg(args, 1, "preset export NAME ZIP")?;
            let archive = required_arg(args, 2, "preset export NAME ZIP")?;
            print_preset_export_report(&app.export_preset(name, Path::new(archive))?);
        }
        Some("import") => {
            let archive = required_arg(args, 1, "preset import ZIP")?;
            print_preset_import_report(&app.import_preset_archive(Path::new(archive))?);
        }
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown preset command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn run_translation_command(app: &App, args: &[String]) -> AppResult<()> {
    match args.first().map(String::as_str) {
        None | Some("list") => print_translation_workspaces(&app.list_translation_workspaces()?),
        Some("extract") => {
            let source = required_arg(args, 1, "translation extract PATH")?;
            print_translation_extract_report(&app.extract_translation(Path::new(source))?);
        }
        Some("merge") => {
            let workspace = required_arg(args, 1, "translation merge WORK TARGET")?;
            let target = required_arg(args, 2, "translation merge WORK TARGET")?;
            print_translation_merge_report(
                &app.merge_translation(Path::new(workspace), Path::new(target))?,
            );
        }
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown translation command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn run_launch_command(app: &App, args: &[String]) -> AppResult<()> {
    match args.first().map(String::as_str) {
        None | Some("status") => print_launch_status(&app.launch_status()),
        Some("current") => print_launch_report(&app.launch_current()?),
        Some("vanilla") => print_launch_report(&app.launch_vanilla()?),
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown launch command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn run_tools_command(app: &App, args: &[String]) -> AppResult<()> {
    match args.first().map(String::as_str) {
        None | Some("status") => print_vendor_tools(&app.vendor_tools()),
        Some(command) => {
            return Err(AppError::InvalidCommand(format!(
                "unknown tools command '{command}'. Run `sts2_mod_manager help`."
            )));
        }
    }

    Ok(())
}

fn required_arg<'a>(args: &'a [String], index: usize, usage: &str) -> AppResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| AppError::InvalidCommand(format!("missing argument. Usage: {usage}")))
}

fn print_presets(presets: &[Preset]) {
    if presets.is_empty() {
        println!("No presets saved.");
        return;
    }

    println!("Presets: {}", presets.len());
    for preset in presets {
        println!("  - {} ({} mod keys)", preset.name, preset.keys.len());
    }
}

fn print_preset(preset: &Preset) {
    println!("Preset: {}", preset.name);
    if preset.keys.is_empty() {
        println!("  No mods in this preset.");
    } else {
        for key in &preset.keys {
            println!("  - {key}");
        }
    }
}

fn print_preset_apply_report(report: &PresetApplyReport) {
    println!("Preset applied.");
    println!("  Disabled before apply: {}", report.disabled.len());
    println!("  Enabled from vault:    {}", report.enabled.len());
    if !report.version_warnings.is_empty() {
        println!("  Version warnings:");
        for warning in &report.version_warnings {
            println!("    - {warning}");
        }
    }
    if !report.missing.is_empty() {
        println!("  Missing vault mods:");
        for key in &report.missing {
            println!("    - {key}");
        }
    }
}

fn print_preset_export_report(report: &PresetExportReport) {
    println!("Preset exported.");
    println!("  archive: {}", report.archive_path.display());
    println!("  included mods: {}", report.included_mods);
    if !report.missing.is_empty() {
        println!("  missing vault mods:");
        for key in &report.missing {
            println!("    - {key}");
        }
    }
}

fn print_preset_import_report(report: &PresetImportReport) {
    println!("Preset imported.");
    println!("  preset: {}", report.preset.name);
    println!("  mod keys: {}", report.preset.keys.len());
    println!("  imported mods: {}", report.imported_mods);
}

fn print_translation_extract_report(report: &TranslationExtractReport) {
    println!("Translation workspace created.");
    println!("  Mod key:      {}", report.mod_key);
    println!("  Version:      {}", report.version_id);
    println!("  Workspace:    {}", report.workspace_dir.display());
    println!("  Candidates:   {}", report.candidates.len());
    if report.review_required {
        println!("  Review needed: yes");
        if let Some(path) = &report.review_path {
            println!("  Review file:  {}", path.display());
        }
    } else {
        println!("  Review needed: no");
    }
}

fn print_translation_workspaces(workspaces: &[TranslationWorkspace]) {
    if workspaces.is_empty() {
        println!("No translation workspaces found.");
        return;
    }

    println!("Translation workspaces: {}", workspaces.len());
    for workspace in workspaces {
        let review = if workspace.review_required {
            "review required"
        } else {
            "ready"
        };
        println!(
            "  - {}/{} [{}] {}",
            workspace.mod_key,
            workspace.version_id,
            review,
            workspace.path.display()
        );
    }
}

fn print_translation_merge_report(report: &TranslationMergeReport) {
    println!("Merged translated files: {}", report.merged_files.len());
    println!("Backup folder: {}", report.backup_dir.display());
    for path in &report.merged_files {
        println!("  - {}", path.display());
    }
}

fn print_launch_status(status: &LaunchStatus) {
    if let Some(path) = &status.game_exe {
        println!("Game executable: {}", path.display());
        println!("Launch ready: yes");
    } else if let Some(path) = &status.steam_exe {
        println!("Game executable: not found");
        println!("Steam executable: {}", path.display());
        println!("Launch ready: yes (Steam app fallback)");
    } else {
        println!("Game executable: not found");
        println!("Launch ready: no");
        println!("Set STS2_GAME_EXE or choose the executable in Settings.");
    }
}

fn print_launch_report(report: &LaunchReport) {
    println!("Game launched.");
    println!("  target:  {}", report.target);
    println!("  pid:     {}", report.process_id);
    println!(
        "  mode:    {}",
        if report.vanilla_mode {
            "vanilla-safe"
        } else {
            "current mods"
        }
    );
    println!("  save backups: {}", report.save_backups_created);
    if report.seeded_modded_profiles > 0 {
        println!(
            "  seeded modded profiles: {}",
            report.seeded_modded_profiles
        );
    }
    if let Some(warning) = &report.save_backup_warning {
        println!("  save backup warning: {warning}");
    }
}

fn print_vendor_tools(tools: &[VendorTool]) {
    println!("Embedded helper tools:");
    for tool in tools {
        println!(
            "  - {}: {}",
            tool.name,
            if tool.available {
                "available"
            } else {
                "missing"
            }
        );
        println!("    purpose: {}", tool.purpose);
        println!("    path:    {}", tool.expected_path.display());
    }
}

fn print_vault_entries(entries: &[VaultEntry]) {
    if entries.is_empty() {
        println!("Vault is empty.");
        return;
    }

    println!("Vault mods: {}", entries.len());
    for entry in entries {
        println!("  - {}: {} [{}]", entry.key, entry.display_name, entry.kind);
    }
}

fn print_action(label: &str, action: &VaultAction) {
    println!("{label}: {}", action.key);
    println!("  from: {}", action.from.display());
    println!("  to:   {}", action.to.display());
}

fn print_paths(config: &AppConfig) {
    println!("Workspace: {}", config.workspace_dir.display());
    println!("Game:      {}", config.game_dir.display());
    println!("Game mods: {}", config.game_mods_dir.display());
    println!("Vault:     {}", config.vault_dir.display());
    println!("Presets:   {}", config.presets_dir.display());
    println!("Translate: {}", config.translation_work_dir.display());
    println!("Logs:      {}", config.logs_dir.display());
    println!("State:     {}", config.state_dir.display());
    println!("Vendor:    {}", config.vendor_dir.display());

    if config.external_manager_dirs.is_empty() {
        println!("Nexus/Vortex paths: none configured");
    } else {
        println!("Nexus/Vortex paths:");
        for path in &config.external_manager_dirs {
            println!("  - {}", path.display());
        }
    }
}

fn print_scan_report(report: &ScanReport) {
    print_scan_summary(&report.summary);
    print_changes(&report.changes);
}

fn print_scan_summary(summary: &ScanSummary) {
    println!("Slay the Spire 2 Mod Manager");
    println!("Scan complete.");
    println!();

    if summary.total_mods() == 0 {
        println!("No mods found. Vanilla-safe startup is ready.");
        return;
    }

    println!("Total discovered mods: {}", summary.total_mods());
    println!(
        "Vanilla-safe startup: {}",
        if summary.is_vanilla_safe() {
            "ready"
        } else {
            "game mods present"
        }
    );
    println!();

    print_mods("Game mods", &summary.game_mods);
    print_mods("Managed vault", &summary.vault_mods);
    print_mods("Nexus/Vortex", &summary.external_manager_mods);
}

fn print_changes(changes: &[ModChange]) {
    println!();
    if changes.is_empty() {
        println!("Detected updates: none since last scan.");
        return;
    }

    println!("Detected updates since last scan: {}", changes.len());
    for change in changes {
        println!(
            "  - {}: {} ({})",
            change.kind, change.record.name, change.record.source
        );
    }
}

fn print_mods(title: &str, mods: &[ModRecord]) {
    println!("{title}: {}", mods.len());
    for record in mods {
        let version = record
            .version_hint
            .as_deref()
            .map(|value| format!(" {value}"))
            .unwrap_or_default();
        println!(
            "  - {}{} [{} | {} | {} bytes]",
            record.name, version, record.source, record.kind, record.fingerprint.bytes
        );
    }
}

fn print_translation_candidates(path: &Path) -> AppResult<()> {
    let candidates = scan_translation_candidates(path)?;

    if candidates.is_empty() {
        println!("No translation candidates found in {}.", path.display());
        return Ok(());
    }

    println!("Translation candidates in {}:", path.display());
    for candidate in candidates {
        println!(
            "  - {} [{} | {}]",
            candidate.path.display(),
            candidate.extension,
            candidate.reason
        );
    }

    Ok(())
}
