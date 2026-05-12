fn extraction_scan_root(source: &Path, cache_key: &str, vendor_dir: &Path) -> Option<PathBuf> {
    if source.is_dir() {
        if directory_contains_pck(source) {
            let destination = language_preview_extract_dir(cache_key);
            if !directory_preview_has_pck_contents(&destination) {
                let _ = fs::remove_dir_all(&destination);
            }
            if !destination.exists() && !expand_directory_preview(source, &destination, vendor_dir)
            {
                return None;
            }
            compact_language_preview_cache(&destination);
            return Some(destination);
        }
        return Some(source.to_path_buf());
    }
    if is_supported_extractable_path(source) {
        let destination = language_preview_extract_dir(cache_key);
        if !destination.exists() && !expand_source(source, &destination, vendor_dir) {
            return None;
        }
        compact_language_preview_cache(&destination);
        return Some(destination);
    }
    None
}

fn full_extraction_scan_root(source: &Path, cache_key: &str, vendor_dir: &Path) -> Option<PathBuf> {
    if source.is_dir() {
        if directory_contains_pck(source) {
            let destination = language_preview_extract_dir(&format!("{cache_key}-full"));
            if !directory_preview_has_pck_contents(&destination) {
                let _ = fs::remove_dir_all(&destination);
            }
            if !destination.exists() && !expand_directory_preview(source, &destination, vendor_dir)
            {
                return None;
            }
            return Some(destination);
        }
        return Some(source.to_path_buf());
    }
    if is_supported_extractable_path(source) {
        let destination = language_preview_extract_dir(&format!("{cache_key}-full"));
        if !destination.exists() && !expand_source(source, &destination, vendor_dir) {
            return None;
        }
        return Some(destination);
    }
    None
}

fn directory_contains_pck(source: &Path) -> bool {
    let mut pcks = Vec::new();
    collect_pck_files(source, &mut pcks);
    !pcks.is_empty()
}

fn directory_preview_has_pck_contents(destination: &Path) -> bool {
    if !destination.exists() {
        return false;
    }
    let mut stack = vec![destination.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let child = entry.path();
            if child
                .file_name()
                .and_then(|value| value.to_str())
                .map(|name| name.ends_with(".pck.contents"))
                .unwrap_or(false)
            {
                return true;
            }
            if child.is_dir() {
                stack.push(child);
            }
        }
    }
    false
}

fn expand_directory_preview(source: &Path, destination: &Path, vendor_dir: &Path) -> bool {
    if fs::create_dir_all(destination).is_err() {
        return false;
    }

    let mut pcks = Vec::new();
    collect_pck_files(source, &mut pcks);
    pcks.sort();
    let mut expanded_any = false;
    for pck in pcks {
        let relative = pck.strip_prefix(source).unwrap_or(&pck);
        let pck_destination = destination.join(relative).with_extension("pck.contents");
        if expand_pck(&pck, &pck_destination, vendor_dir) {
            expanded_any = true;
        }
    }
    let mut hardcoded_files = Vec::new();
    collect_hardcoded_files(source, &mut hardcoded_files);
    for file in hardcoded_files {
        let relative = file.strip_prefix(source).unwrap_or(&file);
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&file, target).is_ok() {
            expanded_any = true;
        }
    }
    expanded_any
}

fn compact_language_preview_cache(root: &Path) {
    let candidates = scan_translation_candidates(root).unwrap_or_default();
    let mut keep_files = candidates
        .into_iter()
        .map(|candidate| candidate.path)
        .collect::<BTreeSet<_>>();
    let mut hardcoded_files = Vec::new();
    collect_hardcoded_files(root, &mut hardcoded_files);
    keep_files.extend(
        hardcoded_files
            .into_iter()
            .filter(|path| hardcoded_file_has_strings(path)),
    );
    collect_preview_metadata_files(root, root, &mut keep_files);
    remove_unkept_preview_files(root, &keep_files);
    remove_empty_preview_dirs(root, root);
}

fn collect_preview_metadata_files(path: &Path, root: &Path, keep_files: &mut BTreeSet<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if is_preview_metadata_file(path, root) {
            keep_files.insert(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_preview_metadata_files(&entry.path(), root, keep_files);
    }
}

fn is_preview_metadata_file(path: &Path, root: &Path) -> bool {
    if path_inside_pck_contents(path) {
        return false;
    }
    let is_json = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if !is_json {
        return false;
    }
    path.strip_prefix(root)
        .map(|relative| relative.components().count() <= 3)
        .unwrap_or(false)
}

fn path_inside_pck_contents(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".pck.contents")
    })
}

fn remove_unkept_preview_files(path: &Path, keep_files: &BTreeSet<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if !keep_files.contains(path) {
            let _ = remove_preview_file(path);
        }
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        remove_unkept_preview_files(&entry.path(), keep_files);
    }
}

fn remove_preview_file(path: &Path) -> std::io::Result<()> {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
    fs::remove_file(path)
}

