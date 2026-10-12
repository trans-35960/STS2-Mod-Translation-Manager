fn sheet_glossary_issues(
    sheet: &JsonTranslationSheet,
    project_key: Option<&str>,
) -> Vec<JsonValidationIssueDto> {
    let mut issues = number_mismatch_issues(sheet)
        .into_iter()
        .map(|issue| JsonValidationIssueDto {
            key: issue.key,
            kind: issue.kind,
            message: issue.message,
            original: None,
            suggestion: None,
        })
        .collect::<Vec<_>>();
    let config = configured_config();
    let target_language = sheet.target_language.trim().to_lowercase();
    if target_language.is_empty() {
        return issues;
    }
    let game = config.game_pck_path().and_then(|pck| {
        load_game_glossary(&pck, &glossary_source_language(sheet), &target_language)
            .ok()
            .flatten()
    });
    let user_terms = combined_user_terms(&config, &target_language, project_key);
    issues.extend(
        glossary_issues(sheet, game.as_deref(), &user_terms)
            .into_iter()
            .map(|issue| JsonValidationIssueDto {
                key: issue.key,
                kind: GLOSSARY_ISSUE_KIND.to_string(),
                message: issue.message,
                original: issue.suggestion.is_some().then_some(issue.original),
                suggestion: issue.suggestion,
            }),
    );
    issues.extend(
        korean_josa_issues(sheet, game.as_deref(), &user_terms)
            .into_iter()
            .map(|issue| JsonValidationIssueDto {
                key: issue.key,
                kind: JOSA_ISSUE_KIND.to_string(),
                message: issue.message,
                original: Some(issue.original),
                suggestion: Some(issue.suggestion),
            }),
    );
    issues
}

pub(crate) fn load_translation_glossary(
    source_path: String,
    target_language: String,
    sample_key: Option<String>,
) -> Result<GlossaryInfoDto, String> {
    let config = configured_config();
    let target_language = target_language.trim().to_lowercase();
    let source_language = source_language_of(&source_path, sample_key.as_deref());
    let game_pck = config.game_pck_path();
    let game_terms = match game_pck.as_deref() {
        Some(pck) if !target_language.is_empty() => {
            load_game_glossary(pck, &source_language, &target_language)
                .map_err(|error| error.to_string())?
                .map_or(0, |glossary| glossary.len())
        }
        _ => 0,
    };
    let user_terms = read_user_glossary(&config.state_dir, &target_language)
        .map_err(|error| error.to_string())?;
    Ok(GlossaryInfoDto {
        source_language,
        target_language,
        game_pck_path: game_pck.as_deref().map(display_path),
        game_terms,
        user_terms: user_terms.into_iter().map(glossary_term_dto).collect(),
    })
}

pub(crate) fn save_user_glossary(
    target_language: String,
    terms: Vec<GlossaryTermDto>,
) -> Result<Vec<GlossaryTermDto>, String> {
    let config = configured_config();
    let target_language = target_language.trim().to_lowercase();
    if target_language.is_empty() {
        return Err("대상 언어가 없습니다.".to_string());
    }
    let terms = terms
        .into_iter()
        .map(|term| GlossaryTerm {
            source: term.source,
            targets: term.targets,
        })
        .collect::<Vec<_>>();
    write_user_glossary(&config.state_dir, &target_language, &terms)
        .map_err(|error| error.to_string())?;
    read_user_glossary(&config.state_dir, &target_language)
        .map(|terms| terms.into_iter().map(glossary_term_dto).collect())
        .map_err(|error| error.to_string())
}

fn glossary_term_dto(term: GlossaryTerm) -> GlossaryTermDto {
    GlossaryTermDto {
        source: term.source,
        targets: term.targets,
    }
}

