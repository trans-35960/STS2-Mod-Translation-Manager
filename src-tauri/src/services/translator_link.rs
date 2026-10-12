/// An installed Sts2ModTranslator, found by its manifest next to the DLL.
struct TranslatorInstall {
    root: PathBuf,
    version: String,
    active: bool,
}

/// Where "번역 저장/적용" sends a sheet, decided per mod (see
/// `docs/translation/translator-link.md` 3.7).
struct SaveTargetDecision {
    mode: translator_link::SaveTargetMode,
    translator: Option<TranslatorInstall>,
    /// Set when the sheet goes to Translator.
    target: Option<TranslatorTarget>,
    reason: String,
    supported_entries: usize,
    unsupported: translator_link::UnsupportedEntries,
}

struct TranslatorTarget {
    mod_key: String,
    mod_id: String,
    mod_version: Option<String>,
    language: String,
}

fn detect_translator(app: &App, summary: &ScanSummary) -> Result<Option<TranslatorInstall>, String> {
    let desired = desired_active_mod_keys(summary, &app.config().state_dir)
        .map_err(|error| error.to_string())?;
    let mut found = None::<TranslatorInstall>;
    for record in summary.clone().into_records() {
        let manifest_path = record
            .path
            .join(format!("{}.json", translator_link::TRANSLATOR_MOD_ID));
        let Ok(text) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
        else {
            continue;
        };
        if value.get("id").and_then(serde_json::Value::as_str)
            != Some(translator_link::TRANSLATOR_MOD_ID)
        {
            continue;
        }
        let install = TranslatorInstall {
            root: translator_link::translations_root(&record.path),
            version: value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            active: desired.contains(&record.stable_key()),
        };
        // Several copies (Workshop + mods folder): the one the game loads wins.
        if found.as_ref().is_none_or(|current| !current.active && install.active) {
            found = Some(install);
        }
    }
    Ok(found)
}

fn translator_target_for_sheet(
    app: &App,
    summary: &ScanSummary,
    sheet: &JsonTranslationSheet,
) -> Option<TranslatorTarget> {
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
    let mod_id = if manifest.is_translation_patch {
        manifest.target_mod_id.clone()
    } else {
        manifest.id.clone()
    }?;
    let mod_version = if manifest.is_translation_patch {
        manifest.target_mod_version
    } else {
        manifest.version
    };
    Some(TranslatorTarget {
        mod_key,
        mod_id,
        mod_version,
        language: sheet.target_language.clone(),
    })
}

fn save_target_decision(sheet_path: &Path) -> Result<SaveTargetDecision, String> {
    use translator_link::SaveTargetMode;

    let app = app();
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    let (tables, unsupported) = translator_link::sheet_override_tables(&sheet);
    let supported_entries = tables.values().map(BTreeMap::len).sum::<usize>();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let translator = detect_translator(&app, &summary)?;
    let mod_key = mod_key_for_translation_sheet(&sheet);
    let mode = mod_key
        .as_deref()
        .map(|key| translator_link::read_save_target_mode(&app.config().state_dir, key))
        .unwrap_or(SaveTargetMode::Auto);
    let mut decision = SaveTargetDecision {
        mode,
        translator,
        target: None,
        reason: String::new(),
        supported_entries,
        unsupported,
    };
    let patch = |mut decision: SaveTargetDecision, reason: String| {
        decision.reason = reason;
        Ok(decision)
    };

    if is_hardcoded_source_file(Path::new(&sheet.source_path)) {
        return patch(decision, "DLL 문자열 시트는 Translator가 지원하지 않습니다".to_string());
    }
    if mode == SaveTargetMode::Patch {
        return patch(decision, "이 모드는 패치 모드로 저장하도록 설정되어 있습니다".to_string());
    }
    let Some((version, active)) = decision
        .translator
        .as_ref()
        .map(|install| (install.version.clone(), install.active))
    else {
        return patch(decision, "Sts2ModTranslator가 설치되어 있지 않습니다".to_string());
    };
    if !translator_link::version_supported(&version) {
        return patch(
            decision,
            format!("Sts2ModTranslator {version} 은(는) 지원하지 않는 버전입니다 (1.19.0 이상 필요)"),
        );
    }
    if !active && mode == SaveTargetMode::Auto {
        return patch(decision, "Sts2ModTranslator가 비활성 상태입니다".to_string());
    }
    if supported_entries == 0 {
        return patch(
            decision,
            "시트에 Translator로 옮길 수 있는 localization 항목이 없습니다".to_string(),
        );
    }
    let Some(target) = translator_target_for_sheet(&app, &summary, &sheet) else {
        return patch(decision, "대상 모드의 manifest id를 찾지 못했습니다".to_string());
    };
    decision.reason = if active {
        format!("Sts2ModTranslator {version}에 저장합니다")
    } else {
        "Sts2ModTranslator가 비활성 상태라 게임에 반영되지 않습니다".to_string()
    };
    decision.target = Some(target);
    Ok(decision)
}

