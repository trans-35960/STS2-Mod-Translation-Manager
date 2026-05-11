fn mod_rows(
    report: &sts2_mod_manager::domain::ScanReport,
    config: &AppConfig,
    deleted_keys: &BTreeSet<String>,
    game_updated_epoch: Option<u64>,
) -> sts2_mod_manager::error::AppResult<Vec<ModRowDto>> {
    let mut rows = BTreeMap::<String, ModRowBuilder>::new();

    for record in &report.summary.game_mods {
        rows.entry(record.stable_key()).or_default().active = Some(record.clone());
    }
    for record in &report.summary.vault_mods {
        rows.entry(record.stable_key()).or_default().vault = Some(record.clone());
    }
    for record in &report.summary.external_manager_mods {
        rows.entry(record.stable_key()).or_default().external = Some(record.clone());
    }

    let mut cache = read_language_preview_cache(config)?;
    let mut current_cache_keys = BTreeSet::new();
    let state_index = read_mod_state_index(&config.mod_index_path)?;
    let desired_active_keys = desired_active_mod_keys(&report.summary, &config.state_dir)?;
    let lifecycle = mod_lifecycle_by_key(&state_index);
    let translation_applies = read_translation_apply_index(config)?;
    let mut rows = rows
        .into_iter()
        .filter(|(key, _)| !deleted_keys.contains(key))
        .map(|(key, builder)| {
            let desired_active = desired_active_keys.contains(&key);
            let (update_state, change_reasons) = builder.change_summary(&state_index);
            let lifecycle = lifecycle.get(&key);
            let translation_apply = translation_applies.get(&key);
            builder.into_dto(
                key,
                update_state,
                change_reasons,
                lifecycle.map(|entry| entry.registered_epoch),
                lifecycle.map(|entry| entry.updated_epoch),
                translation_apply,
                game_updated_epoch,
                desired_active,
                &mut cache,
                &mut current_cache_keys,
                &config.vendor_dir,
            )
        })
        .collect::<Vec<_>>();
    resolve_mod_dependencies(&mut rows);
    resolve_translation_patch_summaries(&mut rows);

    cache
        .entries
        .retain(|key, _| current_cache_keys.contains(key));
    if cache.dirty {
        write_language_preview_cache(config, &cache)?;
    }

    Ok(rows)
}

fn mod_lifecycle_by_key(
    state_index: &std::collections::HashMap<String, ModStateEntry>,
) -> BTreeMap<String, ModLifecycleSummary> {
    let mut output = BTreeMap::<String, ModLifecycleSummary>::new();
    for entry in state_index.values() {
        let existing = output
            .entry(entry.key.clone())
            .or_insert_with(|| lifecycle_summary(entry));
        existing.registered_epoch = existing.registered_epoch.min(entry.registered_epoch);
        existing.updated_epoch = existing.updated_epoch.max(entry.updated_epoch);
    }
    output
}

fn lifecycle_summary(entry: &ModStateEntry) -> ModLifecycleSummary {
    ModLifecycleSummary {
        registered_epoch: entry.registered_epoch,
        updated_epoch: entry.updated_epoch,
    }
}

#[derive(Default)]
struct ModRowBuilder {
    active: Option<ModRecord>,
    vault: Option<ModRecord>,
    external: Option<ModRecord>,
}

