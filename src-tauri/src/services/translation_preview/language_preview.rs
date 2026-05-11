fn cached_language_preview(
    cache_key: &str,
    extraction_source: &Path,
    cache: &mut LanguagePreviewCache,
    current_cache_keys: &mut BTreeSet<String>,
    vendor_dir: &Path,
) -> Vec<LanguagePreviewDto> {
    current_cache_keys.insert(cache_key.to_string());
    if let Some(cached) = cache.entries.get(cache_key) {
        return cached.clone();
    }

    let detected = language_preview(extraction_source, &cache_key, vendor_dir);
    cache
        .entries
        .insert(cache_key.to_string(), detected.clone());
    cache.dirty = true;
    detected
}

fn source_labels(builder: &ModRowBuilder) -> String {
    let mut labels = Vec::new();
    if builder.active.is_some() {
        labels.push("게임 폴더");
    }
    if builder.external.is_some() {
        labels.push("Nexus/Vortex");
    }
    labels.join(" + ")
}

fn is_game_disabled_record(record: &ModRecord) -> bool {
    record.path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value.eq_ignore_ascii_case(".disabled") || value.eq_ignore_ascii_case("mods.disabled")
    })
}

fn translation_state(
    extraction_source: &Path,
    language_preview: &[LanguagePreviewDto],
) -> (String, String) {
    if !language_preview.is_empty() {
        let labels = language_preview
            .iter()
            .take(4)
            .map(|language| language.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return (
            format!("{}개 언어", language_preview.len()),
            format!("{labels} 자동 감지"),
        );
    }
    if hardcoded_source_count(extraction_source) > 0 {
        return (
            "하드코딩".to_string(),
            "DLL/EXE 내부 고정 문자열 후보 감지".to_string(),
        );
    }

    let extension = extraction_source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "zip" => (
            "언어 없음".to_string(),
            "ZIP 내부에서 localization 언어 파일을 찾지 못함".to_string(),
        ),
        "7z" | "rar" => (
            "언어 없음".to_string(),
            "압축 내부에서 localization 언어 파일을 찾지 못함".to_string(),
        ),
        "pck" | "pak" => (
            "언어 없음".to_string(),
            "PCK 내부에서 localization 언어 파일을 찾지 못함".to_string(),
        ),
        _ if extraction_source.is_dir() => (
            "언어 없음".to_string(),
            "폴더에서 localization 언어 파일을 찾지 못함".to_string(),
        ),
        _ => ("검토 필요".to_string(), "지원 형식 확인 필요".to_string()),
    }
}

fn language_preview(source: &Path, cache_key: &str, vendor_dir: &Path) -> Vec<LanguagePreviewDto> {
    let Some(scan_root) = extraction_scan_root(source, cache_key, vendor_dir) else {
        return Vec::new();
    };
    language_preview_from_scan_root(&scan_root)
}

fn language_preview_from_scan_root(scan_root: &Path) -> Vec<LanguagePreviewDto> {
    let Ok(candidates) = scan_translation_candidates(&scan_root) else {
        return Vec::new();
    };

    let mut by_code = BTreeMap::<String, LanguagePreviewBuilder>::new();
    for candidate in candidates {
        if is_hardcoded_source_file(&candidate.path) {
            continue;
        }
        let code = infer_language_code(&candidate.path).unwrap_or_else(|| "unknown".to_string());
        let entry = by_code
            .entry(code.clone())
            .or_insert_with(|| LanguagePreviewBuilder {
                code: code.clone(),
                label: language_label(&code).to_string(),
                files: 0,
                keys: 0,
                sample_path: resource_display_path(&candidate.path),
            });
        entry.files += 1;
        entry.keys += count_json_translation_keys(&candidate.path);
    }

    let mut previews = by_code
        .into_values()
        .map(|entry| LanguagePreviewDto {
            code: entry.code,
            label: entry.label,
            files: entry.files,
            keys: entry.keys,
            sample_path: entry.sample_path,
        })
        .collect::<Vec<_>>();
    sort_language_previews(&mut previews);
    previews.into_iter().take(8).collect()
}

fn compare_scan_root(sheet: &JsonTranslationSheet) -> Option<PathBuf> {
    let source_path = PathBuf::from(&sheet.source_path);
    let context = read_translation_context(&source_path).unwrap_or_default();
    if let Some(mod_key) = context.mod_key.as_deref() {
        let app = app();
        if let Ok(record) = find_mod_record(&app, mod_key) {
            let extraction_source = extraction_source_for_record(&record);
            let cache_key =
                language_cache_key(&record, &extraction_source, &app.config().vendor_dir);
            if let Some(root) =
                extraction_scan_root(&extraction_source, &cache_key, &app.config().vendor_dir)
            {
                return Some(root);
            }
        }
    }
    if let Some(source) = context
        .extraction_source_path
        .as_ref()
        .filter(|path| path.exists())
    {
        let cache_key = format!("compare-{:016x}", stable_hash(&source.to_string_lossy()));
        if let Some(root) = extraction_scan_root(source, &cache_key, &app().config().vendor_dir) {
            return Some(root);
        }
    }
    let source = PathBuf::from(&sheet.source_path);
    source.exists().then_some(source)
}


