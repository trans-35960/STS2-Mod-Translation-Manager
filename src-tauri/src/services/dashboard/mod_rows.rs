fn mod_rows(
    report: &sts2_mod_manager::domain::ScanReport,
    config: &AppConfig,
    deleted_keys: &BTreeSet<String>,
    game_updated_epoch: Option<u64>,
) -> sts2_mod_manager::error::AppResult<Vec<ModRowDto>> {
    let mut trace = PerfTrace::new("mod_rows");
    let mut rows = BTreeMap::<String, ModRowBuilder>::new();

    for record in &report.summary.game_mods {
        rows.entry(record.stable_key()).or_default().active = Some(record.clone());
    }
    for record in &report.summary.disabled_mods {
        rows.entry(record.stable_key()).or_default().disabled = Some(record.clone());
    }
    for record in &report.summary.external_manager_mods {
        rows.entry(record.stable_key()).or_default().external = Some(record.clone());
    }
    for record in &report.summary.workshop_mods {
        rows.entry(record.stable_key()).or_default().workshop = Some(record.clone());
    }
    trace.mark("group_records");

    let mut cache = read_language_preview_cache(config)?;
    trace.mark("read_language_cache");
    let mut current_cache_keys = BTreeSet::new();
    let change_baseline = session_change_baseline(&report.summary, &config.mod_index_path)?;
    let state_index = read_mod_state_index(&config.mod_index_path)?;
    trace.mark("read_state_index");
    let desired_active_keys = desired_active_mod_keys(&report.summary, &config.state_dir)?;
    trace.mark("read_desired_active");
    let lifecycle = mod_lifecycle_by_key(&state_index);
    let translation_applies = read_translation_apply_index(config)?;
    trace.mark("read_translation_apply");
    let mut rows = rows
        .into_iter()
        .filter(|(key, _)| !deleted_keys.contains(key))
        .map(|(key, builder)| {
            let row_started = Instant::now();
            let desired_active = desired_active_keys.contains(&key);
            let (update_state, change_reasons, previous_version) =
                builder.change_summary(&change_baseline);
            let lifecycle = lifecycle.get(&key);
            let translation_apply = translation_applies.get(&key);
            let dto = builder.into_dto(ModRowBuildContext {
                key,
                update_state,
                change_reasons,
                previous_version,
                registered_epoch: lifecycle.map(|entry| entry.registered_epoch),
                updated_epoch: lifecycle.map(|entry| entry.updated_epoch),
                translation_apply,
                game_updated_epoch,
                desired_active,
                cache: &mut cache,
                current_cache_keys: &mut current_cache_keys,
                vendor_dir: &config.vendor_dir,
            });
            let elapsed = row_started.elapsed().as_millis();
            if elapsed >= 30 || env::var_os("STS2_PERF_VERBOSE").is_some() {
                append_performance_log(format!(
                    "mod_row total={}ms key={} name={} active={} bytes={} state={} path={}",
                    elapsed,
                    dto.key,
                    dto.name,
                    dto.active,
                    dto.bytes,
                    dto.translation_state,
                    dto.path
                ));
            }
            dto
        })
        .collect::<Vec<_>>();
    trace.mark("build_rows");
    resolve_mod_dependencies(&mut rows);
    trace.mark("resolve_dependencies");
    resolve_translation_patch_summaries(&mut rows);
    trace.mark("resolve_translation_patches");
    resolve_branch_issues(&mut rows, config);
    trace.mark("resolve_branch_issues");
    resolve_bridge_edit_counts(&mut rows, config);
    trace.mark("resolve_bridge_edits");
    resolve_bridge_stored(&mut rows);
    trace.mark("resolve_bridge_stored");

    cache
        .entries
        .retain(|key, _| current_cache_keys.contains(key));
    if cache.dirty {
        write_language_preview_cache(config, &cache)?;
    }
    trace.mark("write_language_cache");
    trace.finish(format!("rows={}", rows.len()), 0);

    Ok(rows)
}

