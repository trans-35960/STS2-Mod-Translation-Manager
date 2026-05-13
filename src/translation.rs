use crate::domain::TranslationCandidate;
use crate::error::{AppError, AppResult};
use crate::json_translation::{flatten_hardcoded_values, is_hardcoded_source_file};
#[cfg(test)]
use crate::process::powershell_compress_directory_contents;
use crate::process::{hidden_command, powershell_expand_archive};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationExtractReport {
    pub mod_key: String,
    pub version_id: String,
    pub workspace_dir: PathBuf,
    pub candidates: Vec<TranslationCandidate>,
    pub review_required: bool,
    pub review_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationWorkspace {
    pub mod_key: String,
    pub version_id: String,
    pub path: PathBuf,
    pub review_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationMergeReport {
    pub merged_files: Vec<PathBuf>,
    pub backup_dir: PathBuf,
}

pub fn scan_translation_candidates(root: &Path) -> AppResult<Vec<TranslationCandidate>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    visit(root, root, &mut candidates)?;
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(candidates)
}

pub fn extract_translation_work(
    source: &Path,
    work_root: &Path,
    vendor_dir: &Path,
) -> AppResult<TranslationExtractReport> {
    if !source.exists() {
        return Err(AppError::InvalidCommand(format!(
            "translation source does not exist: {}",
            source.display()
        )));
    }

    fs::create_dir_all(work_root).map_err(|error| AppError::io(work_root, error))?;

    let mod_key = stable_key_for_path(source);
    let version_id = version_id_for_path(source)?;
    let mod_root = work_root.join(&mod_key);
    let workspace_dir = mod_root.join(&version_id);
    let source_dir = workspace_dir.join("source");
    let translated_dir = workspace_dir.join("translated");

    fs::create_dir_all(&source_dir).map_err(|error| AppError::io(&source_dir, error))?;
    fs::create_dir_all(&translated_dir).map_err(|error| AppError::io(&translated_dir, error))?;

    let previous_versions = previous_versions(&mod_root, &version_id)?;
    let candidates = if source.is_dir() {
        let candidates = scan_translation_candidates(source)?;
        for candidate in &candidates {
            let relative = candidate.path.strip_prefix(source).map_err(|_| {
                AppError::InvalidCommand(format!(
                    "candidate is outside source: {}",
                    candidate.path.display()
                ))
            })?;
            copy_file_to(&candidate.path, &source_dir.join(relative))?;
            copy_if_missing(&candidate.path, &translated_dir.join(relative))?;
        }
        candidates
    } else if let Some(candidate) = classify_translation_file(source, source)? {
        let file_name = source.file_name().ok_or_else(|| {
            AppError::InvalidCommand(format!("source has no filename: {}", source.display()))
        })?;
        copy_file_to(source, &source_dir.join(file_name))?;
        copy_if_missing(source, &translated_dir.join(file_name))?;
        vec![candidate]
    } else if is_supported_extractable(source) {
        let expanded_dir = workspace_dir.join("expanded_archive");
        if expand_source(source, &expanded_dir, vendor_dir)? {
            let candidates = scan_translation_candidates(&expanded_dir)?;
            for candidate in &candidates {
                let relative = candidate.path.strip_prefix(&expanded_dir).map_err(|_| {
                    AppError::InvalidCommand(format!(
                        "candidate is outside archive extraction: {}",
                        candidate.path.display()
                    ))
                })?;
                copy_file_to(&candidate.path, &source_dir.join(relative))?;
                copy_if_missing(&candidate.path, &translated_dir.join(relative))?;
            }
            let _ = fs::remove_dir_all(&expanded_dir);
            candidates
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    write_manifest(&workspace_dir, source, &candidates)?;

    let review_required = !previous_versions.is_empty() || candidates.is_empty();
    let review_path = if review_required {
        Some(write_review_file(
            &workspace_dir,
            source,
            &previous_versions,
            candidates.is_empty(),
        )?)
    } else {
        None
    };

    Ok(TranslationExtractReport {
        mod_key,
        version_id,
        workspace_dir,
        candidates,
        review_required,
        review_path,
    })
}

pub fn list_translation_workspaces(work_root: &Path) -> AppResult<Vec<TranslationWorkspace>> {
    if !work_root.exists() {
        return Ok(Vec::new());
    }

    let mut workspaces = Vec::new();
    for mod_entry in fs::read_dir(work_root).map_err(|error| AppError::io(work_root, error))? {
        let mod_entry = mod_entry.map_err(|error| AppError::io(work_root, error))?;
        let mod_path = mod_entry.path();
        if !mod_path.is_dir() {
            continue;
        }

        for version_entry in
            fs::read_dir(&mod_path).map_err(|error| AppError::io(&mod_path, error))?
        {
            let version_entry = version_entry.map_err(|error| AppError::io(&mod_path, error))?;
            let path = version_entry.path();
            if path.is_dir() {
                workspaces.push(TranslationWorkspace {
                    mod_key: mod_entry.file_name().to_string_lossy().to_string(),
                    version_id: version_entry.file_name().to_string_lossy().to_string(),
                    review_required: path.join("review_required.md").exists(),
                    path,
                });
            }
        }
    }

    workspaces.sort_by(|left, right| {
        left.mod_key
            .cmp(&right.mod_key)
            .then_with(|| left.version_id.cmp(&right.version_id))
    });
    Ok(workspaces)
}

pub fn merge_translation_workspace(
    workspace_dir: &Path,
    target_root: &Path,
) -> AppResult<TranslationMergeReport> {
    let translated_dir = workspace_dir.join("translated");
    if !translated_dir.exists() {
        return Err(AppError::InvalidCommand(format!(
            "translated folder not found: {}",
            translated_dir.display()
        )));
    }

    fs::create_dir_all(target_root).map_err(|error| AppError::io(target_root, error))?;
    let backup_dir = workspace_dir.join("merge_backups").join(timestamp_string());
    fs::create_dir_all(&backup_dir).map_err(|error| AppError::io(&backup_dir, error))?;

    let mut merged_files = Vec::new();
    for file in collect_files(&translated_dir)? {
        let relative = file.strip_prefix(&translated_dir).map_err(|_| {
            AppError::InvalidCommand(format!(
                "translated file outside workspace: {}",
                file.display()
            ))
        })?;
        let target = target_root.join(relative);
        if target.exists() {
            copy_file_to(&target, &backup_dir.join(relative))?;
        }
        copy_file_to(&file, &target)?;
        merged_files.push(target);
    }

    Ok(TranslationMergeReport {
        merged_files,
        backup_dir,
    })
}

fn visit(root: &Path, path: &Path, candidates: &mut Vec<TranslationCandidate>) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|source| AppError::io(path, source))?;

    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|source| AppError::io(path, source))? {
            let entry = entry.map_err(|source| AppError::io(path, source))?;
            visit(root, &entry.path(), candidates)?;
        }
        return Ok(());
    }

    if let Some(candidate) = classify_translation_file(path, root)? {
        candidates.push(candidate);
    }

    Ok(())
}

