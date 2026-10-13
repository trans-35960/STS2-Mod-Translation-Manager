/// Adds the mod's own text in each of its language folders, so the edit
/// panel can show (and copy) them next to the source. Best effort: a mod
/// whose files cannot be found just shows the source.
fn add_other_languages(sheet: &JsonTranslationSheet, info: &mut bridge::InfoTables) {
    let Some(scan_root) = compare_scan_root(sheet) else {
        return;
    };
    let Some(localization_dir) = sheet
        .entries
        .iter()
        .find_map(|entry| localization_dir_of_entry(&entry.key))
    else {
        return;
    };
    let source_language = sheet
        .entries
        .iter()
        .find_map(|entry| localization_language_component(Path::new(&split_translation_key(&entry.key).0)));
    let Ok(language_dirs) = fs::read_dir(scan_root.join(&localization_dir)) else {
        return;
    };
    for language_dir in language_dirs.flatten() {
        if !language_dir.path().is_dir() {
            continue;
        }
        let language = language_dir.file_name().to_string_lossy().to_string();
        if Some(&language) == source_language.as_ref()
            || language.eq_ignore_ascii_case(&sheet.target_language)
        {
            continue;
        }
        let sample = localization_dir.join(&language);
        let Ok(values) = compare_translation_language_by_files(sheet, &scan_root, &language, &sample)
        else {
            continue;
        };
        for value in values {
            let Some((table, key)) = bridge::sheet_key_table(&value.key) else {
                continue;
            };
            if let Some(entry) = info.get_mut(&table).and_then(|keys| keys.get_mut(&key)) {
                entry.langs.insert(language.clone(), value.value);
            }
        }
    }
}

/// `…/localization` of a sheet key's file, relative to the mod root.
fn localization_dir_of_entry(sheet_key: &str) -> Option<PathBuf> {
    let (file, _) = split_translation_key(sheet_key);
    let parts = path_components(Path::new(&file));
    let index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    Some(parts.iter().take(index + 1).collect())
}

/// Which mod a sheet translates, as the game knows it (manifest id).
struct BridgeTarget {
    mod_id: String,
    mod_version: Option<String>,
}

fn bridge_target_for_sheet(
    app: &App,
    summary: &ScanSummary,
    sheet: &JsonTranslationSheet,
) -> Option<BridgeTarget> {
    let mod_key = mod_key_for_translation_sheet(sheet)?;
    let record = summary
        .clone()
        .into_records()
        .find(|record| record.stable_key() == mod_key)?;
    let scan_root = if record.path.is_dir() {
        record.path.clone()
    } else {
        let source = extraction_source_for_record(&record);
        let cache_key = language_cache_key(&record, &source, &app.config().vendor_dir);
        extraction_scan_root(&source, &cache_key, &app.config().vendor_dir)?
    };
    let manifest = read_mod_manifest_for_record(&record.path, &scan_root);
    // A sheet opened from a translation patch still targets the original mod.
    if manifest.is_translation_patch {
        return Some(BridgeTarget {
            mod_id: manifest.target_mod_id?,
            mod_version: manifest.target_mod_version,
        });
    }
    Some(BridgeTarget {
        mod_id: manifest.id?,
        mod_version: manifest.version,
    })
}