/// Mod index as the previous run of the app left it, per index path, kept for
/// the whole session.
static CHANGE_BASELINES: std::sync::Mutex<BTreeMap<PathBuf, std::collections::HashMap<String, ModStateEntry>>> =
    std::sync::Mutex::new(BTreeMap::new());

/// What the mod list compares against to flag changes. A mod that changed
/// since the previous run stays marked for this session and is clean on the
/// next start: the current scan is recorded right away as that run's
/// baseline. With no index yet (first start) nothing is flagged.
fn session_change_baseline(
    summary: &ScanSummary,
    index_path: &Path,
) -> sts2_mod_manager::error::AppResult<std::collections::HashMap<String, ModStateEntry>> {
    let mut baselines = CHANGE_BASELINES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let first_start = !index_path.exists();
    if !baselines.contains_key(index_path) && !first_start {
        baselines.insert(index_path.to_path_buf(), read_mod_state_index(index_path)?);
    }
    if let Err(error) = write_state(summary, index_path) {
        eprintln!("mod index write skipped: {error}");
    }
    if first_start {
        baselines.insert(index_path.to_path_buf(), read_mod_state_index(index_path)?);
    }
    Ok(baselines.get(index_path).cloned().unwrap_or_default())
}

/// Marks mods with in-game edits or reports, so the list shows them without
/// opening the translation tool.
fn resolve_bridge_edit_counts(rows: &mut [ModRowDto], config: &AppConfig) {
    let Some(shared) = bridge::shared_root().filter(|shared| shared.is_dir()) else {
        return;
    };
    let mut counts = BTreeMap::<String, bridge_edits::PendingCounts>::new();
    for row in rows.iter_mut().filter(|row| !row.is_translation_patch) {
        let Some(mod_id) = row.manifest_id.clone() else {
            continue;
        };
        let pending = counts
            .entry(mod_id.clone())
            .or_insert_with(|| bridge_edits::pending_counts(&shared, &config.state_dir, &mod_id));
        if !pending.is_empty() {
            row.bridge_edits = Some(BridgeEditCountsDto {
                edits: pending.edits,
                reports: pending.reports,
                dll_edits: pending.dll_edits,
                dll_reports: pending.dll_reports,
                dll_assembly: pending.dll_assemblies.iter().next().cloned(),
            });
        }
    }
}

/// Marks mods whose translations the bridge holds, so the list shows which
/// mods are translated in game without a patch mod.
fn resolve_bridge_stored(rows: &mut [ModRowDto]) {
    let Some(shared) = bridge::shared_root().filter(|shared| shared.is_dir()) else {
        return;
    };
    let mut stored = BTreeMap::<String, Vec<BridgeStoredDto>>::new();
    for row in rows.iter_mut().filter(|row| !row.is_translation_patch && !row.system_mod) {
        let Some(mod_id) = row.manifest_id.clone() else {
            continue;
        };
        row.bridge_stored = stored
            .entry(mod_id.clone())
            .or_insert_with(|| {
                bridge::stored_translations(&shared, &mod_id)
                    .into_iter()
                    .map(|stored| BridgeStoredDto {
                        language: stored.language,
                        entries: stored.entries,
                        dll_strings: stored.dll_strings,
                    })
                    .collect()
            })
            .clone();
    }
}

/// Flags Workshop mods the game will refuse to load on the current Steam
/// branch, using the same snapshot ranges the game checks.
fn resolve_branch_issues(rows: &mut [ModRowDto], config: &AppConfig) {
    if !rows.iter().any(|row| row.workshop) {
        return;
    }
    let Some(game) = read_game_branch(&config.game_dir) else {
        return;
    };
    let items = read_workshop_branches(&config.workshop_dirs);
    for row in rows.iter_mut().filter(|row| row.workshop) {
        let Some(item) = row
            .workshop_item_id
            .as_deref()
            .and_then(|item_id| items.get(item_id))
        else {
            continue;
        };
        row.branch_issue = branch_issue_dto(branch_support(item, &game.branch), &game.branch);
    }
}