fn classify_translation_file(path: &Path, root: &Path) -> AppResult<Option<TranslationCandidate>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(extension) = extension else {
        return Ok(None);
    };

    if is_hardcoded_source_file(path) && has_translatable_hardcoded_values(path)? {
        return Ok(Some(TranslationCandidate {
            path: path.to_path_buf(),
            extension,
            reason: "hardcoded .NET/UTF-16 strings".to_string(),
        }));
    }

    let path_text = language_candidate_path_text(path, root);
    let name_text = path
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let looks_like_language_path = path_text.contains("lang")
        || path_text.contains("localization")
        || path_text.contains("localisation")
        || path_text.contains("translation")
        || path_text.contains("i18n")
        || contains_language_code_token(&path_text);

    let known_extension = matches!(
        extension.as_str(),
        "json" | "loc" | "csv" | "tsv" | "po" | "pot" | "mo" | "txt" | "xml" | "yaml" | "yml"
    );

    let known_name = name_text.contains("strings")
        || name_text.contains("dialog")
        || name_text.contains("language")
        || name_text.contains("locale");

    if known_extension && (looks_like_language_path || known_name) {
        let reason = if looks_like_language_path {
            "language-like path"
        } else {
            "language-like filename"
        };
        Ok(Some(TranslationCandidate {
            path: path.to_path_buf(),
            extension,
            reason: reason.to_string(),
        }))
    } else {
        Ok(None)
    }
}

