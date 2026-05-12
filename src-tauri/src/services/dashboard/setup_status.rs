fn setup_issues(
    config: &AppConfig,
    settings: &UiSettingsDto,
    launch: &LaunchStatus,
) -> Vec<SetupIssueDto> {
    let mut issues = Vec::new();

    if !launch.ready {
        issues.push(SetupIssueDto {
            field: "game_exe_path".to_string(),
            message: "게임 실행 파일 또는 Steam 실행 경로를 자동 탐지하지 못했습니다.".to_string(),
            blocking: true,
        });
    } else if !settings.game_exe_path.trim().is_empty()
        && !PathBuf::from(settings.game_exe_path.trim()).is_file()
    {
        issues.push(SetupIssueDto {
            field: "game_exe_path".to_string(),
            message: "설정된 게임 실행 파일을 찾을 수 없습니다.".to_string(),
            blocking: true,
        });
    }

    if settings.save_dir.trim().is_empty() {
        issues.push(SetupIssueDto {
            field: "save_dir".to_string(),
            message: "세이브 폴더를 자동 탐지하지 못했습니다.".to_string(),
            blocking: true,
        });
    } else if !PathBuf::from(settings.save_dir.trim()).is_dir() {
        issues.push(SetupIssueDto {
            field: "save_dir".to_string(),
            message: "세이브 폴더 경로가 존재하지 않습니다.".to_string(),
            blocking: true,
        });
    }

    if settings.save_backup_dir.trim().is_empty() {
        issues.push(SetupIssueDto {
            field: "save_backup_dir".to_string(),
            message: "세이브 백업 경로가 비어 있습니다.".to_string(),
            blocking: true,
        });
    } else {
        let backup_dir = PathBuf::from(settings.save_backup_dir.trim());
        if backup_dir.exists() && !backup_dir.is_dir() {
            issues.push(SetupIssueDto {
                field: "save_backup_dir".to_string(),
                message: "세이브 백업 경로가 폴더가 아닙니다.".to_string(),
                blocking: true,
            });
        }
    }

    if settings.translation_work_dir.trim().is_empty() {
        issues.push(SetupIssueDto {
            field: "translation_work_dir".to_string(),
            message: "번역/추출 작업 경로가 비어 있습니다.".to_string(),
            blocking: false,
        });
    } else {
        let work_dir = PathBuf::from(settings.translation_work_dir.trim());
        if work_dir.exists() && !work_dir.is_dir() {
            issues.push(SetupIssueDto {
                field: "translation_work_dir".to_string(),
                message: "번역/추출 작업 경로가 폴더가 아닙니다.".to_string(),
                blocking: false,
            });
        }
    }

    if config.save_backup_max_entries == 0 {
        issues.push(SetupIssueDto {
            field: "save_backup_max_entries".to_string(),
            message: "세이브 백업 최대 개수는 1개 이상이어야 합니다.".to_string(),
            blocking: true,
        });
    }

    issues.extend(path_overlap_issues(config, settings));

    issues
}

#[derive(Debug, Clone)]
struct CheckedPath {
    field: &'static str,
    label: &'static str,
    path: PathBuf,
}

fn path_overlap_issues(config: &AppConfig, settings: &UiSettingsDto) -> Vec<SetupIssueDto> {
    let paths = checked_paths(config, settings);
    let mut issues = Vec::new();

    for (index, left) in paths.iter().enumerate() {
        for right in paths.iter().skip(index + 1) {
            let Some(kind) = path_overlap_kind(&left.path, &right.path) else {
                continue;
            };
            issues.push(SetupIssueDto {
                field: left.field.to_string(),
                message: format!(
                    "{}와 {} 경로가 {}. 정리/복구 작업이 다른 데이터를 건드릴 수 있으니 분리하는 것을 권장합니다. ({} / {})",
                    left.label,
                    right.label,
                    kind,
                    left.path.display(),
                    right.path.display()
                ),
                blocking: false,
            });
        }
    }

    issues
}