/// Applies a sheet through the bridge: loc table values go to
/// `translations/`, DLL string literals to `strings/`, and the bridge puts
/// them into the game. `Ok(None)` for sheets the bridge cannot carry (files
/// outside `localization/`), which keep the previous apply path.
fn apply_sheet_to_bridge(sheet_path: &Path) -> Result<Option<JsonApplyDto>, String> {
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    let source_path = PathBuf::from(&sheet.source_path);
    let dll_assembly = is_hardcoded_source_file(&source_path)
        .then(|| source_path.file_stem().map(|stem| stem.to_string_lossy().to_string()))
        .flatten();
    let mut tables = bridge::SheetTables::default();
    let mut strings = bridge::SheetStrings::default();
    if dll_assembly.is_some() {
        strings = bridge::sheet_strings(&sheet);
    } else {
        tables = bridge::sheet_tables(&sheet);
        if tables.values.is_empty() {
            return Ok(None);
        }
        add_other_languages(&sheet, &mut tables.info);
    }
    let app = app();
    let config = app.config();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let target = bridge_target_for_sheet(&app, &summary, &sheet)
        .ok_or_else(|| "번역 대상 모드의 manifest id를 찾지 못해 브리지로 적용할 수 없습니다.".to_string())?;
    let shared = bridge::shared_root()
        .ok_or_else(|| "공용 폴더(%APPDATA%)를 찾지 못했습니다.".to_string())?;
    let updated_at = epoch_seconds(Some(SystemTime::now())).unwrap_or(0);
    let (output_dir, entries, table_count) = match dll_assembly.as_deref() {
        Some(assembly) => {
            // An emptied sheet still writes, so the bridge stops replacing.
            let path = bridge::write_strings(
                &shared,
                &target.mod_id,
                &sheet.target_language,
                assembly,
                &strings.strings,
                updated_at,
            )
            .map_err(|error| error.to_string())?;
            let dir = path.parent().map(Path::to_path_buf).unwrap_or(path);
            (dir, strings.strings.len(), 0)
        }
        None => {
            let report = bridge::write_translations(
                &shared,
                &target.mod_id,
                &sheet.target_language,
                &tables,
                target.mod_version.as_deref(),
                updated_at,
            )
            .map_err(|error| error.to_string())?;
            (report.dir, report.entries, report.tables)
        }
    };
    if entries == 0 && dll_assembly.is_none() {
        return Err("적용할 번역값이 없습니다. translated_value를 입력하거나 CSV/JSON을 먼저 매칭해 주세요.".to_string());
    }
    let disabled_patch_mods = disable_patch_mods_once(&app, &summary, &target.mod_id)?;
    record_translation_apply(
        sheet_path,
        &PckPatchReport {
            language_output_path: output_dir.clone(),
            packed_pck_path: None,
            installed_mod_path: Some(output_dir.clone()),
            applied_entries: entries,
        },
        config,
    )?;

    let bridge_row = find_bridge_record(&summary);
    let desired = desired_active_mod_keys(&summary, &config.state_dir).unwrap_or_default();
    Ok(Some(JsonApplyDto {
        output_path: display_path(&output_dir),
        applied_entries: entries,
        language_output_path: display_path(&output_dir),
        packed_pck_path: String::new(),
        installed_mod_path: display_path(&output_dir),
        bridge: Some(BridgeApplyDto {
            mod_id: target.mod_id,
            language: sheet.target_language.clone(),
            tables: table_count,
            unsupported_entries: tables.unsupported.total(),
            unsupported_hardcoded: tables.unsupported.hardcoded,
            dll_strings: dll_assembly.is_some().then_some(entries),
            code_like_skipped: strings.code_like,
            disabled_patch_mods,
            bridge_installed: bridge_row.is_some(),
            bridge_active: bridge_row.is_some_and(|record| desired.contains(&record.stable_key())),
            game_running: app.launch_status().running,
        }),
    }))
}