fn branch_issue_dto(support: BranchSupport, current_branch: &str) -> Option<BranchIssueDto> {
    let (kind, branch_min, branch_max) = match support {
        BranchSupport::Supported | BranchSupport::Unknown => return None,
        BranchSupport::Unsupported {
            branch_min,
            branch_max,
        } => ("unsupported", branch_min, branch_max),
        BranchSupport::AwaitingDownload {
            branch_min,
            branch_max,
        } => ("awaiting_download", branch_min, branch_max),
    };
    let range = branch_range_label(&branch_min, &branch_max);
    let message = if kind == "unsupported" {
        format!("현재 브랜치({current_branch})를 지원하지 않아 게임이 불러오지 않습니다. 지원: {range}")
    } else {
        format!(
            "설치된 버전은 {range}용입니다. 현재 브랜치({current_branch})용 버전을 Steam이 아직 받지 않았습니다."
        )
    };
    Some(BranchIssueDto {
        kind: kind.to_string(),
        current_branch: current_branch.to_string(),
        branch_min,
        branch_max,
        message,
    })
}

fn branch_range_label(branch_min: &str, branch_max: &str) -> String {
    match (branch_min.trim(), branch_max.trim()) {
        ("", "") => "모든 브랜치".to_string(),
        (min, max) if min.eq_ignore_ascii_case(max) => min.to_string(),
        ("", max) => max.to_string(),
        (min, "") => format!("{min} 이상"),
        (min, max) => format!("{min} ~ {max}"),
    }
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
    disabled: Option<ModRecord>,
    external: Option<ModRecord>,
    workshop: Option<ModRecord>,
}

struct ModRowBuildContext<'a> {
    key: String,
    update_state: Option<String>,
    change_reasons: Vec<String>,
    previous_version: Option<String>,
    registered_epoch: Option<u64>,
    updated_epoch: Option<u64>,
    translation_apply: Option<&'a TranslationApplyRecord>,
    game_updated_epoch: Option<u64>,
    desired_active: bool,
    cache: &'a mut LanguagePreviewCache,
    current_cache_keys: &'a mut BTreeSet<String>,
    vendor_dir: &'a Path,
}

