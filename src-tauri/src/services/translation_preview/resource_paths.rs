fn selected_translation_files(scan_root: &Path, resource_path: &str) -> Vec<PathBuf> {
    let normalized = normalize_resource_path(resource_path);
    let Ok(candidates) = scan_translation_candidates(scan_root) else {
        return selected_hardcoded_files(scan_root, resource_path);
    };
    let mut selected = candidates
        .iter()
        .filter_map(|candidate| {
            let display = normalize_resource_path(&resource_display_path(&candidate.path));
            resource_path_matches_selection(&display, &normalized).then_some(candidate.path.clone())
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected = selected_translation_files_from_filesystem_path(scan_root, resource_path, &candidates);
    }
    if selected.is_empty() {
        selected = selected_hardcoded_files(scan_root, resource_path);
    }
    selected.sort();
    selected.dedup();
    selected
}

fn selected_translation_files_from_filesystem_path(
    scan_root: &Path,
    resource_path: &str,
    candidates: &[sts2_mod_manager::domain::TranslationCandidate],
) -> Vec<PathBuf> {
    let trimmed = resource_path.trim();
    if trimmed.is_empty() || trimmed.starts_with("res://") {
        return Vec::new();
    }

    let raw_path = PathBuf::from(trimmed);
    let mut selections = vec![raw_path.clone()];
    if raw_path.is_relative() {
        selections.push(scan_root.join(&raw_path));
    }

    let normalized_selections = selections
        .into_iter()
        .map(|path| normalize_resource_path(&path.to_string_lossy()))
        .collect::<Vec<_>>();

    candidates
        .iter()
        .filter_map(|candidate| {
            let candidate_path = normalize_resource_path(&candidate.path.to_string_lossy());
            normalized_selections
                .iter()
                .any(|selection| path_is_same_or_child(&candidate_path, selection))
                .then_some(candidate.path.clone())
        })
        .collect()
}

fn resource_path_matches_selection(display: &str, selection: &str) -> bool {
    if selection == "res://" {
        return true;
    }
    if path_is_same_or_child(display, selection) {
        return true;
    }
    let display_relative = display.strip_prefix("res://").unwrap_or(display);
    let selection_relative = selection.strip_prefix("res://").unwrap_or(selection);
    if selection_relative.is_empty() {
        return false;
    }
    if path_is_same_or_child(display_relative, selection_relative) {
        return true;
    }
    let scoped_selection = format!("/{selection_relative}");
    display_relative
        .find(&scoped_selection)
        .map(|index| {
            let rest = &display_relative[index + scoped_selection.len()..];
            rest.is_empty() || rest.starts_with('/')
        })
        .unwrap_or(false)
}

fn path_is_same_or_child(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .map(|rest| rest.starts_with('/'))
            .unwrap_or(false)
}

fn copy_existing_target_language_files(
    scan_root: &Path,
    source_files: &[PathBuf],
    translated_root: &Path,
    target_language: &str,
) -> Result<usize, String> {
    let mut copied = 0;
    for source_file in source_files {
        let relative = pck_resource_relative_path(source_file)
            .or_else(|| {
                source_file
                    .strip_prefix(scan_root)
                    .ok()
                    .map(Path::to_path_buf)
            })
            .unwrap_or_else(|| {
                source_file
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("translation.json"))
            });
        let Some(target_relative) = replace_resource_language(&relative, target_language) else {
            continue;
        };
        if target_relative == relative {
            continue;
        }
        let original_target = scan_root.join(&target_relative);
        if !original_target.is_file() {
            continue;
        }
        let workspace_target = translated_root.join(&target_relative);
        if let Some(parent) = workspace_target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&original_target, &workspace_target).map_err(|error| error.to_string())?;
        copied += 1;
    }
    Ok(copied)
}

fn default_translation_resource_path(scan_root: &Path, _target_language: &str) -> Option<String> {
    let mut by_dir = BTreeMap::<PathBuf, (usize, usize)>::new();
    for candidate in scan_translation_candidates(scan_root).ok()? {
        if is_hardcoded_source_file(&candidate.path) {
            continue;
        }
        let Some(dir) = language_dir_for_candidate(&candidate.path) else {
            continue;
        };
        let entry = by_dir.entry(dir).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += count_json_translation_keys(&candidate.path);
    }
    by_dir
        .into_iter()
        .max_by(
            |(left_path, (left_files, left_keys)), (right_path, (right_files, right_keys))| {
                left_keys
                    .cmp(right_keys)
                    .then_with(|| left_files.cmp(right_files))
                    .then_with(|| {
                        resource_display_path(right_path).cmp(&resource_display_path(left_path))
                    })
            },
        )
        .map(|(path, _)| resource_display_path(&path))
}

fn default_hardcoded_resource_path(scan_root: &Path) -> Option<String> {
    let mut files = Vec::new();
    collect_hardcoded_files(scan_root, &mut files);
    files
        .into_iter()
        .filter(|path| hardcoded_file_has_strings(path))
        .map(|path| resource_display_path(&path))
        .next()
}