impl ModRowBuilder {
    fn into_dto(
        self,
        key: String,
        update_state: Option<String>,
        change_reasons: Vec<String>,
        registered_epoch: Option<u64>,
        updated_epoch: Option<u64>,
        translation_apply: Option<&TranslationApplyRecord>,
        game_updated_epoch: Option<u64>,
        desired_active: bool,
        cache: &mut LanguagePreviewCache,
        current_cache_keys: &mut BTreeSet<String>,
        vendor_dir: &Path,
    ) -> ModRowDto {
        let record = self
            .active
            .as_ref()
            .or(self.vault.as_ref())
            .or(self.external.as_ref())
            .expect("row has at least one source");
        let sources = source_labels(&self);
        let extraction_source = extraction_source_for_record(record);
        let cache_key = language_cache_key(record, &extraction_source, vendor_dir);
        let language_preview = cached_language_preview(
            &cache_key,
            &extraction_source,
            cache,
            current_cache_keys,
            vendor_dir,
        );
        let extraction_tree = extraction_tree(&extraction_source, &cache_key, vendor_dir);
        let translation = translation_state(&extraction_source, &language_preview);
        let scan_root = extraction_scan_root(&extraction_source, &cache_key, vendor_dir)
            .unwrap_or_else(|| extraction_source.clone());
        let manifest = read_mod_manifest_for_record(&record.path, &scan_root);
        let manifest_id = manifest.id.clone();
        let group_name = manifest.name.clone();
        let is_translation_patch = manifest.is_translation_patch
            || manifest.target_mod_id.is_some()
            || manifest.target_mod_name.is_some()
            || !manifest.target_languages.is_empty();
        let version_hint = if is_translation_patch {
            manifest
                .target_mod_version
                .clone()
                .or_else(|| manifest.version.clone())
                .or_else(|| record.version_hint.clone())
        } else {
            manifest
                .version
                .clone()
                .or_else(|| record.version_hint.clone())
        };
        let safety_warnings = mod_safety_warnings(&manifest);
        let mut dependencies = manifest
            .dependencies
            .into_iter()
            .map(|dependency| ModDependencyDto {
                name: dependency.id.clone(),
                id: dependency.id,
                key: None,
                active: false,
                available: false,
                version_required: dependency.version,
                version_current: None,
                version_matches: None,
            })
            .collect::<Vec<_>>();
        if let Some(target_id) = manifest.target_mod_id.clone() {
            if !dependencies
                .iter()
                .any(|dependency| dependency.id.eq_ignore_ascii_case(&target_id))
            {
                dependencies.push(ModDependencyDto {
                    name: manifest
                        .target_mod_name
                        .clone()
                        .unwrap_or_else(|| target_id.clone()),
                    id: target_id,
                    key: None,
                    active: false,
                    available: false,
                    version_required: manifest.target_mod_version.clone(),
                    version_current: None,
                    version_matches: None,
                });
            }
        }
        let latest_mod_epoch = updated_epoch.or_else(|| epoch_seconds(record.fingerprint.modified));
        let needs_recheck = desired_active
            && game_updated_epoch
                .zip(latest_mod_epoch)
                .is_some_and(|(game_epoch, mod_epoch)| game_epoch > mod_epoch);
        let translation_review_required = translation_apply
            .and_then(|apply| game_updated_epoch.map(|game_epoch| game_epoch > apply.applied_epoch))
            .unwrap_or(false);

        ModRowDto {
            key,
            name: record.name.clone(),
            manifest_id,
            group_name,
            active: desired_active,
            managed: self.vault.is_some(),
            external: self.external.is_some(),
            source_label: sources,
            kind: kind_label(record.kind).to_string(),
            version_hint,
            bytes: record.fingerprint.bytes,
            modified_epoch: epoch_seconds(record.fingerprint.modified),
            registered_epoch,
            updated_epoch,
            path: display_path(&record.path),
            update_state: update_state.unwrap_or_else(|| "clean".to_string()),
            change_reasons,
            translation_state: translation.0,
            translation_applied: translation_apply.is_some(),
            translation_applied_epoch: translation_apply.map(|record| record.applied_epoch),
            translation_patch_count: 0,
            translation_patch_active_count: 0,
            translation_patch_names: Vec::new(),
            needs_recheck,
            translation_review_required,
            safety_warnings,
            extraction_hint: translation.1,
            extraction_source_path: display_path(&extraction_source),
            extraction_target: extraction_target(&extraction_source, &language_preview),
            is_translation_patch,
            translation_target_id: manifest.target_mod_id,
            translation_target_key: None,
            translation_target_name: manifest.target_mod_name,
            translation_target_version: manifest.target_mod_version,
            dependencies,
            language_preview,
            extraction_tree,
        }
    }

    fn change_summary(
        &self,
        state_index: &std::collections::HashMap<String, ModStateEntry>,
    ) -> (Option<String>, Vec<String>) {
        let records = [
            self.active.as_ref(),
            self.vault.as_ref(),
            self.external.as_ref(),
        ];
        let mut state = None;
        let mut reasons = BTreeSet::new();
        for record in records.into_iter().flatten() {
            let key = mod_record_state_key(record);
            let Some(previous) = state_index.get(&key) else {
                state = Some("new".to_string());
                reasons.insert("새로 감지".to_string());
                continue;
            };
            let current_modified = epoch_seconds(record.fingerprint.modified);
            let bytes_changed = record.fingerprint.bytes != previous.bytes;
            let modified_changed = current_modified != previous.modified_epoch;
            if bytes_changed || modified_changed {
                if state.as_deref() != Some("new") {
                    state = Some("updated".to_string());
                }
                if bytes_changed {
                    reasons.insert("파일 크기".to_string());
                }
                if modified_changed {
                    reasons.insert("수정일".to_string());
                }
            }
        }
        (state, reasons.into_iter().collect())
    }
}