fn checked_paths(config: &AppConfig, settings: &UiSettingsDto) -> Vec<CheckedPath> {
    let mut paths = vec![
        CheckedPath {
            field: "game_mods_dir",
            label: "게임 모드 폴더",
            path: config.game_mods_dir.clone(),
        },
        CheckedPath {
            field: "disabled_mods_dir",
            label: "비활성 모드 저장소",
            path: game_disabled_dir(&config.game_mods_dir),
        },
    ];

    push_checked_setting_path(
        &mut paths,
        "save_dir",
        "세이브 폴더",
        settings.save_dir.trim(),
    );
    push_checked_setting_path(
        &mut paths,
        "save_backup_dir",
        "세이브 백업 경로",
        settings.save_backup_dir.trim(),
    );
    push_checked_setting_path(
        &mut paths,
        "translation_work_dir",
        "번역/추출 작업 경로",
        settings.translation_work_dir.trim(),
    );

    paths
}

fn push_checked_setting_path(
    paths: &mut Vec<CheckedPath>,
    field: &'static str,
    label: &'static str,
    value: &str,
) {
    if value.is_empty() {
        return;
    }
    paths.push(CheckedPath {
        field,
        label,
        path: PathBuf::from(value),
    });
}

fn path_overlap_kind(left: &Path, right: &Path) -> Option<&'static str> {
    let left = comparable_path(left);
    let right = comparable_path(right);
    if left.is_empty() || right.is_empty() {
        return None;
    }
    if left == right {
        return Some("같습니다");
    }
    if is_path_ancestor(&left, &right) || is_path_ancestor(&right, &left) {
        return Some("포함 관계입니다");
    }
    None
}

fn comparable_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn is_path_ancestor(parent: &str, child: &str) -> bool {
    child
        .strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn game_updated_epoch(launch: &LaunchStatus, config: &AppConfig) -> Option<u64> {
    let path = launch
        .game_exe
        .clone()
        .or_else(|| config.game_exe_path.clone())
        .or_else(|| sts2_mod_manager::launcher::resolve_game_exe(config))?;
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| epoch_seconds(Some(time)))
}

fn mod_safety_warnings(_manifest: &ModManifestInfo) -> Vec<String> {
    Vec::new()
}

fn nested_mod_payload_dir(path: &Path) -> Option<PathBuf> {
    let children = fs::read_dir(path)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|child| {
            child
                .file_name()
                .map(|name| !name.to_string_lossy().starts_with('.'))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let dirs = children
        .iter()
        .filter(|child| child.is_dir())
        .collect::<Vec<_>>();
    let files = children
        .iter()
        .filter(|child| child.is_file())
        .collect::<Vec<_>>();
    if dirs.len() != 1 || !files.is_empty() {
        return None;
    }
    let inner = dirs[0].to_path_buf();
    looks_like_mod_payload(&inner).then_some(inner)
}

fn is_vortex_nested_mod_layout(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if !vortex_deployment_marker_exists(parent) {
        return false;
    }
    let Some(inner) = nested_mod_payload_dir(path) else {
        return false;
    };
    let outer_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let inner_name = inner
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    outer_name == inner_name || looks_like_vortex_download_folder(&outer_name)
}

fn vortex_deployment_marker_exists(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            name.starts_with("vortex.deployment.") && (name.ends_with(".json") || name.ends_with(".msgpack"))
        })
}

fn looks_like_vortex_download_folder(name: &str) -> bool {
    name.rsplit_once('-')
        .is_some_and(|(_, suffix)| suffix.len() >= 8 && suffix.chars().all(|character| character.is_ascii_digit()))
}

fn looks_like_mod_payload(path: &Path) -> bool {
    if preferred_extractable_payload(path).is_some() {
        return true;
    }
    let mut manifests = Vec::new();
    collect_manifest_candidates(path, path, &mut manifests);
    !manifests.is_empty()
}


