#[derive(Clone, Default)]
struct ModManifestInfo {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    description: Option<String>,
    dependencies: Vec<ManifestDependency>,
    target_mod_id: Option<String>,
    target_mod_name: Option<String>,
    target_mod_version: Option<String>,
    target_languages: Vec<String>,
    is_translation_patch: bool,
}

#[derive(Clone, Default)]
struct ManifestDependency {
    id: String,
    version: Option<String>,
}

fn read_mod_manifest_info(scan_root: &Path) -> ModManifestInfo {
    let mut manifests = Vec::new();
    collect_manifest_candidates(scan_root, scan_root, &mut manifests);
    manifests.sort();
    for path in manifests {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let json = strip_json_comments(content.trim_start_matches('\u{feff}'));
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
            continue;
        };
        let info = ModManifestInfo {
            id: string_field(&value, &["id", "mod_id"]),
            name: string_field(&value, &["name", "mod_name", "id"]),
            version: string_field(&value, &["version", "mod_version"]),
            author: string_field(&value, &["author", "creator"]),
            description: string_field(&value, &["description", "desc"]),
            dependencies: dependency_list_field(&value, &["dependencies", "deps", "requires"]),
            target_mod_id: string_field(&value, &["target_mod_id", "target_id", "source_mod_id"]),
            target_mod_name: string_field(&value, &["target_mod_name", "target_name", "source_mod_name"]),
            target_mod_version: string_field(&value, &["target_mod_version", "target_version", "source_mod_version"]),
            target_languages: string_list_field(&value, &["target_languages", "languages"]),
            is_translation_patch: bool_field(&value, &["is_translation_patch", "translation_mod", "is_translation_mod"]),
        };
        if info.id.is_some()
            || info.name.is_some()
            || info.version.is_some()
            || !info.dependencies.is_empty()
            || info.target_mod_id.is_some()
            || info.target_mod_name.is_some()
            || info.is_translation_patch
        {
            return with_dependency_versions(info, &value);
        }
    }
    ModManifestInfo::default()
}

fn read_mod_manifest_for_record(record_path: &Path, scan_root: &Path) -> ModManifestInfo {
    let scan_manifest = read_mod_manifest_info(scan_root);
    if record_path == scan_root {
        return scan_manifest;
    }
    let record_manifest = read_mod_manifest_info(record_path);
    if is_translation_patch_manifest(&record_manifest) {
        merge_manifest_info(record_manifest, scan_manifest)
    } else {
        merge_manifest_info(scan_manifest, record_manifest)
    }
}

fn merge_manifest_info(mut primary: ModManifestInfo, fallback: ModManifestInfo) -> ModManifestInfo {
    primary.id = primary.id.or(fallback.id);
    primary.name = primary.name.or(fallback.name);
    primary.version = primary.version.or(fallback.version);
    primary.author = primary.author.or(fallback.author);
    primary.description = primary.description.or(fallback.description);
    primary.target_mod_id = primary.target_mod_id.or(fallback.target_mod_id);
    primary.target_mod_name = primary.target_mod_name.or(fallback.target_mod_name);
    primary.target_mod_version = primary.target_mod_version.or(fallback.target_mod_version);
    primary.is_translation_patch |= fallback.is_translation_patch;
    if primary.target_languages.is_empty() {
        primary.target_languages = fallback.target_languages;
    }
    for dependency in fallback.dependencies {
        if !primary
            .dependencies
            .iter()
            .any(|existing| existing.id.eq_ignore_ascii_case(&dependency.id))
        {
            primary.dependencies.push(dependency);
        }
    }
    primary
}

fn strip_json_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(character) = chars.next() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }

        if character == '/' {
            match chars.peek().copied() {
                Some('/') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut previous = '\0';
                    for next in chars.by_ref() {
                        if previous == '*' && next == '/' {
                            break;
                        }
                        previous = next;
                    }
                    continue;
                }
                _ => {}
            }
        }

        output.push(character);
    }

    output
}

fn collect_manifest_candidates(root: &Path, path: &Path, output: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        let is_json = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if is_json
            && !path_inside_pck_contents(path)
            && path
                .strip_prefix(root)
                .map(|relative| relative.components().count() <= 3)
                .unwrap_or(false)
        {
            output.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_manifest_candidates(root, &entry.path(), output);
    }
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn string_list_field(value: &serde_json::Value, keys: &[&str]) -> Vec<String> {
    let Some(field) = keys.iter().find_map(|key| value.get(*key)) else {
        return Vec::new();
    };
    let mut values = match field {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .or_else(|| item.get("id").and_then(serde_json::Value::as_str))
                    .or_else(|| item.get("name").and_then(serde_json::Value::as_str))
            })
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        serde_json::Value::String(text) => text
            .split([',', ';'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    values.sort();
    values.dedup();
    values
}

fn bool_field(value: &serde_json::Value, keys: &[&str]) -> bool {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn dependency_list_field(value: &serde_json::Value, keys: &[&str]) -> Vec<ManifestDependency> {
    let ids = string_list_field(value, keys);
    let mut dependencies = ids
        .into_iter()
        .map(|id| ManifestDependency { id, version: None })
        .collect::<Vec<_>>();
    let Some(field) = keys.iter().find_map(|key| value.get(*key)) else {
        return dependencies;
    };
    if let serde_json::Value::Array(items) = field {
        dependencies = items
            .iter()
            .filter_map(|item| {
                let id = item
                    .as_str()
                    .or_else(|| item.get("id").and_then(serde_json::Value::as_str))
                    .or_else(|| item.get("name").and_then(serde_json::Value::as_str))?
                    .trim()
                    .to_string();
                if id.is_empty() {
                    return None;
                }
                Some(ManifestDependency {
                    id,
                    version: string_field(item, &["version", "required_version", "target_version"]),
                })
            })
            .collect();
    }
    dependencies.sort_by(|left, right| left.id.cmp(&right.id));
    dependencies.dedup_by(|left, right| left.id.eq_ignore_ascii_case(&right.id));
    dependencies
}

fn with_dependency_versions(
    mut info: ModManifestInfo,
    value: &serde_json::Value,
) -> ModManifestInfo {
    if let Some(versions) = value.get("dependency_versions").and_then(serde_json::Value::as_object)
    {
        for dependency in &mut info.dependencies {
            if dependency.version.is_some() {
                continue;
            }
            dependency.version = versions
                .get(&dependency.id)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
        }
    }
    if let (Some(target_id), Some(target_version)) = (
        info.target_mod_id.as_deref(),
        info.target_mod_version.as_ref(),
    ) && let Some(dependency) = info
            .dependencies
            .iter_mut()
            .find(|dependency| dependency.id.eq_ignore_ascii_case(target_id))
        && dependency.version.is_none()
    {
        dependency.version = Some(target_version.clone());
    }
    info
}