fn selected_hardcoded_files(scan_root: &Path, resource_path: &str) -> Vec<PathBuf> {
    let normalized = normalize_resource_path(resource_path);
    let mut files = Vec::new();
    collect_hardcoded_files(scan_root, &mut files);
    files
        .into_iter()
        .filter(|file| {
            let display = normalize_resource_path(&resource_display_path(file));
            resource_path_matches_selection(&display, &normalized)
                || selected_hardcoded_file_from_filesystem_path(scan_root, resource_path, file)
        })
        .filter(|path| hardcoded_file_has_strings(path))
        .collect()
}

fn selected_hardcoded_file_from_filesystem_path(
    scan_root: &Path,
    resource_path: &str,
    candidate: &Path,
) -> bool {
    let trimmed = resource_path.trim();
    if trimmed.is_empty() || trimmed.starts_with("res://") {
        return false;
    }
    let raw_path = PathBuf::from(trimmed);
    let mut selections = vec![raw_path.clone()];
    if raw_path.is_relative() {
        selections.push(scan_root.join(&raw_path));
    }
    let candidate_path = normalize_resource_path(&candidate.to_string_lossy());
    selections.into_iter().any(|selection| {
        let selection = normalize_resource_path(&selection.to_string_lossy());
        path_is_same_or_child(&candidate_path, &selection)
    })
}

fn collect_hardcoded_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(root) else {
        return;
    };
    if metadata.is_file() {
        if is_hardcoded_source_file(root) {
            files.push(root.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        collect_hardcoded_files(&entry, files);
    }
}

fn hardcoded_file_has_strings(path: &Path) -> bool {
    flatten_hardcoded_values(path)
        .map(|values| {
            values
                .values()
                .any(|value| looks_like_translatable_hardcoded_value(value))
        })
        .unwrap_or(false)
}

fn looks_like_translatable_hardcoded_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.chars().count() >= 4
        && trimmed.chars().any(char::is_alphabetic)
        && trimmed.chars().any(|character| character.is_whitespace() || ".:,;!?".contains(character))
}

fn language_dir_for_candidate(path: &Path) -> Option<PathBuf> {
    let mut ancestors = path.ancestors();
    if path.is_file() {
        let _ = ancestors.next();
    }
    for ancestor in ancestors {
        if localization_language_component(ancestor).is_some() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn fallback_translation_memory_sheet(
    translation_work_dir: &Path,
    mod_key: &str,
    target_language: &str,
    resource_path: &str,
) -> Option<PathBuf> {
    let memory_dir = translation_work_dir
        .join("translation_memory")
        .join(mod_key);
    let suffix = format!(".{target_language}.translation.json");
    let mut candidates = fs::read_dir(memory_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().ends_with(&suffix))
                .unwrap_or(false)
        })
        .filter(|path| translation_sheet_matches_resource(path, resource_path))
        .filter_map(|path| {
            read_sheet(&path)
                .ok()
                .filter(|sheet| {
                    sheet
                        .entries
                        .iter()
                        .any(|entry| !entry.translated_value.is_empty())
                })
                .map(|sheet| (sheet.updated_epoch, path))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop().map(|(_, path)| path)
}

fn select_translation_memory_sheet(
    translation_work_dir: &Path,
    exact_sheet: Option<PathBuf>,
    mod_keys: &[String],
    target_language: &str,
    resource_path: &str,
) -> Option<PathBuf> {
    if exact_sheet
        .as_deref()
        .is_some_and(translation_sheet_has_translations)
    {
        return exact_sheet;
    }
    for mod_key in mod_keys {
        let Some(candidate) = fallback_translation_memory_sheet(
            translation_work_dir,
            mod_key,
            target_language,
            resource_path,
        ) else {
            continue;
        };
        if exact_sheet.as_ref().is_some_and(|exact| exact == &candidate) {
            continue;
        }
        if translation_sheet_has_translations(&candidate) {
            return Some(candidate);
        }
    }
    exact_sheet
}

fn translation_sheet_has_translations(sheet_path: &Path) -> bool {
    read_sheet(sheet_path)
        .map(|sheet| {
            sheet
                .entries
                .iter()
                .any(|entry| !entry.translated_value.is_empty())
        })
        .unwrap_or(false)
}

fn translation_memory_candidate_keys(
    app: &App,
    record: &ModRecord,
    manifest: &ModManifestInfo,
) -> Vec<String> {
    let current_key = record.stable_key();
    let current_tokens = translation_memory_family_tokens(record, manifest);
    let mut keys = vec![current_key.clone()];
    let Ok(summary) = app.scan_preview_report().map(|report| report.summary) else {
        return keys;
    };
    for candidate in summary
        .game_mods
        .into_iter()
        .chain(summary.vault_mods)
        .chain(summary.external_manager_mods)
    {
        let candidate_key = candidate.stable_key();
        if candidate_key == current_key || keys.iter().any(|key| key == &candidate_key) {
            continue;
        }
        let candidate_source = extraction_source_for_record(&candidate);
        let candidate_cache_key =
            language_cache_key(&candidate, &candidate_source, &app.config().vendor_dir);
        let candidate_scan_root =
            extraction_scan_root(&candidate_source, &candidate_cache_key, &app.config().vendor_dir)
                .unwrap_or(candidate_source);
        let candidate_manifest = read_mod_manifest_for_record(&candidate.path, &candidate_scan_root);
        let candidate_tokens = translation_memory_family_tokens(&candidate, &candidate_manifest);
        if related_translation_memory_tokens(&current_tokens, &candidate_tokens) {
            keys.push(candidate_key);
        }
    }
    keys
}

fn translation_memory_family_tokens(
    record: &ModRecord,
    manifest: &ModManifestInfo,
) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    add_translation_memory_token(&mut tokens, &record.stable_key());
    add_translation_memory_token(&mut tokens, &record.name);
    if let Some(prefix) = record.stable_key().split(['-', '_', ' ']).next() {
        add_translation_memory_token(&mut tokens, prefix);
    }
    if let Some(id) = manifest.id.as_deref() {
        add_translation_memory_token(&mut tokens, id);
    }
    if let Some(name) = manifest.name.as_deref() {
        add_translation_memory_token(&mut tokens, name);
    }
    tokens
}

fn add_translation_memory_token(tokens: &mut BTreeSet<String>, value: &str) {
    let token = normalize_dependency_token(value);
    let char_count = token.chars().count();
    let numeric_only = token.chars().all(|character| character.is_ascii_digit());
    if char_count >= 2 && !numeric_only {
        tokens.insert(token);
    }
}

fn related_translation_memory_tokens(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
) -> bool {
    left.iter().any(|left_token| {
        right.iter().any(|right_token| {
            left_token == right_token
                || left_token.starts_with(right_token)
                || right_token.starts_with(left_token)
        })
    })
}

fn translation_sheet_matches_resource(sheet_path: &Path, resource_path: &str) -> bool {
    let Ok(sheet) = read_sheet(sheet_path) else {
        return false;
    };
    let source_path = PathBuf::from(sheet.source_path);
    let Some(mut relative) = relative_after_named_component(&source_path, "source") else {
        return false;
    };
    if relative
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "json" | "loc"))
        .unwrap_or(false)
    {
        relative.pop();
    }
    resource_paths_match(&format!("res://{}", slash_path(&relative)), resource_path)
}