impl ModRowBuilder {
    fn into_dto(self, context: ModRowBuildContext<'_>) -> ModRowDto {
        let record = self
            .active
            .as_ref()
            .or(self.disabled.as_ref())
            .or(self.external.as_ref())
            .or(self.workshop.as_ref())
            .expect("row has at least one source");
        let sources = source_labels(&self);
        let extraction_source = extraction_source_for_record(record);
        let download_state = download_state_for_record(record);
        let is_downloading = download_state.as_deref() == Some("downloading");
        let cache_key = language_cache_key(record, &extraction_source, context.vendor_dir);
        let (language_preview, extraction_tree, translation, manifest) = if is_downloading {
            (
                Vec::new(),
                Vec::new(),
                (
                    "다운로드 중".to_string(),
                    "Vortex 다운로드가 완료되면 다시 인식됩니다".to_string(),
                ),
                ModManifestInfo::default(),
            )
        } else {
            let language_preview = cached_language_preview(
                &cache_key,
                &extraction_source,
                context.cache,
                context.current_cache_keys,
                context.vendor_dir,
            );
            let extraction_tree =
                extraction_tree(&extraction_source, &cache_key, context.vendor_dir);
            let translation = translation_state(&extraction_source, &language_preview);
            let scan_root = if source_has_pck_payload(&extraction_source) {
                extraction_source.clone()
            } else {
                extraction_scan_root(&extraction_source, &cache_key, context.vendor_dir)
                    .unwrap_or_else(|| extraction_source.clone())
            };
            let manifest = read_mod_manifest_for_record(&record.path, &scan_root);
            (language_preview, extraction_tree, translation, manifest)
        };
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
        if let Some(target_id) = manifest.target_mod_id.clone()
            && !dependencies
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
        let latest_mod_epoch = context
            .updated_epoch
            .or_else(|| epoch_seconds(record.fingerprint.modified));
        let needs_recheck = context.desired_active
            && context
                .game_updated_epoch
                .zip(latest_mod_epoch)
                .is_some_and(|(game_epoch, mod_epoch)| game_epoch > mod_epoch);
        // A new mod version can rename or add text keys, so an applied translation
        // needs another look just like after a game update.
        let translation_review_required = context.translation_apply.is_some_and(|apply| {
            context.previous_version.is_some()
                || context
                    .game_updated_epoch
                    .is_some_and(|game_epoch| game_epoch > apply.applied_epoch)
        });

        let system_mod = manifest_id.as_deref() == Some(bridge::BRIDGE_MOD_ID);
        ModRowDto {
            key: context.key,
            name: record.name.clone(),
            manifest_id,
            group_name,
            active: context.desired_active,
            managed: self.disabled.is_some(),
            external: self.external.is_some(),
            workshop: self.workshop.is_some(),
            workshop_item_id: record
                .workshop_item_id()
                .or_else(|| self.workshop.as_ref().and_then(ModRecord::workshop_item_id)),
            source_label: sources,
            kind: kind_label(record.kind).to_string(),
            version_hint,
            bytes: record.fingerprint.bytes,
            modified_epoch: epoch_seconds(record.fingerprint.modified),
            registered_epoch: context.registered_epoch,
            updated_epoch: context.updated_epoch,
            path: display_path(&record.path),
            download_state,
            update_state: context.update_state.unwrap_or_else(|| "clean".to_string()),
            change_reasons: context.change_reasons,
            previous_version: context.previous_version,
            translation_state: translation.0,
            translation_applied: context.translation_apply.is_some(),
            translation_applied_epoch: context.translation_apply.map(|record| record.applied_epoch),
            translation_patch_count: 0,
            translation_patch_active_count: 0,
            translation_patch_names: Vec::new(),
            bridge_stored: Vec::new(),
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
            branch_issue: None,
            system_mod,
            bridge_edits: None,
            language_preview,
            extraction_tree,
        }
    }

    fn change_summary(
        &self,
        state_index: &std::collections::HashMap<String, ModStateEntry>,
    ) -> (Option<String>, Vec<String>, Option<String>) {
        let records = [
            self.active.as_ref(),
            self.disabled.as_ref(),
            self.external.as_ref(),
            self.workshop.as_ref(),
        ];
        let mut state = None;
        let mut reasons = BTreeSet::new();
        let mut previous_version = None;
        for record in records.into_iter().flatten() {
            let key = mod_record_state_key(record);
            let Some(previous) = state_index.get(&key) else {
                state = Some("new".to_string());
                reasons.insert("새로 감지".to_string());
                continue;
            };
            if let Some((from, to)) = version_change(record, previous) {
                if state.as_deref() != Some("new") {
                    state = Some("updated".to_string());
                }
                reasons.insert(format!("버전 {from} → {to}"));
                previous_version = Some(from);
            }
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
        (state, reasons.into_iter().collect(), previous_version)
    }
}

struct DependencyCandidate {
    key: String,
    name: String,
    manifest_token: Option<String>,
    group_name: Option<String>,
    active: bool,
    is_translation_patch: bool,
    version: Option<String>,
    tokens: Vec<String>,
}

impl DependencyCandidate {
    fn from_row(row: &ModRowDto) -> Self {
        Self {
            key: row.key.clone(),
            name: row.name.clone(),
            manifest_token: row.manifest_id.as_deref().map(normalize_dependency_token),
            group_name: row.group_name.clone(),
            active: row.active,
            is_translation_patch: row.is_translation_patch,
            version: row.version_hint.clone(),
            tokens: dependency_match_tokens(
                &row.key,
                &row.name,
                row.manifest_id.as_deref(),
                row.group_name.as_deref(),
            ),
        }
    }

