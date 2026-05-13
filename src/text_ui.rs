use crate::app::App;
use crate::error::{AppError, AppResult};
use crate::vault::DisabledModAction;
use std::io::{self, Write};
use std::path::Path;

pub fn run(app: &App) -> AppResult<()> {
    app.ensure_workspace_dirs()?;

    loop {
        print_dashboard(app)?;
        println!();
        println!("Choose an action:");
        println!("  1. Scan mods and update change state");
        println!("  2. Disabled mods: list/import/enable/disable");
        println!("  3. Presets: list/save/apply");
        println!("  4. Translation: list/extract/merge");
        println!("  5. Vanilla-safe mode: disable all game mods");
        println!("  6. Launch game");
        println!("  7. Embedded tools status");
        println!("  q. Quit");

        match prompt("Selection")?.as_str() {
            "1" => {
                let report = app.scan_and_update_state()?;
                println!(
                    "Scan complete: {} mod(s), {} change(s).",
                    report.summary.total_mods(),
                    report.changes.len()
                );
                pause()?;
            }
            "2" => vault_menu(app)?,
            "3" => preset_menu(app)?,
            "4" => translation_menu(app)?,
            "5" => {
                if confirm("Disable all currently enabled game mods?")? {
                    let actions = app.disable_all_mods()?;
                    println!("Disabled {} mod(s).", actions.len());
                    for action in actions {
                        print_action("Moved", &action);
                    }
                }
                pause()?;
            }
            "6" => launch_menu(app)?,
            "7" => tools_status(app)?,
            "q" | "Q" => break,
            other => {
                println!("Unknown selection: {other}");
                pause()?;
            }
        }
    }

    Ok(())
}

fn print_dashboard(app: &App) -> AppResult<()> {
    let scan = app.scan()?;
    println!();
    println!("Slay the Spire 2 Mod Manager");
    println!("================================");
    println!("Game mods:      {}", scan.game_mods.len());
    println!("Disabled mods:  {}", app.list_vault()?.len());
    println!("External mods:  {}", scan.external_manager_mods.len());
    println!("Presets:        {}", app.list_presets()?.len());
    println!(
        "Vanilla-safe:   {}",
        if scan.is_vanilla_safe() { "yes" } else { "no" }
    );
    println!("Game folder:    {}", app.config().game_dir.display());
    if let Some(path) = app.launch_status().game_exe {
        println!("Executable:     {}", path.display());
    } else {
        let status = app.launch_status();
        if let Some(path) = status.steam_exe {
            println!("Executable:     Steam fallback ({})", path.display());
        } else {
            println!("Executable:     not found");
        }
    }
    Ok(())
}

fn vault_menu(app: &App) -> AppResult<()> {
    println!();
    println!("Disabled mods");
    println!("  1. List");
    println!("  2. Import path");
    println!("  3. Enable key");
    println!("  4. Disable key");
    println!("  b. Back");

    match prompt("Disabled mod action")?.as_str() {
        "1" => {
            let entries = app.list_vault()?;
            if entries.is_empty() {
                println!("Disabled mod storage is empty.");
            } else {
                for entry in entries {
                    println!("  - {}: {} [{}]", entry.key, entry.display_name, entry.kind);
                }
            }
        }
        "2" => {
            let path = prompt("Path to mod file/folder")?;
            let action = app.import_mod(Path::new(&path))?;
            print_action("Imported", &action);
        }
        "3" => {
            let key = prompt("Disabled mod key to enable")?;
            let action = app.enable_mod(&key)?;
            print_action("Enabled", &action);
        }
        "4" => {
            let key = prompt("Enabled mod key to disable")?;
            let action = app.disable_mod(&key)?;
            print_action("Disabled", &action);
        }
        "b" | "B" => return Ok(()),
        other => println!("Unknown disabled mod action: {other}"),
    }
    pause()
}

