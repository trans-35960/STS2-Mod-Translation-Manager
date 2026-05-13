use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn ensure_existing_path_in_roots(
    path: &Path,
    roots: &[PathBuf],
    description: &str,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("{description} 확인 실패: {} ({error})", path.display()))?;
    ensure_canonical_path_in_roots(&canonical_path, roots, description)
}

pub fn ensure_path_in_roots(
    path: &Path,
    roots: &[PathBuf],
    description: &str,
) -> Result<(), String> {
    if !path.exists()
        && path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "{description}에 안전하지 않은 상대 경로가 포함되어 있습니다: {}",
            path.display()
        ));
    }

    let canonical_path = canonicalize_existing_or_parent(path)
        .map_err(|error| format!("{description} 확인 실패: {} ({error})", path.display()))?;
    ensure_canonical_path_in_roots(&canonical_path, roots, description)
}

fn ensure_canonical_path_in_roots(
    canonical_path: &Path,
    roots: &[PathBuf],
    description: &str,
) -> Result<(), String> {
    let allowed_roots = roots
        .iter()
        .filter_map(|root| canonicalize_existing_or_parent(root).ok())
        .collect::<Vec<_>>();
    if allowed_roots
        .iter()
        .any(|root| path_is_inside(canonical_path, root))
    {
        return Ok(());
    }

    let roots = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "{description}가 허용된 경로 밖에 있습니다: {} (허용: {roots})",
        canonical_path.display()
    ))
}

fn canonicalize_existing_or_parent(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path);
    }

    let ancestor = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no existing ancestor for {}", path.display()),
            )
        })?;
    let relative = path.strip_prefix(ancestor).map_err(std::io::Error::other)?;
    if relative.components().any(is_unsafe_relative_component) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsafe relative path: {}", path.display()),
        ));
    }
    Ok(fs::canonicalize(ancestor)?.join(relative))
}

fn is_unsafe_relative_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

#[cfg(windows)]
fn path_is_inside(path: &Path, root: &Path) -> bool {
    let path = normalized_windows_path(path);
    let root = normalized_windows_path(root);
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(windows)]
fn normalized_windows_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

#[cfg(not(windows))]
fn path_is_inside(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_existing_path_outside_allowed_root_after_canonicalization() {
        let root = test_root("rejects_existing_path_outside_allowed_root");
        let mods = root.join("mods");
        let outside = root.join("outside");
        fs::create_dir_all(&mods).expect("create mods");
        fs::create_dir_all(&outside).expect("create outside");

        let traversal = mods.join("..").join("outside");
        let result = ensure_existing_path_in_roots(&traversal, &[mods], "test path");

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accepts_existing_path_inside_allowed_root() {
        let root = test_root("accepts_existing_path_inside_allowed_root");
        let mods = root.join("mods");
        let child = mods.join("example");
        fs::create_dir_all(&child).expect("create child");

        ensure_existing_path_in_roots(&child, &[mods], "test path").expect("path allowed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_existing_path_with_unsafe_relative_components() {
        let root = test_root("rejects_non_existing_path_with_unsafe_relative_components");
        let mods = root.join("mods");
        fs::create_dir_all(&mods).expect("create mods");

        let traversal = mods.join("missing").join("..").join("outside");
        let result = ensure_path_in_roots(&traversal, &[mods], "test path");

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn accepts_windows_case_differences() {
        let root = test_root("accepts_windows_case_differences");
        let mods = root.join("Mods");
        let child = mods.join("Example");
        fs::create_dir_all(&child).expect("create child");
        let lower_root = PathBuf::from(mods.to_string_lossy().to_ascii_lowercase());

        ensure_existing_path_in_roots(&child, &[lower_root], "test path").expect("path allowed");
        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("sts2-path-guard-{name}-{stamp}"))
    }
}