fn resource_paths_match(left: &str, right: &str) -> bool {
    let left_normalized = normalize_resource_path(left);
    let right_normalized = normalize_resource_path(right);
    if left_normalized == right_normalized {
        return true;
    }
    let left_localization = localization_resource_tail(&left_normalized);
    let right_localization = localization_resource_tail(&right_normalized);
    left_localization
        .zip(right_localization)
        .is_some_and(|(left_tail, right_tail)| left_tail == right_tail)
}

fn localization_resource_tail(path: &str) -> Option<String> {
    let parts = path
        .trim_start_matches("res://")
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let localization = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    Some(parts[localization..].join("/"))
}

fn normalize_resource_path(path: &str) -> String {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized == "res://" {
        normalized
    } else {
        normalized.trim_end_matches('/').to_string()
    }
}

fn pck_resource_roots(scan_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    collect_pck_resource_roots(scan_root, &mut roots);
    roots.sort();
    roots
}

fn collect_pck_resource_roots(path: &Path, roots: &mut Vec<PathBuf>) {
    if !path.is_dir() {
        return;
    }
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.ends_with(".pck.contents"))
        .unwrap_or(false)
    {
        roots.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_pck_resource_roots(&entry.path(), roots);
    }
}

fn pck_resource_relative_path(path: &Path) -> Option<PathBuf> {
    let components = path
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();
    let root_index = components.iter().position(|component| {
        component
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".pck.contents")
    })?;
    if root_index + 1 >= components.len() {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in components.iter().skip(root_index + 1) {
        relative.push(component);
    }
    Some(relative)
}

fn resource_path_to_relative(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if let Some(rest) = trimmed.strip_prefix("res://") {
        return Some(PathBuf::from(rest.trim_start_matches('/')));
    }
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn localization_language_component(path: &Path) -> Option<String> {
    let parts = path_components(path);
    let localization = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    parts.get(localization + 1).cloned()
}

fn replace_resource_language(path: &Path, language: &str) -> Option<PathBuf> {
    let mut parts = path_components(path);
    let localization = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    if localization + 1 >= parts.len() {
        return None;
    }
    parts[localization + 1] = language.to_string();
    Some(parts.iter().collect())
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect()
}

fn pck_contents_root_for_path(path: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    let mut found = false;
    for component in path.components() {
        root.push(component.as_os_str());
        if component
            .as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".pck.contents")
        {
            found = true;
            break;
        }
    }
    found.then_some(root)
}

fn resource_display_path(path: &Path) -> String {
    pck_resource_relative_path(path)
        .map(|relative| format!("res://{}", slash_path(&relative)))
        .unwrap_or_else(|| display_path(path))
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

