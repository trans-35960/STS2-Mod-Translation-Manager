use super::source_json::{
    collect_json_files, directory_entry_key, flatten_source_values, slash_path,
    split_directory_entry_key,
};
use crate::error::AppResult;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) fn matching_target_language_values(
    source_path: &Path,
    target_language: &str,
) -> AppResult<BTreeMap<String, String>> {
    let Some(target_path) = matching_target_language_path(source_path, target_language) else {
        return Ok(BTreeMap::new());
    };
    if !target_path.exists() {
        return Ok(BTreeMap::new());
    }
    let target_values = flatten_source_values(&target_path)?;
    Ok(remap_target_language_values(
        target_values,
        source_path,
        target_language,
    ))
}

pub(crate) fn matching_target_language_path(
    source_path: &Path,
    target_language: &str,
) -> Option<PathBuf> {
    if let Some(target_path) = selected_source_target_language_path(source_path, target_language) {
        return Some(target_path);
    }
    if source_path.is_dir() || source_path.extension().is_none() {
        if let Some(language_dir) = find_first_localization_language_dir(source_path) {
            if language_dir == source_path {
                let localization_dir = language_dir.parent()?;
                return Some(localization_dir.join(target_language));
            }
            let relative_language_dir = language_dir.strip_prefix(source_path).ok()?;
            let target_relative =
                replace_localization_language_component(relative_language_dir, target_language)?;
            return Some(
                source_path
                    .parent()?
                    .join("translated")
                    .join(target_relative),
            );
        }
        let parent = source_path.parent()?;
        return Some(parent.join(target_language));
    }
    let source_language_dir = source_path.parent()?;
    let localization_dir = source_language_dir.parent()?;
    let file_name = source_path.file_name()?;
    Some(localization_dir.join(target_language).join(file_name))
}

pub(crate) fn source_path_matches_target_language(
    source_path: &Path,
    target_language: &str,
) -> bool {
    if source_path.is_file() {
        return source_path
            .parent()
            .is_some_and(|parent| path_language_matches(parent, target_language));
    }
    find_first_localization_language_dir(source_path)
        .as_deref()
        .is_some_and(|path| path_language_matches(path, target_language))
}

fn remap_target_language_values(
    values: BTreeMap<String, String>,
    source_path: &Path,
    target_language: &str,
) -> BTreeMap<String, String> {
    if source_path.is_file() {
        return values;
    }
    let Some(source_language_dir) = find_first_localization_language_dir(source_path) else {
        return values;
    };
    let Some(source_language) = source_language_dir
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
    else {
        return values;
    };
    if source_language.eq_ignore_ascii_case(target_language) {
        return values;
    }
    let source_language_prefix = (source_language_dir != source_path)
        .then(|| source_language_dir.strip_prefix(source_path).ok())
        .flatten()
        .map(Path::to_path_buf);
    values
        .into_iter()
        .map(|(key, value)| {
            let remapped_key = split_directory_entry_key(&key)
                .and_then(|(file, pointer)| {
                    replace_localization_language_component(Path::new(file), &source_language)
                        .or_else(|| {
                            source_language_prefix
                                .as_ref()
                                .map(|prefix| prefix.join(file))
                        })
                        .map(|path| directory_entry_key(&slash_path(&path), pointer))
                })
                .unwrap_or(key);
            (remapped_key, value)
        })
        .collect()
}

fn path_language_matches(path: &Path, target_language: &str) -> bool {
    path.file_name()
        .map(|value| {
            value
                .to_string_lossy()
                .eq_ignore_ascii_case(target_language)
        })
        .unwrap_or(false)
}

fn selected_source_target_language_path(
    source_path: &Path,
    target_language: &str,
) -> Option<PathBuf> {
    let source_root = selected_source_root(source_path)?;
    let relative = source_path.strip_prefix(source_root).ok()?;
    let target_relative = replace_localization_language_component(relative, target_language)?;
    Some(
        source_root
            .parent()?
            .join("translated")
            .join(target_relative),
    )
}

fn selected_source_root(path: &Path) -> Option<&Path> {
    path.ancestors().find(|ancestor| {
        ancestor
            .file_name()
            .map(|name| name.to_string_lossy().eq_ignore_ascii_case("source"))
            .unwrap_or(false)
    })
}

fn find_first_localization_language_dir(source_path: &Path) -> Option<PathBuf> {
    if is_localization_language_dir(source_path) {
        return Some(source_path.to_path_buf());
    }
    let mut json_files = collect_json_files(source_path).ok()?;
    json_files.sort();
    json_files.into_iter().find_map(|file| {
        for ancestor in file.ancestors() {
            if ancestor == source_path {
                break;
            }
            if is_localization_language_dir(ancestor) {
                return Some(ancestor.to_path_buf());
            }
        }
        None
    })
}

fn is_localization_language_dir(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .map(|value| value.to_string_lossy().eq_ignore_ascii_case("localization"))
        .unwrap_or(false)
}

pub(crate) fn target_language_relative_path(relative: &Path, target_language: &str) -> PathBuf {
    if let Some(after_language) = relative_after_localization_language(relative) {
        return after_language;
    }
    replace_localization_language_component(relative, target_language)
        .unwrap_or_else(|| relative.to_path_buf())
}

fn relative_after_localization_language(relative: &Path) -> Option<PathBuf> {
    let parts = path_parts(relative);
    let index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    if index + 2 >= parts.len() {
        return None;
    }
    Some(parts[index + 2..].iter().collect())
}

fn replace_localization_language_component(
    relative: &Path,
    target_language: &str,
) -> Option<PathBuf> {
    let mut parts = path_parts(relative);
    let index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    if index + 1 >= parts.len() {
        return None;
    }
    parts[index + 1] = target_language.to_string();
    Some(parts.iter().collect())
}

fn path_parts(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect()
}