    /// How well this row stands in for a dependency id: 2 for an exact
    /// identifier match, 1 for the id followed only by a version suffix
    /// (`baselib` vs `baselibv330`), `None` when it is a different mod.
    fn match_strength(&self, needle: &str) -> Option<u8> {
        if needle.is_empty() {
            return None;
        }
        // A translation patch must never satisfy its own target's id just
        // because its folder name starts with it (`MaidenSuccubus_tr`).
        if self.is_translation_patch {
            return (self.manifest_token.as_deref() == Some(needle)).then_some(2);
        }
        if self.tokens.iter().any(|token| token == needle) {
            return Some(2);
        }
        self.tokens
            .iter()
            .any(|token| {
                token
                    .strip_prefix(needle)
                    .is_some_and(is_version_suffix_token)
            })
            .then_some(1)
    }
}

fn resolve_mod_dependencies(rows: &mut [ModRowDto]) {
    let index = rows
        .iter()
        .map(DependencyCandidate::from_row)
        .collect::<Vec<_>>();
    for row in rows {
        for dependency in &mut row.dependencies {
            let needle = normalize_dependency_token(&dependency.id);
            // Several installed versions of the same mod can all match; an
            // active one satisfies the dependency, so prefer it over the
            // first row that happens to come up in key order.
            let Some(candidate) = index
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.key != row.key)
                .filter_map(|(position, candidate)| {
                    let strength = candidate.match_strength(&needle)?;
                    let version_ok = dependency.version_required.as_deref().is_none_or(
                        |required| dependency_version_matches(required, candidate.version.as_deref()),
                    );
                    Some((
                        (candidate.active, version_ok, strength, std::cmp::Reverse(position)),
                        candidate,
                    ))
                })
                .max_by(|left, right| left.0.cmp(&right.0))
                .map(|(_, candidate)| candidate)
            else {
                dependency.available = false;
                dependency.active = false;
                continue;
            };
            dependency.key = Some(candidate.key.clone());
            dependency.name = candidate
                .group_name
                .clone()
                .unwrap_or_else(|| candidate.name.clone());
            dependency.active = candidate.active;
            dependency.available = true;
            dependency.version_current = candidate.version.clone();
            dependency.version_matches = dependency
                .version_required
                .as_ref()
                .map(|required| dependency_version_matches(required, candidate.version.as_deref()));
            if row
                .translation_target_id
                .as_deref()
                .is_some_and(|target_id| normalize_dependency_token(target_id) == needle)
            {
                row.translation_target_key = Some(candidate.key.clone());
                if row.translation_target_name.is_none() {
                    row.translation_target_name = Some(candidate.name.clone());
                }
            }
        }
    }
}

fn download_state_for_record(record: &ModRecord) -> Option<String> {
    if record.source != ModSource::ExternalManager
        || record.kind != ModKind::Archive
        || !is_vortex_download_path(&record.path)
    {
        return None;
    }
    if record.fingerprint.bytes == 0
        || has_vortex_download_marker(&record.path)
        || record_recently_modified(record, 180)
    {
        return Some("downloading".to_string());
    }
    None
}

fn is_vortex_download_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    normalized.contains("\\vortex\\downloads\\")
}

fn has_vortex_download_marker(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name == file_name {
            return false;
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        (name.starts_with(&file_name) || name.starts_with(&stem))
            && matches!(
                extension.as_str(),
                "part" | "partial" | "tmp" | "download" | "crdownload"
            )
    })
}

fn record_recently_modified(record: &ModRecord, seconds: u64) -> bool {
    let Some(modified) = record.fingerprint.modified else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|elapsed| elapsed <= std::time::Duration::from_secs(seconds))
        .unwrap_or(false)
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
    tokens.retain(|token| !token.is_empty());
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Whether what follows a dependency id inside a longer token is only a
/// version (`v330`, `330workshop`), so `BaseLib-v3.3.0` still satisfies
/// `BaseLib` while `BaseLibExtra` or `MaidenSuccubus_tr` does not.
fn is_version_suffix_token(rest: &str) -> bool {
    let mut characters = rest.chars();
    match characters.next() {
        Some(first) if first.is_ascii_digit() => true,
        Some('v') => characters.next().is_some_and(|next| next.is_ascii_digit()),
        _ => false,
    }
}

fn normalize_dependency_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}