pub(crate) fn translation_save_target(sheet_path: String) -> Result<TranslationSaveTargetDto, String> {
    let decision = save_target_decision(Path::new(sheet_path.trim()))?;
    Ok(save_target_dto(&decision))
}

pub(crate) fn set_translation_save_mode(sheet_path: String, mode: String) -> Result<TranslationSaveTargetDto, String> {
    use translator_link::SaveTargetMode;

    let sheet_path = PathBuf::from(sheet_path.trim());
    let sheet = read_sheet(&sheet_path).map_err(|error| error.to_string())?;
    let mod_key = mod_key_for_translation_sheet(&sheet)
        .ok_or_else(|| "이 시트가 어느 모드의 것인지 찾지 못했습니다.".to_string())?;
    let mode = match mode.as_str() {
        "auto" => SaveTargetMode::Auto,
        "patch" => SaveTargetMode::Patch,
        "translator" => SaveTargetMode::Translator,
        other => return Err(format!("알 수 없는 저장 위치입니다: {other}")),
    };
    translator_link::write_save_target_mode(&app().config().state_dir, &mod_key, mode)
        .map_err(|error| error.to_string())?;
    translation_save_target(sheet_path.display().to_string())
}

fn save_target_dto(decision: &SaveTargetDecision) -> TranslationSaveTargetDto {
    use translator_link::SaveTargetMode;

    TranslationSaveTargetDto {
        destination: if decision.target.is_some() { "translator" } else { "patch" }.to_string(),
        mode: match decision.mode {
            SaveTargetMode::Auto => "auto",
            SaveTargetMode::Patch => "patch",
            SaveTargetMode::Translator => "translator",
        }
        .to_string(),
        reason: decision.reason.clone(),
        translator_version: decision.translator.as_ref().map(|install| install.version.clone()),
        translator_active: decision.translator.as_ref().is_some_and(|install| install.active),
        supported_entries: decision.supported_entries,
        unsupported_entries: decision.unsupported.total(),
    }
}