fn has_translatable_hardcoded_values(path: &Path) -> AppResult<bool> {
    let values = flatten_hardcoded_values(path)?;
    Ok(values
        .values()
        .any(|value| looks_like_translatable_hardcoded_value(value)))
}

fn looks_like_translatable_hardcoded_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.chars().count() >= 4
        && trimmed.chars().any(char::is_alphabetic)
        && trimmed
            .chars()
            .any(|character| character.is_whitespace() || ".:,;!?".contains(character))
}

fn contains_language_code_token(path_text: &str) -> bool {
    path_text
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .flat_map(|part| part.split(['-', '_']))
        .map(|part| part.to_ascii_lowercase())
        .any(|part| {
            matches!(
                part.as_str(),
                "eng"
                    | "en"
                    | "kor"
                    | "ko"
                    | "zhs"
                    | "chs"
                    | "zht"
                    | "cht"
                    | "jpn"
                    | "ja"
                    | "rus"
                    | "ru"
            )
        })
}

fn language_candidate_path_text(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut parts = Vec::new();
    if relative
        .parent()
        .is_none_or(|parent| parent.as_os_str().is_empty())
        && let Some(root_name) = root.file_name()
    {
        parts.push(root_name.to_string_lossy().to_string());
    }
    parts.push(relative.to_string_lossy().to_string());
    parts.join("/").to_ascii_lowercase()
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn is_supported_archive(path: &Path) -> bool {
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

fn is_supported_extractable(path: &Path) -> bool {
    is_supported_archive(path) || is_pck_file(path)
}

fn is_pck_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "pck" | "pak"))
        .unwrap_or(false)
}

fn expand_source(source: &Path, destination: &Path, vendor_dir: &Path) -> AppResult<bool> {
    let expanded = if is_pck_file(source) {
        expand_pck(source, destination, vendor_dir)?
    } else {
        expand_archive(source, destination, vendor_dir)?
    };
    if expanded {
        expand_nested_pcks(destination, vendor_dir)?;
    }
    Ok(expanded)
}

fn expand_archive(source: &Path, destination: &Path, vendor_dir: &Path) -> AppResult<bool> {
    if let Some(seven_zip) = embedded_7z_path(vendor_dir) {
        return expand_with_7z(&seven_zip, source, destination);
    }

    if is_zip_file(source) {
        return expand_zip_archive(source, destination);
    }

    Ok(false)
}

fn embedded_7z_path(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir.join("7zip").join("7z.exe");
    path.exists().then_some(path)
}

fn expand_with_7z(seven_zip: &Path, source: &Path, destination: &Path) -> AppResult<bool> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| AppError::io(destination, error))?;
    }
    fs::create_dir_all(destination).map_err(|error| AppError::io(destination, error))?;

    let status = hidden_command(seven_zip)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", destination.to_string_lossy()))
        .arg(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| AppError::io(seven_zip, error))?;

    Ok(status.success())
}

fn expand_nested_pcks(root: &Path, vendor_dir: &Path) -> AppResult<()> {
    let pcks = collect_pck_files(root)?;
    for pck in pcks {
        let destination = pck.with_extension("pck.contents");
        let _ = expand_pck(&pck, &destination, vendor_dir)?;
    }
    Ok(())
}