fn remove_empty_preview_dirs(path: &Path, root: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.is_file() {
        return false;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    for entry in entries.filter_map(Result::ok) {
        let child = entry.path();
        if remove_empty_preview_dirs(&child, root) {
            let _ = fs::remove_dir(&child);
        }
    }
    path != root
        && fs::read_dir(path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
}


fn extraction_source_for_record(record: &ModRecord) -> PathBuf {
    if record.path.is_dir() {
        if directory_contains_translation_candidates(&record.path) {
            return record.path.clone();
        }
        if directory_contains_hardcoded_candidates(&record.path) {
            return record.path.clone();
        }
        preferred_extractable_payload(&record.path)
            .or_else(|| first_active_payload(&record.path))
            .unwrap_or_else(|| record.path.clone())
    } else {
        record.path.clone()
    }
}

fn directory_contains_translation_candidates(source: &Path) -> bool {
    scan_translation_candidates(source)
        .map(|candidates| !candidates.is_empty())
        .unwrap_or(false)
}

fn directory_contains_hardcoded_candidates(source: &Path) -> bool {
    let mut files = Vec::new();
    collect_hardcoded_files(source, &mut files);
    files.into_iter().any(|path| hardcoded_file_has_strings(&path))
}

fn preferred_extractable_payload(dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::<(usize, usize, PathBuf)>::new();
    collect_extractable_payloads(dir, dir, &mut candidates);
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    candidates.into_iter().map(|(_, _, path)| path).next()
}

fn collect_extractable_payloads(
    root: &Path,
    path: &Path,
    output: &mut Vec<(usize, usize, PathBuf)>,
) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let child = entry.path();
        if child
            .file_name()
            .map(|name| name.to_string_lossy().eq_ignore_ascii_case("disabled"))
            .unwrap_or(false)
        {
            continue;
        }
        if child.is_dir() {
            collect_extractable_payloads(root, &child, output);
            continue;
        }
        if is_supported_extractable_path(&child) {
            let depth = child
                .strip_prefix(root)
                .ok()
                .map(|relative| relative.components().count())
                .unwrap_or(usize::MAX);
            let kind_priority = if is_pck_path(&child) { 0 } else { 1 };
            output.push((depth, kind_priority, child));
        }
    }
}

fn first_active_payload(dir: &Path) -> Option<PathBuf> {
    let mut children = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| !name.to_string_lossy().eq_ignore_ascii_case("disabled"))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    children.sort();
    children.into_iter().next()
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn is_supported_archive_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "zip" | "7z" | "rar"
            )
        })
        .unwrap_or(false)
}

fn is_supported_extractable_path(path: &Path) -> bool {
    is_supported_archive_path(path) || is_pck_path(path)
}

fn is_pck_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "pck" | "pak"))
        .unwrap_or(false)
}

fn expand_source(source: &Path, destination: &Path, vendor_dir: &Path) -> bool {
    let expanded = if is_pck_path(source) {
        expand_pck(source, destination, vendor_dir)
    } else {
        expand_archive(source, destination, vendor_dir)
    };
    if expanded {
        expand_nested_pcks(destination, vendor_dir);
    }
    expanded
}

fn expand_archive(source: &Path, destination: &Path, vendor_dir: &Path) -> bool {
    if let Some(seven_zip) = embedded_7z_path(vendor_dir) {
        return expand_with_7z(&seven_zip, source, destination);
    }

    is_zip_path(source) && expand_zip_archive(source, destination)
}

fn embedded_7z_path(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir.join("7zip").join("7z.exe");
    path.exists().then_some(path)
}

fn expand_with_7z(seven_zip: &Path, source: &Path, destination: &Path) -> bool {
    if destination.exists() {
        return true;
    }
    if fs::create_dir_all(destination).is_err() {
        return false;
    }

    hidden_command(seven_zip)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", destination.to_string_lossy()))
        .arg(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn expand_nested_pcks(root: &Path, vendor_dir: &Path) {
    let mut pcks = Vec::new();
    collect_pck_files(root, &mut pcks);
    pcks.sort();
    for pck in pcks {
        let destination = pck.with_extension("pck.contents");
        let _ = expand_pck(&pck, &destination, vendor_dir);
    }
}

fn collect_pck_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if is_pck_path(path) {
            files.push(path.to_path_buf());
        }
        return;
    }
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.ends_with(".pck.contents"))
        .unwrap_or(false)
    {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        collect_pck_files(&entry.path(), files);
    }
}

fn expand_pck(source: &Path, destination: &Path, vendor_dir: &Path) -> bool {
    let Some(pck_explorer) = embedded_pck_explorer_path(vendor_dir) else {
        return false;
    };
    if destination.exists() {
        return true;
    }
    if fs::create_dir_all(destination).is_err() {
        return false;
    }
    hidden_command(pck_explorer)
        .arg("-e")
        .arg(source)
        .arg(destination)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn embedded_pck_explorer_path(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir
        .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
        .join("GodotPCKExplorer.Console.exe");
    path.exists().then_some(path)
}

fn expand_zip_archive(source: &Path, destination: &Path) -> bool {
    if destination.exists() {
        return true;
    }
    if fs::create_dir_all(destination).is_err() {
        return false;
    }
    let command = format!(
        "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        powershell_quote(source),
        powershell_quote(destination)
    );
    hidden_command("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn language_preview_extract_dir(cache_key: &str) -> PathBuf {
    resolve_workspace_dir()
        .join("state")
        .join("language_preview_extract")
        .join(format!("scan-{:016x}", stable_hash(cache_key)))
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_resource_id(resource_path: &str) -> String {
    let slug = resource_path
        .trim_start_matches("res://")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "resource".to_string()
    } else {
        slug
    };
    format!("{slug}-{:016x}", stable_hash(resource_path))
}

