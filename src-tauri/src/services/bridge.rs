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

/// Applies a sheet through the bridge: its loc table values go to the
/// shared folder and the bridge puts them into the game. `Ok(None)` for
/// sheets the bridge cannot carry yet (DLL strings, non-table files), which
/// keep the previous apply path.
fn apply_sheet_to_bridge(sheet_path: &Path) -> Result<Option<JsonApplyDto>, String> {
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    if is_hardcoded_source_file(Path::new(&sheet.source_path)) {
        return Ok(None);
    }
    let (tables, unsupported) = bridge::sheet_tables(&sheet);
    if tables.is_empty() {
        return Ok(None);
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
    let report = bridge::write_translations(
        &shared,
        &target.mod_id,
        &sheet.target_language,
        &tables,
        target.mod_version.as_deref(),
        epoch_seconds(Some(SystemTime::now())).unwrap_or(0),
    )
    .map_err(|error| error.to_string())?;
    if report.entries == 0 {
        return Err("적용할 번역값이 없습니다. translated_value를 입력하거나 CSV/JSON을 먼저 매칭해 주세요.".to_string());
    }
    let disabled_patch_mods = disable_patch_mods_once(&app, &summary, &target.mod_id)?;
    record_translation_apply(
        sheet_path,
        &PckPatchReport {
            language_output_path: report.dir.clone(),
            packed_pck_path: None,
            installed_mod_path: Some(report.dir.clone()),
            applied_entries: report.entries,
        },
        config,
    )?;

    let bridge_row = find_bridge_record(&summary);
    let desired = desired_active_mod_keys(&summary, &config.state_dir).unwrap_or_default();
    Ok(Some(JsonApplyDto {
        output_path: display_path(&report.dir),
        applied_entries: report.entries,
        language_output_path: display_path(&report.dir),
        packed_pck_path: String::new(),
        installed_mod_path: display_path(&report.dir),
        bridge: Some(BridgeApplyDto {
            mod_id: target.mod_id,
            language: sheet.target_language.clone(),
            tables: report.tables,
            unsupported_entries: unsupported.total(),
            unsupported_hardcoded: unsupported.hardcoded,
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