/// Writes the translation JSON plus a rules guide and the glossary terms this
/// sheet uses, so an external AI agent can translate with the same rules the
/// validator checks. The result is imported back with "CSV/JSON 매칭".
pub(crate) fn export_ai_translation_kit(
    output_path: String,
    sheet: JsonSheetDto,
    only_empty: Option<bool>,
    include_keys: Option<Vec<String>>,
    project_key: Option<String>,
) -> Result<AiKitExportDto, String> {
    let json_path = PathBuf::from(output_path.trim());
    if json_path.as_os_str().is_empty() {
        return Err("AI 작업 키트 출력 경로를 선택하세요.".to_string());
    }
    let parent = json_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let domain = json_sheet_from_dto(sheet)?;
    let include_keys = include_keys.map(|keys| keys.into_iter().collect::<BTreeSet<_>>());

    let output = compact_source_translation_map_with_keys(
        &domain,
        only_empty.unwrap_or(false),
        include_keys.as_ref(),
    );
    let rows = output.values().map(BTreeMap::len).sum();
    let content = serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?;
    fs::write(&json_path, content).map_err(|error| error.to_string())?;

    let config = configured_config();
    let source_language = glossary_source_language(&domain);
    let target_language = domain.target_language.trim().to_lowercase();
    let game = config.game_pck_path().and_then(|pck| {
        load_game_glossary(&pck, &source_language, &target_language)
            .ok()
            .flatten()
    });
    let user_terms = combined_user_terms(&config, &target_language, project_key.as_deref());
    let terms = sheet_glossary_terms(&domain, game.as_deref(), &user_terms, include_keys.as_ref());

    let stem = json_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.trim_end_matches(".json").trim_end_matches(".short"))
        .unwrap_or("translation")
        .to_string();
    let glossary_path = parent.join(format!("{stem}.glossary.tsv"));
    let mut glossary = String::from("source\ttarget\n");
    for term in &terms {
        glossary.push_str(&format!(
            "{}\t{}\n",
            term.source.replace(['\t', '\n'], " "),
            term.targets.join(" / ").replace(['\t', '\n'], " ")
        ));
    }
    fs::write(&glossary_path, glossary).map_err(|error| error.to_string())?;

    let guide_path = parent.join(format!("{stem}.AI_GUIDE.md"));
    let json_name = json_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let glossary_name = glossary_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let guide = ai_translation_guide(
        json_name,
        glossary_name,
        &source_language,
        &target_language,
        rows,
        terms.len(),
    );
    fs::write(&guide_path, guide).map_err(|error| error.to_string())?;

    Ok(AiKitExportDto {
        output_path: display_path(&json_path),
        rows,
        guide_path: display_path(&guide_path),
        glossary_path: display_path(&glossary_path),
        glossary_terms: terms.len(),
    })
}

fn ai_translation_guide(
    json_name: &str,
    glossary_name: &str,
    source_language: &str,
    target_language: &str,
    rows: usize,
    glossary_terms: usize,
) -> String {
    let mut guide = format!(
        "# Slay the Spire 2 모드 번역 작업 안내\n\n\
이 폴더는 STS2 Mod Translation Manager가 만든 AI 번역 작업 키트입니다.\n\n\
- 원문 언어: `{source_language}` → 대상 언어: `{target_language}`\n\
- 번역 파일: `{json_name}` ({rows}행)\n\
- 용어집: `{glossary_name}` ({glossary_terms}개)\n\n\
## 작업 방법\n\n\
1. `{json_name}`의 구조는 `{{ \"파일\": {{ \"ID\": \"원문\" }} }}`입니다.\n\
2. 각 값(원문)을 `{target_language}` 번역으로 바꿉니다. 파일 이름과 ID는 절대 바꾸거나 빼지 않습니다.\n\
3. 결과는 같은 구조의 UTF-8 JSON으로 저장합니다. 항목이 많으면 파일 단위로 나눠 작업해도 됩니다.\n\
4. 완료한 JSON은 앱의 \"CSV/JSON 매칭\"으로 가져온 뒤 \"검증\"을 실행합니다.\n\n\
## 반드시 지킬 규칙\n\n\
1. **강조 태그**: `[gold]...[/gold]`, `[blue]...[/blue]`, `[color=#CC0000]...[/color]` 같은 대괄호 태그는 쌍과 개수를 그대로 두고 안의 텍스트만 번역합니다.\n\
2. **플레이스홀더**: `{{Damage}}`, `{{Damage:diff()}}`, `{{Amount}}` 같은 중괄호 값은 그대로 둡니다.\n\
3. **SmartFormat 선택지**: `{{Cards:plural:card|cards}}`, `{{IfUpgraded:show:A|B}}`는 첫 콜론 앞(변수 이름과 형식)은 그대로 두고, `|`로 나뉜 각 선택지 텍스트만 번역합니다. 선택지 개수는 유지합니다.\n\
4. **기타 토큰**: `!Var!` 형태의 느낌표 토큰, `<tag>` 꺾쇠 태그, `NL` 줄바꿈 마커, 실제 줄바꿈(`\\n`)의 개수와 위치를 유지합니다.\n\
5. **숫자**: 원문의 숫자와 수치는 바꾸지 않습니다.\n\
6. **용어집**: `{glossary_name}`에 있는 용어는 반드시 지정된 번역어를 사용합니다. 번역어가 `/`로 여러 개면 그중 하나를 씁니다. 원문에서 `[gold]` 등으로 강조된 게임 용어는 게임 공식 명칭과 같아야 합니다.\n\
7. **일관성**: 같은 원문 표현은 파일 전체에서 같은 번역어를 사용합니다.\n"
    );
    if is_korean_language(target_language) {
        guide.push_str(
            "\n## 한국어 규칙\n\n\
- **조사는 강조 태그 밖에** 둡니다. `[gold]취약[/gold]을` (O), `[gold]취약을[/gold]` (X)\n\
- 카드·효과 설명은 게임 공식 번역처럼 합니다체로 씁니다. 예: `피해를 6 줍니다.`, `[gold]방어도[/gold]를 5 얻습니다.`\n\
- 플레이스홀더 뒤 조사는 받침을 알 수 없으므로 `을(를)`, `이(가)`처럼 쓰거나 조사가 필요 없는 어순으로 씁니다.\n\
- 복수형 구분이 없으므로 `{Cards:plural:카드|카드}`처럼 같은 말을 반복해도 됩니다.\n",
        );
    }
    guide
}