fn extraction_target(source: &Path, language_preview: &[LanguagePreviewDto]) -> String {
    if !language_preview.is_empty() {
        let labels = language_preview
            .iter()
            .map(|language| language.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{labels} 언어 파일 {}개", language_preview.len());
    }
    if hardcoded_source_count(source) > 0 {
        return "DLL/EXE 내부 고정 문자열 후보".to_string();
    }

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "zip" => "ZIP 내부 localization/lang/strings 계열 파일".to_string(),
        "7z" | "rar" => "압축 내부 localization/lang/strings 계열 파일".to_string(),
        "pck" | "pak" => "PCK 내부 파일".to_string(),
        _ if source.is_dir() => "폴더 내부 localization/lang/strings 계열 파일".to_string(),
        _ => "지원 형식 검토 필요".to_string(),
    }
}

struct LanguagePreviewBuilder {
    code: String,
    label: String,
    files: usize,
    keys: usize,
    sample_path: String,
}

fn sort_language_previews(previews: &mut Vec<LanguagePreviewDto>) {
    previews.sort_by(|left, right| {
        right
            .keys
            .cmp(&left.keys)
            .then_with(|| right.files.cmp(&left.files))
            .then_with(|| left.code.cmp(&right.code))
    });
    previews.dedup_by(|left, right| left.code == right.code);
}

fn count_json_translation_keys(path: &Path) -> usize {
    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return count_translation_file_keys(path);
    }
    let mut files = Vec::new();
    collect_files_with_extension(path, "json", &mut files);
    collect_files_with_extension(path, "loc", &mut files);
    files
        .iter()
        .map(|file| count_translation_file_keys(file))
        .sum()
}

fn hardcoded_source_count(source: &Path) -> usize {
    if source.is_file() && is_hardcoded_source_file(source) {
        return usize::from(hardcoded_file_has_strings(source));
    }
    if !source.is_dir() {
        return 0;
    }
    let mut files = Vec::new();
    collect_hardcoded_files(source, &mut files);
    files
        .into_iter()
        .filter(|path| hardcoded_file_has_strings(path))
        .count()
}

fn count_translation_file_keys(path: &Path) -> usize {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "json" | "loc" => count_json_file_string_keys(path),
        _ => 0,
    }
}

fn count_json_file_string_keys(path: &Path) -> usize {
    let Ok(content) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(value) =
        serde_json::from_str::<serde_json::Value>(content.trim_start_matches('\u{feff}'))
    else {
        return 0;
    };
    count_json_string_leaves(&value)
}

fn count_json_string_leaves(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => usize::from(!text.trim().is_empty()),
        serde_json::Value::Array(items) => items.iter().map(count_json_string_leaves).sum(),
        serde_json::Value::Object(entries) => entries.values().map(count_json_string_leaves).sum(),
        _ => 0,
    }
}

fn infer_language_code(path: &Path) -> Option<String> {
    let mut tokens = Vec::new();
    for component in path.components() {
        tokens.extend(split_language_tokens(
            &component.as_os_str().to_string_lossy(),
        ));
    }
    if let Some(stem) = path.file_stem() {
        tokens.extend(split_language_tokens(&stem.to_string_lossy()));
    }

    for token in tokens {
        let normalized = normalize_language_code(&token);
        if is_known_language_code(&normalized) {
            return Some(normalized);
        }
    }

    None
}

fn split_language_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
    })
    .flat_map(|part| part.split(['-', '_']))
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .map(|part| part.to_ascii_lowercase())
    .collect()
}

fn normalize_language_code(token: &str) -> String {
    match token {
        "kr" | "kor" | "korean" => "kor",
        "eng" | "english" => "en",
        "jpn" | "jp" | "japanese" => "ja",
        "zhs" | "chs" | "sc" | "simplified" => "zh-cn",
        "zht" | "cht" | "tc" | "traditional" => "zh-tw",
        "deu" | "ger" | "german" => "de",
        "fra" | "fre" | "french" => "fr",
        "spa" | "spanish" => "es",
        "rus" | "russian" => "ru",
        "por" | "portuguese" => "pt",
        "italian" => "it",
        other => other,
    }
    .to_string()
}

fn is_known_language_code(code: &str) -> bool {
    matches!(
        code,
        "ko" | "kor"
            | "en"
            | "ja"
            | "zh"
            | "zh-cn"
            | "zh-tw"
            | "de"
            | "fr"
            | "es"
            | "ru"
            | "pt"
            | "it"
            | "pl"
            | "tr"
            | "vi"
            | "th"
            | "id"
    )
}

