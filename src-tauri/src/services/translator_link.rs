/// Auto sync after a save, an explicit apply and a restore can run at the
/// same time; each reads, merges and writes the same override and base files.
fn lock_translator_sync() -> std::sync::MutexGuard<'static, ()> {
    static TRANSLATOR_SYNC_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TRANSLATOR_SYNC_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

fn detect_translator(
    config: &AppConfig,
    summary: &ScanSummary,
) -> Result<Option<TranslatorInstall>, String> {
    let desired = desired_active_mod_keys(summary, &config.state_dir)
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

fn save_target_decision(sheet: &JsonTranslationSheet) -> Result<SaveTargetDecision, String> {
    use translator_link::SaveTargetMode;

    let app = app();
    let (tables, unsupported) = translator_link::sheet_override_tables(sheet);
    let supported_entries = tables.values().map(BTreeMap::len).sum::<usize>();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let translator = detect_translator(app.config(), &summary)?;
    let mod_key = mod_key_for_translation_sheet(sheet);
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
    let Some(target) = translator_target_for_sheet(&app, &summary, sheet) else {
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
    let sheet = read_sheet(Path::new(sheet_path.trim())).map_err(|error| error.to_string())?;
    let decision = save_target_decision(&sheet)?;
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
        auto_sync: translator_link::auto_sync_on_save(&app().config().state_dir),
        translator_active: decision.translator.as_ref().is_some_and(|install| install.active),
        supported_entries: decision.supported_entries,
        unsupported_entries: decision.unsupported.total(),
    }
}

/// Everything one Translator sync needs, read but not yet written.
struct PreparedSync {
    target: TranslatorTarget,
    install: TranslatorInstall,
    ours: translator_link::OverrideTables,
    theirs: translator_link::OverrideTables,
    unsupported: translator_link::UnsupportedEntries,
    previous: Option<translator_link::SyncBase>,
    previous_tables: translator_link::OverrideTables,
    base_path: PathBuf,
}

/// `Ok(None)` when the sheet does not go to Translator (the caller builds a
/// patch mod as before); an error when Translator was forced but cannot be used.
fn prepare_translator_sync(sheet: &JsonTranslationSheet) -> Result<Option<PreparedSync>, String> {
    let SaveTargetDecision {
        mode,
        translator,
        target,
        reason,
        unsupported,
        ..
    } = save_target_decision(sheet)?;
    let (Some(target), Some(install)) = (target, translator) else {
        if mode == translator_link::SaveTargetMode::Translator {
            return Err(format!("Translator에 저장할 수 없습니다: {reason}"));
        }
        return Ok(None);
    };
    let (ours, _) = translator_link::sheet_override_tables(sheet);
    let base_path =
        translator_link::base_path(&app().config().state_dir, &target.mod_key, &target.language);
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
    Ok(Some(PreparedSync {
        target,
        install,
        ours,
        theirs,
        unsupported,
        previous,
        previous_tables,
        base_path,
    }))
}

/// Lists what a save would not write on its own: values changed in game
/// since the last save, and keys changed on both sides. Uses the sheet the
/// editor holds, so unsaved edits count.
pub(crate) fn translator_sync_preview(
    sheet: JsonSheetDto,
) -> Result<TranslatorSyncPreviewDto, String> {
    let sheet = json_sheet_from_dto(sheet)?;
    let Some(prepared) = prepare_translator_sync(&sheet)? else {
        return Ok(TranslatorSyncPreviewDto {
            available: false,
            mod_id: String::new(),
            language: sheet.target_language.clone(),
            pending_writes: 0,
            items: Vec::new(),
        });
    };
    let plan =
        translator_link::plan_sync(&prepared.ours, &prepared.theirs, &prepared.previous_tables);
    let sheet_keys = translator_link::sheet_override_keys(&sheet);
    let keys_of = |table: &str, key: &str| {
        sheet_keys
            .get(&(table.to_string(), key.to_string()))
            .cloned()
            .unwrap_or_default()
    };
    let ours_of = |table: &str, key: &str| {
        prepared
            .ours
            .get(table)
            .and_then(|values| values.get(key))
            .cloned()
            .unwrap_or_default()
    };
    let mut items = Vec::new();
    for (table, values) in &plan.kept_in_game {
        for (key, theirs) in values {
            items.push(TranslatorSyncItemDto {
                kind: "game_changed".to_string(),
                sheet_keys: keys_of(table, key),
                table: table.clone(),
                key: key.clone(),
                ours: ours_of(table, key),
                theirs: theirs.clone(),
            });
        }
    }
    for conflict in &plan.conflicts {
        items.push(TranslatorSyncItemDto {
            kind: "conflict".to_string(),
            sheet_keys: keys_of(&conflict.table, &conflict.key),
            table: conflict.table.clone(),
            key: conflict.key.clone(),
            ours: conflict.ours.clone(),
            theirs: conflict.theirs.clone(),
        });
    }
    Ok(TranslatorSyncPreviewDto {
        available: true,
        mod_id: prepared.target.mod_id,
        language: prepared.target.language,
        pending_writes: plan.writes.values().map(BTreeMap::len).sum(),
        items,
    })
}

/// Saves the sheet into Translator overrides when the per-mod decision says
/// so; `Ok(None)` means the caller should build a patch mod as before.
/// `keep_sheet_keys` are sheet keys whose sheet value should replace what
/// the player changed in game (the user chose the sheet side).
fn save_sheet_to_translator(
    sheet_path: &Path,
    keep_sheet_keys: &[String],
) -> Result<Option<JsonApplyDto>, String> {
    let _guard = lock_translator_sync();
    let sheet = read_sheet(sheet_path).map_err(|error| error.to_string())?;
    let Some(prepared) = prepare_translator_sync(&sheet)? else {
        return Ok(None);
    };
    let PreparedSync {
        target,
        install,
        ours,
        theirs,
        unsupported,
        previous,
        mut previous_tables,
        base_path,
    } = prepared;
    // Choosing the sheet side means "the game value is known and replaced":
    // treating it as the base turns the key into a plain write.
    let keep = keep_sheet_keys.iter().collect::<BTreeSet<_>>();
    let kept_keys = translator_link::sheet_override_keys(&sheet)
        .into_iter()
        .filter(|(_, sheet_keys)| sheet_keys.iter().any(|sheet_key| keep.contains(sheet_key)))
        .map(|(location, _)| location)
        .collect::<Vec<_>>();
    translator_link::prefer_sheet_values(&mut previous_tables, &theirs, &kept_keys);

    let app = app();
    let config = app.config();
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
            translator_root: install.root.display().to_string(),
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

/// Saves the sheet to Translator after the editor saved it, when auto sync
/// is on. `Ok(None)` when it is off or the sheet goes to a patch mod.
pub(crate) fn translator_auto_sync(sheet_path: String) -> Result<Option<TranslatorSaveDto>, String> {
    if !translator_link::auto_sync_on_save(&app().config().state_dir) {
        return Ok(None);
    }
    Ok(save_sheet_to_translator(Path::new(sheet_path.trim()), &[])?
        .and_then(|result| result.translator))
}

pub(crate) fn set_translator_auto_sync(enabled: bool) -> Result<bool, String> {
    translator_link::set_auto_sync_on_save(&app().config().state_dir, enabled)
        .map_err(|error| error.to_string())?;
    Ok(enabled)
}

/// Translator sync state of every mod this app has saved to Translator.
fn resolve_translator_links(rows: &mut [ModRowDto], config: &AppConfig, summary: &ScanSummary) {
    let mut install = None::<Option<TranslatorInstall>>;
    for row in rows.iter_mut() {
        let Ok(bases) = translator_link::read_bases(&config.state_dir, &row.key) else {
            continue;
        };
        if bases.is_empty() {
            continue;
        }
        let install = install
            .get_or_insert_with(|| detect_translator(config, summary).ok().flatten())
            .as_ref();
        row.translator_links = bases
            .into_iter()
            .map(|(language, base)| {
                let health = install.and_then(|install| {
                    translator_link::link_health(&install.root, &base, &language).ok()
                });
                TranslatorLinkDto {
                    mod_id: base.mod_id,
                    language,
                    translator_installed: install.is_some(),
                    translator_active: install.is_some_and(|install| install.active),
                    missing_entries: health.map(|health| health.missing_entries).unwrap_or(0),
                    game_changed_entries: health
                        .map(|health| health.game_changed_entries)
                        .unwrap_or(0),
                }
            })
            .collect();
    }
}

/// Writes back what was last synced for `keys` where it is missing from
/// Translator, e.g. after a Workshop update replaced its folder.
pub(crate) fn restore_translator_overrides(keys: Vec<String>) -> Result<ActionDto, String> {
    let app = app();
    let config = app.config();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let install = detect_translator(config, &summary)?
        .ok_or_else(|| "Sts2ModTranslator가 설치되어 있지 않아 복구할 수 없습니다.".to_string())?;
    if !translator_link::version_supported(&install.version) {
        return Err(format!(
            "Sts2ModTranslator {} 은(는) 지원하지 않는 버전입니다 (1.19.0 이상 필요)",
            install.version
        ));
    }
    let _guard = lock_translator_sync();
    let mut restored = Vec::new();
    for key in &keys {
        for (language, mut base) in
            translator_link::read_bases(&config.state_dir, key).map_err(|error| error.to_string())?
        {
            let report = translator_link::restore_from_base(&install.root, &base, &language)
                .map_err(|error| error.to_string())?;
            if report.written_entries > 0 {
                restored.push(format!("{}/{} {}개", base.mod_id, language, report.written_entries));
            }
            // The files now live under the current Translator copy.
            let root_text = install.root.display().to_string();
            if base.translator_root != root_text {
                base.translator_root = root_text;
                translator_link::write_base(
                    &translator_link::base_path(&config.state_dir, key, &language),
                    &base,
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(ActionDto {
        message: if restored.is_empty() {
            "Translator에서 다시 쓸 번역이 없습니다.".to_string()
        } else {
            format!("Translator 번역 복구: {}", restored.join(", "))
        },
        dashboard: dashboard().map_err(|error| error.to_string())?,
    })
}