/// Mod-specific terms first, then the language-wide user glossary.
fn combined_user_terms(
    config: &AppConfig,
    target_language: &str,
    project_key: Option<&str>,
) -> Vec<GlossaryTerm> {
    let language_terms = read_user_glossary(&config.state_dir, target_language).unwrap_or_default();
    let project_terms = project_key
        .filter(|key| !key.trim().is_empty())
        .and_then(|key| read_project_settings(&config.state_dir, key).ok())
        .map(|settings| settings.glossary_terms())
        .unwrap_or_default();
    merge_glossary_terms(&project_terms, &language_terms)
}

pub(crate) fn load_project_translation_settings(
    project_key: String,
) -> Result<ProjectTranslationSettings, String> {
    let config = configured_config();
    read_project_settings(&config.state_dir, &project_key).map_err(|error| error.to_string())
}

pub(crate) fn save_project_translation_settings(
    project_key: String,
    settings: ProjectTranslationSettings,
) -> Result<(), String> {
    let config = configured_config();
    write_project_settings(&config.state_dir, &project_key, &settings)
        .map_err(|error| error.to_string())
}

pub(crate) fn build_ai_prompt_parts(
    sheet: JsonSheetDto,
    request: AiPromptRequestDto,
) -> Result<AiPromptPartsDto, String> {
    let domain = json_sheet_from_dto(sheet)?;
    let config = configured_config();
    let source_language = glossary_source_language(&domain);
    let target_language = domain.target_language.trim().to_lowercase();
    let game = config.game_pck_path().and_then(|pck| {
        load_game_glossary(&pck, &source_language, &target_language)
            .ok()
            .flatten()
    });
    let user_terms = combined_user_terms(&config, &target_language, request.project_key.as_deref());
    let include_keys = request
        .include_keys
        .map(|keys| keys.into_iter().collect::<BTreeSet<_>>());
    let options = PromptOptions {
        include_prompt: request.include_prompt,
        max_chars: request.max_chars.unwrap_or(DEFAULT_PROMPT_MAX_CHARS),
        only_empty: request.only_empty,
        include_keys: include_keys.as_ref(),
        source_language: &source_language,
        mod_name: &request.mod_name,
        mod_description: &request.mod_description,
        tone: &request.tone,
        extra_instructions: &request.extra_instructions,
    };
    let parts = build_prompt_parts(&domain, game.as_deref(), &user_terms, &options);
    Ok(AiPromptPartsDto {
        total_rows: parts.iter().map(|part| part.rows).sum(),
        parts: parts
            .into_iter()
            .map(|part| AiPromptPartDto {
                text: part.text,
                rows: part.rows,
                chars: part.chars,
                terms: part.terms,
                oversized: part.oversized,
                slots: part
                    .slots
                    .into_iter()
                    .map(|(file, id)| AiPromptSlotDto { file, id })
                    .collect(),
            })
            .collect(),
    })
}