fn collect_pck_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_pck_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_pck_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io(path, error))?;
    if metadata.is_dir() {
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|name| name.ends_with(".pck.contents"))
            .unwrap_or(false)
        {
            return Ok(());
        }
        for entry in fs::read_dir(path).map_err(|error| AppError::io(path, error))? {
            let entry = entry.map_err(|error| AppError::io(path, error))?;
            collect_pck_files_inner(&entry.path(), files)?;
        }
    } else if is_pck_file(path) {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn expand_pck(source: &Path, destination: &Path, vendor_dir: &Path) -> AppResult<bool> {
    let Some(pck_explorer) = embedded_pck_explorer_path(vendor_dir) else {
        return Ok(false);
    };
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| AppError::io(destination, error))?;
    }
    fs::create_dir_all(destination).map_err(|error| AppError::io(destination, error))?;

    let status = hidden_command(&pck_explorer)
        .arg("-e")
        .arg(source)
        .arg(destination)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| AppError::io(&pck_explorer, error))?;

    Ok(status.success())
}

fn embedded_pck_explorer_path(vendor_dir: &Path) -> Option<PathBuf> {
    let path = vendor_dir
        .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
        .join("GodotPCKExplorer.Console.exe");
    path.exists().then_some(path)
}

fn expand_zip_archive(source: &Path, destination: &Path) -> AppResult<bool> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| AppError::io(destination, error))?;
    }
    fs::create_dir_all(destination).map_err(|error| AppError::io(destination, error))?;

    let status = powershell_expand_archive(source, destination)
        .map_err(|error| AppError::io(source, error))?;

    Ok(status.success())
}

fn write_manifest(
    workspace_dir: &Path,
    source: &Path,
    candidates: &[TranslationCandidate],
) -> AppResult<()> {
    let mut output = String::new();
    output.push_str("source\tpath\textension\treason\n");
    for candidate in candidates {
        output.push_str(&source.to_string_lossy());
        output.push('\t');
        output.push_str(&candidate.path.to_string_lossy());
        output.push('\t');
        output.push_str(&candidate.extension);
        output.push('\t');
        output.push_str(&candidate.reason);
        output.push('\n');
    }

    let path = workspace_dir.join("manifest.tsv");
    fs::write(&path, output).map_err(|error| AppError::io(path, error))
}

fn write_review_file(
    workspace_dir: &Path,
    source: &Path,
    previous_versions: &[String],
    unsupported_or_empty: bool,
) -> AppResult<PathBuf> {
    let path = workspace_dir.join("review_required.md");
    let mut output = String::new();
    output.push_str("# Translation Review Required\n\n");
    output.push_str(&format!("Source: `{}`\n\n", source.display()));

    if !previous_versions.is_empty() {
        output.push_str("A previous translation workspace exists for this mod. Compare source files before merging translated content.\n\n");
        output.push_str("Previous versions:\n");
        for version in previous_versions {
            output.push_str(&format!("- `{version}`\n"));
        }
        output.push('\n');
    }

    if unsupported_or_empty {
        output.push_str("No direct language files were extracted. If this is an archive, add an embedded extractor under `vendor/` before automated extraction.\n");
    }

    fs::write(&path, output).map_err(|error| AppError::io(&path, error))?;
    Ok(path)
}

fn previous_versions(mod_root: &Path, current_version: &str) -> AppResult<Vec<String>> {
    if !mod_root.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in fs::read_dir(mod_root).map_err(|error| AppError::io(mod_root, error))? {
        let entry = entry.map_err(|error| AppError::io(mod_root, error))?;
        if entry.path().is_dir() {
            let version = entry.file_name().to_string_lossy().to_string();
            if version != current_version {
                versions.push(version);
            }
        }
    }
    versions.sort();
    Ok(versions)
}

fn version_id_for_path(path: &Path) -> AppResult<String> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io(path, error))?;
    let bytes = if metadata.is_dir() {
        collect_files(path)?
            .into_iter()
            .map(|file| {
                fs::metadata(&file)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0)
            })
            .sum::<u64>()
    } else {
        metadata.len()
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(format!("b{bytes}-t{modified}"))
}

fn stable_key_for_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "translation-source".to_string())
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
        .collect::<Vec<_>>()
        .join("-")
}

