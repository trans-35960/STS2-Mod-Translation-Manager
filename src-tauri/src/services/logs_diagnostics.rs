pub(crate) fn read_game_logs() -> Result<Vec<GameLogDto>, String> {
    let config = AppConfig::from_workspace(resolve_workspace_dir());
    let settings = read_ui_settings(&config).map_err(|error| error.to_string())?;
    Ok(game_log_candidates(&config, settings.game_log_path.trim())
        .into_iter()
        .map(read_game_log)
        .collect())
}


fn troubleshoot_diagnostics(
    config: &AppConfig,
    settings: &UiSettingsDto,
    launch: &LaunchStatus,
    summary: &ScanSummary,
    mods: &[ModRowDto],
    setup_issues: &[SetupIssueDto],
) -> Vec<TroubleshootDiagnosticDto> {
    let mut diagnostics = Vec::new();

    for issue in setup_issues {
        diagnostics.push(TroubleshootDiagnosticDto {
            id: format!("setup-{}", issue.field),
            category: "setup".to_string(),
            severity: if issue.blocking { "error" } else { "warn" }.to_string(),
            title: if issue.blocking {
                "필수 경로 확인 필요".to_string()
            } else {
                "권장 설정 확인 필요".to_string()
            },
            detail: issue.message.clone(),
            action_label: "설정에서 경로 지정".to_string(),
            related_path: String::new(),
            mod_key: None,
            can_auto_fix: false,
        });
    }

    if !config.game_mods_dir.exists() {
        diagnostics.push(TroubleshootDiagnosticDto {
            id: "setup-missing-mods-dir".to_string(),
            category: "setup".to_string(),
            severity: "warn".to_string(),
            title: "mods 폴더가 아직 없습니다".to_string(),
            detail: "게임 설치는 감지했지만 활성 모드 폴더가 없어 첫 설치 전 상태로 보입니다."
                .to_string(),
            action_label: "게임 폴더 열기".to_string(),
            related_path: display_path(&config.game_dir),
            mod_key: None,
            can_auto_fix: false,
        });
    }

    for record in &summary.game_mods {
        if record.kind == ModKind::Archive && is_supported_archive_path(&record.path) {
            diagnostics.push(TroubleshootDiagnosticDto {
                id: format!("install-archive-{}", record.stable_key()),
                category: "install".to_string(),
                severity: "warn".to_string(),
                title: "압축 파일이 mods 폴더에 그대로 있습니다".to_string(),
                detail: format!(
                    "{}은(는) 게임이 바로 읽기 어려운 압축 파일입니다. 폴더로 풀고 원본 압축은 백업해야 합니다.",
                    record.name
                ),
                action_label: "설치 자동 정리".to_string(),
                related_path: display_path(&record.path),
                mod_key: Some(record.stable_key()),
                can_auto_fix: true,
            });
        }
        if record.kind == ModKind::Directory
            && nested_mod_payload_dir(&record.path).is_some()
            && !is_vortex_nested_mod_layout(&record.path)
        {
            diagnostics.push(TroubleshootDiagnosticDto {
                id: format!("install-nested-{}", record.stable_key()),
                category: "install".to_string(),
                severity: "warn".to_string(),
                title: "모드 폴더가 한 단계 더 중첩되어 있습니다".to_string(),
                detail: format!(
                    "{} 폴더 안에 실제 모드 폴더가 한 번 더 들어 있습니다. 로더가 모드를 놓칠 수 있습니다.",
                    record.name
                ),
                action_label: "설치 자동 정리".to_string(),
                related_path: display_path(&record.path),
                mod_key: Some(record.stable_key()),
                can_auto_fix: true,
            });
        }
    }

    for mod_row in mods {
        for dependency in &mod_row.dependencies {
            if !dependency.available {
                diagnostics.push(TroubleshootDiagnosticDto {
                    id: format!("dependency-{}-{}", mod_row.key, dependency.id),
                    category: "dependency".to_string(),
                    severity: "error".to_string(),
                    title: "선행 모드가 없습니다".to_string(),
                    detail: format!(
                        "{} 모드는 {} 선행 모드가 필요합니다.",
                        mod_row.name, dependency.id
                    ),
                    action_label: "모드 경로 열기".to_string(),
                    related_path: mod_row.path.clone(),
                    mod_key: Some(mod_row.key.clone()),
                    can_auto_fix: false,
                });
            }
            if dependency.version_matches == Some(false) {
                diagnostics.push(TroubleshootDiagnosticDto {
                    id: format!("dependency-version-{}-{}", mod_row.key, dependency.id),
                    category: "dependency".to_string(),
                    severity: "warn".to_string(),
                    title: "선행 모드 버전이 다릅니다".to_string(),
                    detail: format!(
                        "{} 모드는 {} {} 버전을 기준으로 만들어졌지만 현재 버전은 {}입니다.",
                        mod_row.name,
                        dependency.name,
                        dependency.version_required.as_deref().unwrap_or("-"),
                        dependency.version_current.as_deref().unwrap_or("-")
                    ),
                    action_label: "모드 경로 열기".to_string(),
                    related_path: mod_row.path.clone(),
                    mod_key: Some(mod_row.key.clone()),
                    can_auto_fix: false,
                });
            }
        }
        if mod_row.needs_recheck {
            diagnostics.push(TroubleshootDiagnosticDto {
                id: format!("game-recheck-{}", mod_row.key),
                category: "update".to_string(),
                severity: "warn".to_string(),
                title: "게임 업데이트 후 재검증 필요".to_string(),
                detail: format!(
                    "{}은(는) 게임 실행 파일보다 오래된 모드 상태입니다. 업데이트 이후 정상 실행을 한 번 확인하세요.",
                    mod_row.name
                ),
                action_label: "모드 경로 열기".to_string(),
                related_path: mod_row.path.clone(),
                mod_key: Some(mod_row.key.clone()),
                can_auto_fix: false,
            });
        }
        if mod_row.translation_review_required {
            diagnostics.push(TroubleshootDiagnosticDto {
                id: format!("translation-review-{}", mod_row.key),
                category: "translation".to_string(),
                severity: "warn".to_string(),
                title: "번역 적용본 재검토 필요".to_string(),
                detail: format!(
                    "{}의 번역 적용 시점보다 게임 파일이 더 최근입니다.",
                    mod_row.name
                ),
                action_label: "번역 도구에서 확인".to_string(),
                related_path: mod_row.extraction_source_path.clone(),
                mod_key: Some(mod_row.key.clone()),
                can_auto_fix: false,
            });
        }
    }

    diagnostics.extend(log_diagnostics(config, settings, mods));

    if diagnostics.is_empty() && launch.ready {
        diagnostics.push(TroubleshootDiagnosticDto {
            id: "healthy-ready".to_string(),
            category: "setup".to_string(),
            severity: "good".to_string(),
            title: "기본 진단 통과".to_string(),
            detail:
                "설치 경로, 실행 대상, 세이브 백업 설정에서 즉시 막히는 문제를 찾지 못했습니다."
                    .to_string(),
            action_label: "새로고침".to_string(),
            related_path: String::new(),
            mod_key: None,
            can_auto_fix: false,
        });
    }

    diagnostics.sort_by_key(|item| diagnostic_sort_key(&item.severity, &item.category));
    diagnostics.dedup_by(|left, right| left.id == right.id);
    diagnostics.truncate(24);
    diagnostics
}

