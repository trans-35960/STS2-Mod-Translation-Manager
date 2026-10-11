fn sheet_glossary_issues(sheet: &JsonTranslationSheet) -> Vec<JsonValidationIssueDto> {
    let config = configured_config();
    let target_language = sheet.target_language.trim().to_lowercase();
    if target_language.is_empty() {
        return Vec::new();
    }
    let game = config.game_pck_path().and_then(|pck| {
        load_game_glossary(&pck, &glossary_source_language(sheet), &target_language)
            .ok()
            .flatten()
    });
    let user_terms = read_user_glossary(&config.state_dir, &target_language).unwrap_or_default();
    glossary_issues(sheet, game.as_deref(), &user_terms)
        .into_iter()
        .map(|issue| JsonValidationIssueDto {
            key: issue.key,
            kind: issue.kind,
            message: issue.message,
        })
        .collect()
}

pub(crate) fn load_translation_glossary(
    source_path: String,
    target_language: String,
) -> Result<GlossaryInfoDto, String> {
    let config = configured_config();
    let target_language = target_language.trim().to_lowercase();
    let probe = JsonTranslationSheet {
        source_path,
        target_language: target_language.clone(),
        updated_epoch: 0,
        entries: Vec::new(),
    };
    let source_language = glossary_source_language(&probe);
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