fn resolve_mod_dependencies(rows: &mut [ModRowDto]) {
    let index = rows
        .iter()
        .map(|row| {
            (
                row.key.clone(),
                row.name.clone(),
                row.manifest_id.clone(),
                row.group_name.clone(),
                row.active,
                row.version_hint.clone(),
                dependency_match_tokens(
                    &row.key,
                    &row.name,
                    row.manifest_id.as_deref(),
                    row.group_name.as_deref(),
                ),
            )
        })
        .collect::<Vec<_>>();
    for row in rows {
        for dependency in &mut row.dependencies {
            let needle = normalize_dependency_token(&dependency.id);
            let Some((key, name, _manifest_id, group_name, active, version, _)) = index.iter().find(|(key, name, _, _, _, _, tokens)| {
                key != &row.key
                    && (tokens.iter().any(|token| token == &needle)
                        || tokens.iter().any(|token| token.starts_with(&needle))
                        || normalize_dependency_token(name) == needle)
            }) else {
                dependency.available = false;
                dependency.active = false;
                continue;
            };
            dependency.key = Some(key.clone());
            dependency.name = group_name.clone().unwrap_or_else(|| name.clone());
            dependency.active = *active;
            dependency.available = true;
            dependency.version_current = version.clone();
            dependency.version_matches = dependency
                .version_required
                .as_ref()
                .map(|required| dependency_version_matches(required, version.as_deref()));
            if row
                .translation_target_id
                .as_deref()
                .is_some_and(|target_id| normalize_dependency_token(target_id) == needle)
            {
                row.translation_target_key = Some(key.clone());
                if row.translation_target_name.is_none() {
                    row.translation_target_name = Some(name.clone());
                }
            }
        }
    }
}

fn resolve_translation_patch_summaries(rows: &mut [ModRowDto]) {
    let patches = rows
        .iter()
        .filter(|row| row.is_translation_patch)
        .filter_map(|row| {
            let target_key = row
                .translation_target_key
                .clone()
                .or_else(|| {
                    row.dependencies
                        .iter()
                        .find_map(|dependency| dependency.key.clone())
                })?;
            Some((target_key, row.name.clone(), row.active))
        })
        .collect::<Vec<_>>();

    for row in rows {
        if row.is_translation_patch {
            continue;
        }
        let mut names = Vec::new();
        let mut active_count = 0usize;
        for (target_key, patch_name, patch_active) in &patches {
            if target_key != &row.key {
                continue;
            }
            names.push(patch_name.clone());
            if *patch_active {
                active_count += 1;
            }
        }
        names.sort();
        names.dedup();
        row.translation_patch_count = names.len();
        row.translation_patch_active_count = active_count.min(row.translation_patch_count);
        row.translation_patch_names = names;
    }
}

fn dependency_version_matches(required: &str, current: Option<&str>) -> bool {
    let required = normalize_version_token(required);
    if required.is_empty() || required == "-" {
        return true;
    }
    let current = current.map(normalize_version_token).unwrap_or_default();
    !current.is_empty() && current == required
}

fn normalize_version_token(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_ascii_lowercase()
}

fn dependency_match_tokens(
    key: &str,
    name: &str,
    manifest_id: Option<&str>,
    group_name: Option<&str>,
) -> Vec<String> {
    let mut tokens = vec![
        normalize_dependency_token(key),
        normalize_dependency_token(name),
    ];
    if let Some(manifest_id) = manifest_id {
        tokens.push(normalize_dependency_token(manifest_id));
    }
    if let Some(group_name) = group_name {
        tokens.push(normalize_dependency_token(group_name));
    }
    if let Some(prefix) = key.split(['-', '_', ' ']).next() {
        tokens.push(normalize_dependency_token(prefix));
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn normalize_dependency_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}