fn preset_menu(app: &App) -> AppResult<()> {
    println!();
    println!("Presets");
    println!("  1. List");
    println!("  2. Save current enabled mods");
    println!("  3. Apply preset");
    println!("  b. Back");

    match prompt("Preset action")?.as_str() {
        "1" => {
            let presets = app.list_presets()?;
            if presets.is_empty() {
                println!("No presets saved.");
            } else {
                for preset in presets {
                    println!("  - {} ({} mod keys)", preset.name, preset.keys.len());
                }
            }
        }
        "2" => {
            let name = prompt("Preset name")?;
            let preset = app.save_preset(&name)?;
            println!(
                "Saved preset '{}' with {} mod key(s).",
                preset.name,
                preset.keys.len()
            );
        }
        "3" => {
            let name = prompt("Preset name")?;
            let report = app.apply_preset(&name)?;
            println!(
                "Applied preset. Enabled {}, missing {}.",
                report.enabled.len(),
                report.missing.len()
            );
            for key in report.missing {
                println!("  missing: {key}");
            }
        }
        "b" | "B" => return Ok(()),
        other => println!("Unknown preset action: {other}"),
    }
    pause()
}

fn translation_menu(app: &App) -> AppResult<()> {
    println!();
    println!("Translation");
    println!("  1. List workspaces");
    println!("  2. Extract from path");
    println!("  3. Merge workspace into target");
    println!("  b. Back");

    match prompt("Translation action")?.as_str() {
        "1" => {
            let workspaces = app.list_translation_workspaces()?;
            if workspaces.is_empty() {
                println!("No translation workspaces found.");
            } else {
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
        }
        "2" => {
            let path = prompt("Source mod folder/file")?;
            let report = app.extract_translation(Path::new(&path))?;
            println!(
                "Extracted {} candidate(s) to {}.",
                report.candidates.len(),
                report.workspace_dir.display()
            );
            if let Some(review_path) = report.review_path {
                println!("Review required: {}", review_path.display());
            }
        }
        "3" => {
            let workspace = prompt("Workspace path")?;
            let target = prompt("Target folder")?;
            let report = app.merge_translation(Path::new(&workspace), Path::new(&target))?;
            println!(
                "Merged {} file(s). Backup: {}",
                report.merged_files.len(),
                report.backup_dir.display()
            );
        }
        "b" | "B" => return Ok(()),
        other => println!("Unknown translation action: {other}"),
    }
    pause()
}

fn launch_menu(app: &App) -> AppResult<()> {
    println!();
    println!("Launch");
    println!("  1. Show status");
    println!("  2. Launch with current mods");
    println!("  3. Disable mods and launch vanilla-safe");
    println!("  b. Back");

    match prompt("Launch action")?.as_str() {
        "1" => {
            let status = app.launch_status();
            if let Some(path) = status.game_exe {
                println!("Executable: {}", path.display());
            } else if let Some(path) = status.steam_exe {
                println!("Executable not found. Steam fallback: {}", path.display());
            } else {
                println!("Executable not found. Set STS2_GAME_EXE.");
            }
        }
        "2" => {
            let report = app.launch_current()?;
            println!("Launched pid {} from {}", report.process_id, report.target);
        }
        "3" => {
            if confirm("Disable all mods before launching?")? {
                let report = app.launch_vanilla()?;
                println!(
                    "Launched vanilla-safe pid {} from {}",
                    report.process_id, report.target
                );
            }
        }
        "b" | "B" => return Ok(()),
        other => println!("Unknown launch action: {other}"),
    }
    pause()
}

fn tools_status(app: &App) -> AppResult<()> {
    println!();
    println!("Embedded Tools");
    for tool in app.vendor_tools() {
        println!(
            "  - {}: {}",
            tool.name,
            if tool.available {
                "available"
            } else {
                "missing"
            }
        );
        println!("    {}", tool.expected_path.display());
    }
    pause()
}

fn prompt(label: &str) -> AppResult<String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| AppError::io("stdout", error))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| AppError::io("stdin", error))?;
    Ok(input.trim().to_string())
}

fn confirm(label: &str) -> AppResult<bool> {
    let answer = prompt(&format!("{label} [y/N]"))?;
    Ok(matches!(answer.as_str(), "y" | "Y" | "yes" | "YES"))
}

fn pause() -> AppResult<()> {
    let _ = prompt("Press Enter to continue")?;
    Ok(())
}

fn print_action(label: &str, action: &DisabledModAction) {
    println!("{label}: {}", action.key);
    println!("  from: {}", action.from.display());
    println!("  to:   {}", action.to.display());
}