pub(crate) fn extract_term_candidates(
    sheet: JsonSheetDto,
    project_key: Option<String>,
) -> Result<Vec<TermCandidateDto>, String> {
    let domain = json_sheet_from_dto(sheet)?;
    let config = configured_config();
    let source_language = glossary_source_language(&domain);
    let target_language = domain.target_language.trim().to_lowercase();
    let game = config.game_pck_path().and_then(|pck| {
        load_game_glossary(&pck, &source_language, &target_language)
            .ok()
            .flatten()
    });
    let known = combined_user_terms(&config, &target_language, project_key.as_deref());
    Ok(
        term_candidates(&domain, &source_language, game.as_deref(), &known)
            .into_iter()
            .map(|candidate| TermCandidateDto {
                source: candidate.source,
                count: candidate.count,
                kinds: candidate.kinds.into_iter().map(str::to_string).collect(),
                suggestions: candidate
                    .suggestions
                    .into_iter()
                    .map(|(text, count)| TermSuggestionDto { text, count })
                    .collect(),
                shares_translation_with: candidate.shares_translation_with,
                example: candidate.example,
            })
            .collect(),
    )
}

pub(crate) fn build_term_prompt(
    sheet: JsonSheetDto,
    request: TermPromptRequestDto,
) -> Result<AiPromptPartsDto, String> {
    let domain = json_sheet_from_dto(sheet)?;
    let source_language = glossary_source_language(&domain);
    let terms = request
        .terms
        .iter()
        .map(|term| TermToTranslate {
            source: &term.source,
            example: &term.example,
        })
        .collect::<Vec<_>>();
    let options = PromptOptions {
        include_prompt: true,
        max_chars: request.max_chars.unwrap_or(DEFAULT_PROMPT_MAX_CHARS),
        only_empty: false,
        include_keys: None,
        source_language: &source_language,
        mod_name: &request.mod_name,
        mod_description: &request.mod_description,
        tone: &request.tone,
        extra_instructions: &request.extra_instructions,
    };
    let parts = build_term_prompt_parts(&terms, &domain.target_language, &options);
    Ok(AiPromptPartsDto {
        total_rows: parts.iter().map(|part| part.rows).sum(),
        parts: parts
            .into_iter()
            .map(|part| AiPromptPartDto {
                text: part.text,
                rows: part.rows,
                chars: part.chars,
                terms: part.terms,
                oversized: part.oversized,
                slots: Vec::new(),
            })
            .collect(),
    })
}

/// Writes prompt parts for pasting or uploading elsewhere: `txt` puts every
/// part in one file, `txt_parts` and `pdf` write one file per part next to
/// `output_path` (`name.part01.txt`). Returns the written paths.
pub(crate) fn export_ai_prompt_parts(
    output_path: String,
    parts: Vec<String>,
    format: String,
    font_size: Option<f32>,
) -> Result<Vec<String>, String> {
    let output_path = PathBuf::from(output_path.trim());
    if output_path.as_os_str().is_empty() {
        return Err("내보낼 경로를 선택하세요.".to_string());
    }
    if parts.is_empty() {
        return Err("내보낼 파트가 없습니다.".to_string());
    }
    let parent = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let total = parts.len();

    if format == "txt" {
        let content = parts
            .iter()
            .enumerate()
            .map(|(index, text)| format!("===== 파트 {}/{total} =====\n\n{}\n", index + 1, text.trim_end()))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&output_path, content).map_err(|error| error.to_string())?;
        return Ok(vec![display_path(&output_path)]);
    }

    let (extension, pdf) = match format.as_str() {
        "txt_parts" => ("txt", false),
        "pdf" => ("pdf", true),
        other => return Err(format!("알 수 없는 내보내기 형식: {other}")),
    };
    let stem = output_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("prompt")
        .to_string();
    let digits = total.to_string().len().max(2);
    let mut written = Vec::with_capacity(total);
    for (index, text) in parts.iter().enumerate() {
        let path = parent.join(format!("{stem}.part{:0digits$}.{extension}", index + 1));
        if pdf {
            let bytes = crate::prompt_pdf::render_text_pdf(
                text,
                font_size.unwrap_or(crate::prompt_pdf::DEFAULT_PDF_FONT_SIZE),
            )?;
            fs::write(&path, bytes).map_err(|error| error.to_string())?;
        } else {
            fs::write(&path, text).map_err(|error| error.to_string())?;
        }
        written.push(display_path(&path));
    }
    Ok(written)
}