fn collect_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io(path, error))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| AppError::io(path, error))? {
            let entry = entry.map_err(|error| AppError::io(path, error))?;
            collect_files_inner(&entry.path(), files)?;
        }
    } else {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn copy_file_to(source: &Path, target: &Path) -> AppResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }
    fs::copy(source, target)
        .map(|_| ())
        .map_err(|error| AppError::io(source, error))
}

fn copy_if_missing(source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() {
        return Ok(());
    }
    copy_file_to(source, target)
}

fn timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::SystemTime;

    #[test]
    fn finds_language_files() {
        let fixture = TestDir::create("translation_candidates");
        fixture.write_file("localization/en.json", r#"{"hello":"Hello"}"#);
        fixture.write_file("assets/image.png", "not language");

        let candidates = scan_translation_candidates(fixture.path()).expect("scan translations");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].extension, "json");
    }

    #[test]
    fn finds_loc_language_files() {
        let fixture = TestDir::create("translation_loc_candidates");
        fixture.write_file("localization/eng/all.loc", r#"{"hello":"Hello"}"#);
        fixture.write_file("blight.dll", "not language");

        let candidates = scan_translation_candidates(fixture.path()).expect("scan translations");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].extension, "loc");
        assert!(candidates[0].path.ends_with("localization/eng/all.loc"));
    }

    #[test]
    fn finds_hardcoded_dll_language_candidates() {
        let fixture = TestDir::create("translation_hardcoded_dll_candidates");
        fixture.write_bytes(
            "AscensionUnlockMod.dll",
            &[
                b"prefix".as_slice(),
                &[0, 0xff],
                &utf16le_test_bytes("Current ascension state unavailable."),
                &[0, 0xff],
                b"suffix".as_slice(),
            ]
            .concat(),
        );
        fixture.write_bytes("UnifiedSavePath.dll", &utf16le_test_bytes("profile"));

        let candidates = scan_translation_candidates(fixture.path()).expect("scan translations");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].extension, "dll");
        assert_eq!(candidates[0].reason, "hardcoded .NET/UTF-16 strings");
        assert!(candidates[0].path.ends_with("AscensionUnlockMod.dll"));
    }

    #[test]
    fn language_code_roots_are_translation_candidates() {
        let fixture = TestDir::create("translation_code_root");
        fixture.write_file(
            "zhs/cards.json",
            r#"{"title":"打击","description":"造成伤害。"}"#,
        );
        fixture.write_file("zhs/powers.json", r#"{"name":"力量"}"#);

        let candidates =
            scan_translation_candidates(&fixture.path().join("zhs")).expect("scan code root");

        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn ignores_work_dir_names_when_classifying_json() {
        let fixture = TestDir::create("translation_work_false_positive");
        fixture.write_file(
            "translation_work/expanded_archive/AkiSister/AkiSister.json",
            "{}",
        );

        let candidates = scan_translation_candidates(
            &fixture
                .path()
                .join("translation_work")
                .join("expanded_archive"),
        )
        .expect("scan translations");

        assert!(candidates.is_empty());
    }

    #[test]
    fn extracts_translation_workspace() {
        let fixture = TestDir::create("extracts_translation_workspace");
        fixture.write_file("mod/localization/en.json", r#"{"hello":"Hello"}"#);

        let report = extract_translation_work(
            &fixture.path().join("mod"),
            &fixture.path().join("work"),
            &fixture.path().join("vendor"),
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert!(
            report
                .workspace_dir
                .join("source/localization/en.json")
                .exists()
        );
        assert!(
            report
                .workspace_dir
                .join("translated/localization/en.json")
                .exists()
        );
    }

    #[test]
    fn extracts_loc_translation_workspace() {
        let fixture = TestDir::create("extracts_loc_translation_workspace");
        fixture.write_file("mod/localization/eng/all.loc", r#"{"hello":"Hello"}"#);

        let report = extract_translation_work(
            &fixture.path().join("mod"),
            &fixture.path().join("work"),
            &fixture.path().join("vendor"),
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert!(!report.review_required);
        assert!(
            report
                .workspace_dir
                .join("source/localization/eng/all.loc")
                .exists()
        );
        assert!(
            report
                .workspace_dir
                .join("translated/localization/eng/all.loc")
                .exists()
        );
    }

    #[test]
    fn archive_like_sources_create_review_request() {
        let fixture = TestDir::create("archive_like_sources_create_review_request");
        fixture.write_file("CoolMod.zip", "archive bytes");

        let report = extract_translation_work(
            &fixture.path().join("CoolMod.zip"),
            &fixture.path().join("work"),
            &fixture.path().join("vendor"),
        )
        .expect("extract archive marker");

        assert!(report.review_required);
        assert!(report.review_path.expect("review path").exists());
    }

    #[test]
    fn zip_archives_extract_language_files() {
        let fixture = TestDir::create("zip_archives_extract_language_files");
        fixture.write_file("zip-src/localization/en.json", r#"{"hello":"Hello"}"#);
        let archive_path = fixture.path().join("ArchiveMod.zip");

        let status =
            powershell_compress_directory_contents(&fixture.path().join("zip-src"), &archive_path)
                .expect("run Compress-Archive");
        assert!(status.success());

        let report = extract_translation_work(
            &archive_path,
            &fixture.path().join("work"),
            &fixture.path().join("vendor"),
        )
        .expect("extract");

        assert_eq!(report.candidates.len(), 1);
        assert!(!report.review_required);
        assert!(
            report
                .workspace_dir
                .join("source/localization/en.json")
                .exists()
        );
    }

    #[test]
    #[cfg(windows)]
    fn seven_zip_archives_extract_language_files() {
        let seven_zip = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("vendor")
            .join("7zip")
            .join("7z.exe");
        assert!(
            seven_zip.exists(),
            "embedded 7-Zip is required for this test"
        );

        let fixture = TestDir::create("seven_zip_archives_extract_language_files");
        fixture.write_file("7z-src/localization/ko.json", r#"{"hello":"안녕"}"#);
        let archive_path = fixture.path().join("ArchiveMod.7z");

        let status = hidden_command(&seven_zip)
            .arg("a")
            .arg("-y")
            .arg(&archive_path)
            .arg(fixture.path().join("7z-src").join("*"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("create 7z archive");
        assert!(status.success());

        let report = extract_translation_work(
            &archive_path,
            &fixture.path().join("work"),
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor"),
        )
        .expect("extract 7z archive");

        assert_eq!(report.candidates.len(), 1);
        assert!(!report.review_required);
        assert!(
            report
                .workspace_dir
                .join("source/localization/ko.json")
                .exists()
        );
    }

    #[test]
    fn merges_translated_files_with_backup() {
        let fixture = TestDir::create("merges_translated_files");
        fixture.write_file(
            "work/translated/localization/en.json",
            r#"{"hello":"Hello KR"}"#,
        );
        fixture.write_file("target/localization/en.json", r#"{"hello":"Hello"}"#);

        let report = merge_translation_workspace(
            &fixture.path().join("work"),
            &fixture.path().join("target"),
        )
        .expect("merge");

        assert_eq!(report.merged_files.len(), 1);
        assert_eq!(
            fs::read_to_string(fixture.path().join("target/localization/en.json"))
                .expect("merged content"),
            r#"{"hello":"Hello KR"}"#
        );
        assert!(report.backup_dir.join("localization/en.json").exists());
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn create(name: &str) -> Self {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("target");
            path.push("test-work");
            path.push(format!(
                "{}-{}",
                name,
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("create test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, child: &str, content: &str) {
            let path = self.path.join(child);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            let mut file = fs::File::create(path).expect("create file");
            file.write_all(content.as_bytes()).expect("write file");
        }

        fn write_bytes(&self, child: &str, content: &[u8]) {
            let path = self.path.join(child);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            let mut file = fs::File::create(path).expect("create file");
            file.write_all(content).expect("write file");
        }
    }

    fn utf16le_test_bytes(value: &str) -> Vec<u8> {
        value
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    }
}