/// Saves the sheet into Translator overrides when the per-mod decision says
/// so; `Ok(None)` means the caller should build a patch mod as before.
fn save_sheet_to_translator(sheet_path: &Path) -> Result<Option<JsonApplyDto>, String> {
    let decision = save_target_decision(sheet_path)?;
    let (Some(target), Some(install)) = (decision.target.as_ref(), decision.translator.as_ref())
    else {
        if decision.mode == translator_link::SaveTargetMode::Translator {
            return Err(format!("Translator에 저장할 수 없습니다: {}", decision.reason));
        }
        return Ok(None);
    };

    let app = app();
    let config = app.config();
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    let (ours, unsupported) = translator_link::sheet_override_tables(&sheet);
    let base_path = translator_link::base_path(&config.state_dir, &target.mod_key, &target.language);
    let root_text = install.root.display().to_string();
    // A base recorded for another Translator copy or mod id says nothing
    // about the files we are about to read.
    let previous = translator_link::read_base(&base_path)
        .map_err(|error| error.to_string())?
        .filter(|base| base.mod_id == target.mod_id && base.translator_root == root_text);
    let previous_tables = previous.as_ref().map(|base| base.tables.clone()).unwrap_or_default();
    let theirs = translator_link::read_override_tables(
        &install.root,
        &target.mod_id,
        &target.language,
        ours.keys().cloned(),
    )
    .map_err(|error| error.to_string())?;
    let plan = translator_link::plan_sync(&ours, &theirs, &previous_tables);
    let overrides_dir =
        translator_link::overrides_dir(&install.root, &target.mod_id, &target.language);

    let mut result = TranslatorSaveDto {
        mod_id: target.mod_id.clone(),
        language: target.language.clone(),
        overrides_dir: display_path(&overrides_dir),
        translator_version: install.version.clone(),
        translator_active: install.active,
        written_entries: 0,
        unchanged_entries: plan.unchanged,
        kept_in_game_entries: plan.kept_in_game.values().map(BTreeMap::len).sum(),
        unsupported_entries: unsupported.total(),
        unsupported_hardcoded: unsupported.hardcoded,
        conflicts: plan
            .conflicts
            .iter()
            .map(|conflict| TranslatorConflictDto {
                table: conflict.table.clone(),
                key: conflict.key.clone(),
                ours: conflict.ours.clone(),
                theirs: conflict.theirs.clone(),
            })
            .collect(),
        disabled_patch_mods: Vec::new(),
        game_running: app.launch_status().running,
    };
    // Conflicts mean the player translated something differently in game;
    // writing anything now could bury that, so nothing is written.
    if !plan.conflicts.is_empty() {
        return Ok(Some(translator_apply_dto(&overrides_dir, 0, result)));
    }

    let report = translator_link::apply_writes(
        &install.root,
        &target.mod_id,
        &target.language,
        &plan.writes,
    )
    .map_err(|error| error.to_string())?;
    if let Some(version) = target.mod_version.as_deref() {
        translator_link::record_target_version(&install.root, &target.mod_id, version)
            .map_err(|error| error.to_string())?;
    }
    result.written_entries = report.written_entries;

    let patch_mods_disabled = previous.as_ref().is_some_and(|base| base.patch_mods_disabled);
    if !patch_mods_disabled {
        result.disabled_patch_mods = disable_translation_patch_mods(&app, &target.mod_id)?;
    }
    translator_link::write_base(
        &base_path,
        &translator_link::SyncBase {
            mod_id: target.mod_id.clone(),
            translator_root: root_text,
            patch_mods_disabled: true,
            tables: translator_link::next_base(&ours, &previous_tables, &plan),
        },
    )
    .map_err(|error| error.to_string())?;

    let translated_entries = ours
        .values()
        .flat_map(BTreeMap::values)
        .filter(|value| !value.is_empty())
        .count();
    record_translation_apply(
        sheet_path,
        &PckPatchReport {
            language_output_path: overrides_dir.clone(),
            packed_pck_path: None,
            installed_mod_path: Some(overrides_dir.clone()),
            applied_entries: translated_entries,
        },
        config,
    )?;
    Ok(Some(translator_apply_dto(&overrides_dir, translated_entries, result)))
}

/// Turns off active translation patch mods for `mod_id` the first time the
/// mod is saved to Translator; both would apply, but managing two copies of
/// the same translation is how they drift apart.
fn disable_translation_patch_mods(app: &App, mod_id: &str) -> Result<Vec<String>, String> {
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let desired = desired_active_mod_keys(&summary, &app.config().state_dir)
        .map_err(|error| error.to_string())?;
    let mut disabled = Vec::new();
    let mut changes = Vec::new();
    for record in summary.into_records() {
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
    Ok(disabled)
}

fn translator_apply_dto(
    overrides_dir: &Path,
    applied_entries: usize,
    translator: TranslatorSaveDto,
) -> JsonApplyDto {
    JsonApplyDto {
        output_path: display_path(overrides_dir),
        applied_entries,
        language_output_path: display_path(overrides_dir),
        packed_pck_path: String::new(),
        installed_mod_path: display_path(overrides_dir),
        translator: Some(translator),
    }
}