fn diagnostic_sort_key(severity: &str, category: &str) -> String {
    let severity_rank = match severity {
        "error" => 0,
        "warn" => 1,
        "good" => 3,
        _ => 2,
    };
    format!("{severity_rank}-{category}")
}

fn log_diagnostics(
    config: &AppConfig,
    settings: &UiSettingsDto,
    mods: &[ModRowDto],
) -> Vec<TroubleshootDiagnosticDto> {
    let mut output = Vec::new();
    let Some(log) = game_log_candidates(config, settings.game_log_path.trim())
        .into_iter()
        .find(|path| path.is_file())
    else {
        return output;
    };
    let Ok(lines) = read_tail_lines(&log) else {
        return output;
    };
    let current_run_paths =
        save_backup::modded_current_run_paths_for_mode_switch(config).unwrap_or_default();
    for line in lines.iter().rev().take(180) {
        if let Some((category, title, detail)) = classify_game_log_line(line) {
            let save_fix = category == "current-run";
            if save_fix && current_run_paths.is_empty() {
                continue;
            }
            let detail = if save_fix {
                current_run_paths
                    .first()
                    .map(|path| format!("남은 진행 런: {}", path.display()))
                    .unwrap_or(detail)
            } else {
                detail
            };
            let mod_key = mods
                .iter()
                .find(|row| {
                    let haystack = line.to_ascii_lowercase();
                    haystack.contains(&row.key.to_ascii_lowercase())
                        || haystack.contains(&row.name.to_ascii_lowercase())
                })
                .map(|row| row.key.clone());
            output.push(TroubleshootDiagnosticDto {
                id: format!("log-{}-{:016x}", category, stable_hash(line)),
                category: if save_fix { "safety" } else { "log" }.to_string(),
                severity: "error".to_string(),
                title,
                detail,
                action_label: if save_fix {
                    "current_run 정리".to_string()
                } else {
                    "로그 위치 열기".to_string()
                },
                related_path: display_path(&log),
                mod_key,
                can_auto_fix: save_fix,
            });
        }
    }
    output.truncate(6);
    output
}