/// The first time a mod is applied through the bridge, its translation patch
/// mods are switched off: both would apply the same text, and keeping two
/// copies is how they drift apart. Remembered per mod id, so a patch the user
/// turns back on stays on.
fn disable_patch_mods_once(
    app: &App,
    summary: &ScanSummary,
    mod_id: &str,
) -> Result<Vec<String>, String> {
    let path = app
        .config()
        .state_dir
        .join("bridge")
        .join("patch_mods_disabled.json");
    let mut handled = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default();
    if !handled.insert(mod_id.to_string()) {
        return Ok(Vec::new());
    }
    let desired = desired_active_mod_keys(summary, &app.config().state_dir)
        .map_err(|error| error.to_string())?;
    let mut disabled = Vec::new();
    let mut changes = Vec::new();
    for record in summary.clone().into_records() {
        let key = record.stable_key();
        if !desired.contains(&key) || !record.path.is_dir() {
            continue;
        }
        let manifest = read_mod_manifest_info(&record.path);
        if manifest.is_translation_patch
            && manifest
                .target_mod_id
                .as_deref()
                .is_some_and(|target| target.eq_ignore_ascii_case(mod_id))
        {
            disabled.push(manifest.name.unwrap_or_else(|| record.name.clone()));
            changes.push((key, false));
        }
    }
    if !changes.is_empty() {
        app.set_mods_desired_active(&changes)
            .map_err(|error| error.to_string())?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(&handled).map_err(|error| error.to_string())?;
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(disabled)
}

fn bridge_manifest_file_name() -> String {
    format!("{}.json", bridge::BRIDGE_MOD_ID)
}

fn find_bridge_record(summary: &ScanSummary) -> Option<ModRecord> {
    summary
        .clone()
        .into_records()
        .find(|record| record.path.is_dir() && record.path.join(bridge_manifest_file_name()).is_file())
}

fn bundled_bridge_dir(config: &AppConfig) -> PathBuf {
    config.vendor_dir.join("bridge")
}

/// Puts the bridge first in the game's mod order. Mods without dependencies
/// load in that order, and the bridge has to be loaded before a mod's
/// assembly to replace DLL strings its initializer already uses. Best effort:
/// the game creates `settings.save` on its first run.
fn ensure_bridge_loads_first(app: &App) {
    let Some(settings_path) = app.config().game_settings_path() else {
        return;
    };
    if let Err(error) = game_settings::move_mod_first(
        &settings_path,
        &app.config().save_backup_dir,
        bridge::BRIDGE_MOD_ID,
        game_settings::SOURCE_MODS_DIRECTORY,
    ) {
        eprintln!("bridge load order not set: {error}");
    }
}

/// Installs the bridge bundled with the app into the game's mods folder, or
/// updates an older copy wherever it is (active or disabled). A fresh install
/// is switched on; an existing copy keeps the on/off state the user chose.
pub(crate) fn ensure_bridge_installed() -> Result<BridgeInstallDto, String> {
    let app = app();
    let config = app.config();
    let source = bundled_bridge_dir(config);
    let dll_name = format!("{}.dll", bridge::BRIDGE_MOD_ID);
    let Some(bundled_version) = bridge::manifest_version(&source.join(bridge_manifest_file_name()))
        .filter(|_| source.join(&dll_name).is_file())
    else {
        return Ok(BridgeInstallDto {
            state: "missing_source".to_string(),
            version: None,
            message: format!(
                "프로그램에 브리지 모드가 들어 있지 않습니다({}). scripts/build-bridge.ps1로 빌드하세요.",
                display_path(&source)
            ),
        });
    };
    let summary = app.scan().map_err(|error| error.to_string())?;
    let installed = find_bridge_record(&summary);
    let installed_version = installed
        .as_ref()
        .and_then(|record| bridge::manifest_version(&record.path.join(bridge_manifest_file_name())));
    // A rebuild without a version bump (development) still has to reach the
    // game, so the same version with a different DLL counts as an update.
    let needs_copy = match (installed_version.as_deref(), installed.as_ref()) {
        (Some(version), Some(record)) => {
            bridge::is_newer_version(&bundled_version, version)
                || (!bridge::is_newer_version(version, &bundled_version)
                    && fs::read(source.join(&dll_name)).ok()
                        != fs::read(record.path.join(&dll_name)).ok())
        }
        _ => true,
    };
    if !needs_copy {
        if !app.launch_status().running {
            ensure_bridge_loads_first(&app);
        }
        return Ok(BridgeInstallDto {
            state: "current".to_string(),
            version: installed_version,
            message: String::new(),
        });
    }
    if app.launch_status().running {
        return Ok(BridgeInstallDto {
            state: "game_running".to_string(),
            version: installed_version,
            message: "게임이 실행 중이라 브리지 모드를 설치/갱신하지 못했습니다. 게임을 끈 뒤 다시 열어 주세요.".to_string(),
        });
    }
    let target = installed
        .as_ref()
        .map(|record| record.path.clone())
        .unwrap_or_else(|| config.game_mods_dir.join(bridge::BRIDGE_MOD_ID));
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    for file in [dll_name.clone(), bridge_manifest_file_name()] {
        fs::copy(source.join(&file), target.join(&file))
            .map_err(|error| format!("브리지 모드 복사 실패 ({file}): {error}"))?;
    }
    if installed.is_none() {
        let summary = app.scan().map_err(|error| error.to_string())?;
        if let Some(record) = find_bridge_record(&summary) {
            app.set_mods_desired_active(&[(record.stable_key(), true)])
                .map_err(|error| error.to_string())?;
        }
    }
    ensure_bridge_loads_first(&app);
    Ok(BridgeInstallDto {
        state: if installed.is_some() { "updated" } else { "installed" }.to_string(),
        version: Some(bundled_version.clone()),
        message: if installed.is_some() {
            format!("브리지 모드를 {bundled_version}(으)로 갱신했습니다.")
        } else {
            format!("브리지 모드 {bundled_version}을(를) 설치했습니다. 게임 안에서 F8로 번역을 다시 불러올 수 있습니다.")
        },
    })
}

/// Bridge state for the dashboard: install, on/off, and what it last applied.
fn bridge_dto(config: &AppConfig, rows: &[ModRowDto]) -> BridgeDto {
    let row = rows.iter().find(|row| row.system_mod);
    let bundled_version =
        bridge::manifest_version(&bundled_bridge_dir(config).join(bridge_manifest_file_name()));
    let shared = bridge::shared_root();
    BridgeDto {
        installed: row.is_some(),
        active: row.is_some_and(|row| row.active),
        installed_version: row.and_then(|row| row.version_hint.clone()),
        bundled_version,
        shared_dir: shared.as_deref().map(display_path).unwrap_or_default(),
        status: shared.as_deref().and_then(bridge::read_status).map(|status| BridgeStatusDto {
            bridge_version: status.bridge_version,
            game_version: status.game_version,
            language: status.language,
            applied_at: status.applied_at,
            applied_entries: status.mods.values().sum(),
            mods: status.mods.into_iter().collect(),
            invalid_count: status.invalid_count,
            invalid: status
                .invalid
                .into_iter()
                .take(20)
                .map(|entry| format!("{}/{}/{}: {}", entry.mod_id, entry.table, entry.key, entry.error))
                .collect(),
            missing_tables: status.missing_tables,
            errors: status.errors,
        }),
    }
}

fn bridge_diagnostics(bridge: &BridgeDto) -> Vec<TroubleshootDiagnosticDto> {
    let diagnostic = |id: &str, severity: &str, title: &str, detail: String| TroubleshootDiagnosticDto {
        id: id.to_string(),
        category: "translation".to_string(),
        severity: severity.to_string(),
        title: title.to_string(),
        detail,
        action_label: "공용 폴더 열기".to_string(),
        related_path: bridge.shared_dir.clone(),
        mod_key: None,
        can_auto_fix: false,
    };
    let mut diagnostics = Vec::new();
    if !bridge.installed {
        diagnostics.push(diagnostic(
            "bridge-missing",
            "warn",
            "번역 브리지 모드가 설치되어 있지 않습니다",
            "번역 도구에서 적용한 번역은 브리지 모드가 게임에 넣습니다. 프로그램을 다시 열면 자동으로 설치합니다.".to_string(),
        ));
        return diagnostics;
    }
    if !bridge.active {
        diagnostics.push(diagnostic(
            "bridge-inactive",
            "warn",
            "번역 브리지 모드가 꺼져 있습니다",
            "모드 관리에서 STS2 Mod Manager Bridge를 켜야 적용한 번역이 게임에 나옵니다.".to_string(),
        ));
    }
    let Some(status) = &bridge.status else {
        return diagnostics;
    };
    if status.invalid_count > 0 {
        diagnostics.push(diagnostic(
            "bridge-invalid",
            "warn",
            "형식이 깨져 적용하지 않은 번역이 있습니다",
            format!(
                "게임의 문자열 검사에 실패한 번역 {}개를 건너뛰었습니다(그대로 넣으면 게임이 멈출 수 있습니다). 예: {}",
                status.invalid_count,
                status.invalid.first().cloned().unwrap_or_default()
            ),
        ));
    }
    if !status.errors.is_empty() || !status.missing_tables.is_empty() {
        let mut details = status.errors.clone();
        if !status.missing_tables.is_empty() {
            details.push(format!("게임에 없는 테이블: {}", status.missing_tables.join(", ")));
        }
        diagnostics.push(diagnostic(
            "bridge-errors",
            "warn",
            "브리지 모드가 번역 일부를 적용하지 못했습니다",
            details.join(" / "),
        ));
    }
    diagnostics
}

/// The shared folder and the mod a sheet translates, for in-game edits.
fn bridge_edit_target(sheet: &JsonTranslationSheet) -> Result<(PathBuf, String), String> {
    let app = app();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let target = bridge_target_for_sheet(&app, &summary, sheet)
        .ok_or_else(|| "번역 대상 모드의 manifest id를 찾지 못했습니다.".to_string())?;
    let shared = bridge::shared_root()
        .ok_or_else(|| "공용 폴더(%APPDATA%)를 찾지 못했습니다.".to_string())?;
    Ok((shared, target.mod_id))
}

fn bridge_edits_dto(sheet: &JsonTranslationSheet, shared: &Path, mod_id: &str) -> BridgeEditsDto {
    let language = sheet.target_language.as_str();
    let ack = bridge_edits::read_edits_ack(shared, mod_id, language);
    let pending = bridge_edits::pending_edits(
        &bridge_edits::read_edits(shared, mod_id, language),
        ack.as_deref(),
    );
    let sheet_keys = sheet
        .entries
        .iter()
        .filter_map(|entry| bridge::sheet_key_table(&entry.key).map(|slot| (slot, entry.key.clone())))
        .collect::<BTreeMap<_, _>>();
    let report_dto = |record: &bridge_edits::EditRecord, pending: bool| BridgeReportDto {
        id: record.id.clone(),
        table: record.table.clone(),
        key: record.key.clone(),
        sheet_key: sheet_keys
            .get(&(record.table.clone(), record.key.clone()))
            .cloned(),
        source: record.source.clone(),
        shown: record.before.clone(),
        note: record.note.clone(),
        pending,
    };
    let stored_reports = bridge_edits::read_stored_reports(&app().config().state_dir);
    let stored = stored_reports
        .iter()
        .filter(|item| item.mod_id == mod_id && item.language == language)
        .map(|item| (item.record.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let mut reports = stored
        .values()
        .filter(|item| !item.resolved)
        .map(|item| report_dto(&item.record, false))
        .collect::<Vec<_>>();
    reports.extend(
        pending
            .reports
            .iter()
            .filter(|record| !stored.contains_key(&record.id))
            .map(|record| report_dto(record, true)),
    );
    BridgeEditsDto {
        mod_id: mod_id.to_string(),
        language: language.to_string(),
        items: bridge_edits::edit_import_items(sheet, &pending)
            .into_iter()
            .map(|item| BridgeEditItemDto {
                kind: match item.kind {
                    bridge_edits::EditImportKind::GameChanged => "game_changed",
                    bridge_edits::EditImportKind::Conflict => "conflict",
                }
                .to_string(),
                sheet_key: item.sheet_key,
                table: item.table,
                key: item.key,
                source: item.source,
                sheet_value: item.sheet_value,
                game_value: item.game_value,
            })
            .collect(),
        reports,
        ack_id: pending.last_id,
    }
}

pub(crate) fn bridge_edits_preview(sheet: JsonSheetDto) -> Result<BridgeEditsDto, String> {
    let sheet = json_sheet_from_dto(sheet)?;
    let (shared, mod_id) = bridge_edit_target(&sheet)?;
    Ok(bridge_edits_dto(&sheet, &shared, &mod_id))
}

/// Called once the chosen game values are in the sheet: keeps new reports
/// and moves `edits_ack` past everything the preview showed.
pub(crate) fn acknowledge_bridge_edits(
    sheet: JsonSheetDto,
    ack_id: String,
) -> Result<BridgeEditsDto, String> {
    let sheet = json_sheet_from_dto(sheet)?;
    let (shared, mod_id) = bridge_edit_target(&sheet)?;
    bridge_edits::acknowledge(
        &shared,
        &app().config().state_dir,
        &mod_id,
        &sheet.target_language,
        &ack_id,
    )
    .map_err(|error| error.to_string())?;
    Ok(bridge_edits_dto(&sheet, &shared, &mod_id))
}

/// Marks a report handled without acking the edits before it.
pub(crate) fn resolve_bridge_report(sheet: JsonSheetDto, id: String) -> Result<BridgeEditsDto, String> {
    let sheet = json_sheet_from_dto(sheet)?;
    let (shared, mod_id) = bridge_edit_target(&sheet)?;
    bridge_edits::resolve_report(
        &shared,
        &app().config().state_dir,
        &mod_id,
        &sheet.target_language,
        &id,
    )
    .map_err(|error| error.to_string())?;
    Ok(bridge_edits_dto(&sheet, &shared, &mod_id))
}