fn language_label(code: &str) -> &str {
    match code {
        "ko" => "한국어",
        "kor" => "한국어",
        "en" => "English",
        "ja" => "日本語",
        "zh" | "zh-cn" => "简体中文",
        "zh-tw" => "繁體中文",
        "de" => "Deutsch",
        "fr" => "Français",
        "es" => "Español",
        "ru" => "Русский",
        "pt" => "Português",
        "it" => "Italiano",
        "pl" => "Polski",
        "tr" => "Türkçe",
        "vi" => "Tiếng Việt",
        "th" => "ไทย",
        "id" => "Indonesia",
        _ => "Unknown",
    }
}

fn read_language_preview_cache(
    config: &AppConfig,
) -> sts2_mod_manager::error::AppResult<LanguagePreviewCache> {
    let path = language_preview_cache_path(config);
    if !path.exists() {
        return Ok(LanguagePreviewCache {
            entries: BTreeMap::new(),
            dirty: false,
        });
    }

    let content = fs::read_to_string(&path)
        .map_err(|source| sts2_mod_manager::error::AppError::io(path.as_path(), source))?;
    let mut entries = BTreeMap::<String, Vec<LanguagePreviewDto>>::new();
    for line in content.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() == 5 && parts[1] == "__none__" {
            entries.entry(parts[0].to_string()).or_default();
            continue;
        }
        if parts.len() != 6 {
            continue;
        }
        let files = parts[3].parse::<usize>().unwrap_or(0);
        let keys = parts[4].parse::<usize>().unwrap_or(files);
        if parts[1].is_empty() && parts[2].is_empty() && files == 0 {
            continue;
        }
        if parts[1] == "__none__" {
            entries.entry(parts[0].to_string()).or_default();
            continue;
        }
        entries
            .entry(parts[0].to_string())
            .or_default()
            .push(LanguagePreviewDto {
                code: unescape_cache_field(parts[1]),
                label: unescape_cache_field(parts[2]),
                files,
                keys,
                sample_path: unescape_cache_field(parts[5]),
            });
    }

    Ok(LanguagePreviewCache {
        entries,
        dirty: false,
    })
}

fn write_language_preview_cache(
    config: &AppConfig,
    cache: &LanguagePreviewCache,
) -> sts2_mod_manager::error::AppResult<()> {
    let path = language_preview_cache_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| sts2_mod_manager::error::AppError::io(parent, source))?;
    }

    let mut output = String::new();
    for (cache_key, languages) in &cache.entries {
        if languages.is_empty() {
            output.push_str(cache_key);
            output.push_str("\t__none__\t\t0\t\n");
            continue;
        }
        for language in languages {
            output.push_str(cache_key);
            output.push('\t');
            output.push_str(&escape_cache_field(&language.code));
            output.push('\t');
            output.push_str(&escape_cache_field(&language.label));
            output.push('\t');
            output.push_str(&language.files.to_string());
            output.push('\t');
            output.push_str(&language.keys.to_string());
            output.push('\t');
            output.push_str(&escape_cache_field(&language.sample_path));
            output.push('\n');
        }
    }

    fs::write(&path, output)
        .map_err(|source| sts2_mod_manager::error::AppError::io(path.as_path(), source))
}

fn language_preview_cache_path(config: &AppConfig) -> PathBuf {
    config.state_dir.join("language_preview_cache.tsv")
}

fn language_cache_key(record: &ModRecord, extraction_source: &Path, vendor_dir: &Path) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        "sts2-localization-v4",
        record.source.as_key(),
        record.stable_key(),
        record.fingerprint.bytes,
        epoch_seconds(record.fingerprint.modified).unwrap_or(0),
        record.path.to_string_lossy(),
        extraction_source.to_string_lossy(),
        vendor_extractor_signature(vendor_dir)
    )
}

fn vendor_extractor_signature(vendor_dir: &Path) -> String {
    [
        vendor_dir.join("7zip").join("7z.exe"),
        vendor_dir
            .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
            .join("GodotPCKExplorer.Console.exe"),
    ]
    .into_iter()
    .map(|path| match fs::metadata(&path) {
        Ok(metadata) => format!(
            "{}:{}:{}",
            path.file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default(),
            metadata.len(),
            epoch_seconds(metadata.modified().ok()).unwrap_or(0)
        ),
        Err(_) => format!(
            "{}:missing",
            path.file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default()
        ),
    })
    .collect::<Vec<_>>()
    .join("|")
}

fn escape_cache_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_cache_field(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some('t') => output.push('\t'),
                Some('n') => output.push('\n'),
                Some('\\') => output.push('\\'),
                Some(other) => {
                    output.push('\\');
                    output.push(other);
                }
                None => output.push('\\'),
            }
        } else {
            output.push(character);
        }
    }
    output
}