fn classify_game_log_line(line: &str) -> Option<(&'static str, String, String)> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("modelnotfoundexception")
        || (lower.contains("model id=") && lower.contains(" not found"))
    {
        return Some((
            "current-run",
            "진행 중 런이 현재 모드 구성과 충돌할 수 있습니다".to_string(),
            line.trim().to_string(),
        ));
    }
    if lower.contains("pck") && any_keyword(&lower, &["error", "failed", "invalid", "corrupt"]) {
        return Some((
            "pck",
            "PCK 로딩 오류가 감지되었습니다".to_string(),
            line.trim().to_string(),
        ));
    }
    if any_keyword(
        &lower,
        &[
            "missing dependency",
            "dependency missing",
            "could not find dependency",
            "requires",
        ],
    ) {
        return Some((
            "dependency",
            "누락된 선행 모드 로그가 있습니다".to_string(),
            line.trim().to_string(),
        ));
    }
    if any_keyword(
        &lower,
        &[
            "access is denied",
            "being used by another process",
            "sharing violation",
            "permission denied",
        ],
    ) {
        return Some((
            "locked",
            "파일 잠금 또는 권한 문제가 감지되었습니다".to_string(),
            line.trim().to_string(),
        ));
    }
    if any_keyword(&lower, &["mod", "workshop", "loader"])
        && any_keyword(&lower, &["error", "failed", "exception"])
    {
        return Some((
            "mod",
            "모드 로딩 실패 로그가 있습니다".to_string(),
            line.trim().to_string(),
        ));
    }
    None
}

fn any_keyword(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

fn game_log_candidates(config: &AppConfig, configured_log_path: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if !configured_log_path.is_empty() {
        push_unique_path(&mut candidates, PathBuf::from(configured_log_path));
    }

    if let Some(default_log) = default_sts2_game_log_path() {
        push_unique_path(&mut candidates, default_log);
    }

    push_unique_path(
        &mut candidates,
        config.game_dir.join("logs").join("godot.log"),
    );
    push_unique_path(&mut candidates, config.game_dir.join("godot.log"));

    candidates
}

fn read_game_log(path: PathBuf) -> GameLogDto {
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => GameLogDto {
            path: display_path(&path),
            exists: true,
            modified_epoch: epoch_seconds(metadata.modified().ok()),
            bytes: metadata.len(),
            lines: read_tail_lines(&path)
                .unwrap_or_else(|error| vec![format!("로그를 읽지 못했습니다: {error}")]),
        },
        _ => GameLogDto {
            path: display_path(&path),
            exists: false,
            modified_epoch: None,
            bytes: 0,
            lines: Vec::new(),
        },
    }
}

fn read_tail_lines(path: &Path) -> Result<Vec<String>, String> {
    const TAIL_BYTES: u64 = 256 * 1024;
    const MAX_LINES: usize = 260;

    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let len = file.metadata().map_err(|error| error.to_string())?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text
        .lines()
        .rev()
        .take(MAX_LINES)
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    lines.reverse();
    Ok(lines)
}


