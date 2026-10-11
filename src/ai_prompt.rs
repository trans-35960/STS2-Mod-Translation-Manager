//! Copy-paste prompts for translating a sheet in a web AI chat.
//!
//! A sheet is split into parts that each stay under a character budget
//! (instructions + that part's glossary + data), so every part fits one chat
//! message. The AI answers with the same `{file: {id: text}}` JSON, which the
//! sheet accepts as a paste and matches by slot id.

use crate::error::{AppError, AppResult};
use crate::glossary::{GameGlossary, GlossaryTerm, entry_glossary_terms, is_korean_language};
use crate::json_translation::{JsonTranslationSheet, translation_slot_entries};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROMPT_MAX_CHARS: usize = 5000;
const MIN_PROMPT_MAX_CHARS: usize = 500;
/// Upper bound used while sizing parts, before the real part count is known.
const PART_LABEL_RESERVE: &str = "999/999";

/// Per-mod translation context kept between sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectTranslationSettings {
    pub mod_description: String,
    pub tone: String,
    pub terms: Vec<ProjectTerm>,
    /// Free-form instructions appended to every prompt for this mod.
    pub extra_instructions: String,
    /// Last prompt options used for this mod.
    pub include_prompt: Option<bool>,
    pub only_empty: Option<bool>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTerm {
    pub source: String,
    pub targets: Vec<String>,
}

impl ProjectTranslationSettings {
    pub fn glossary_terms(&self) -> Vec<GlossaryTerm> {
        self.terms
            .iter()
            .filter(|term| !term.source.trim().is_empty() && !term.targets.is_empty())
            .map(|term| GlossaryTerm {
                source: term.source.trim().to_string(),
                targets: term.targets.clone(),
            })
            .collect()
    }
}

pub fn project_settings_path(state_dir: &Path, project_key: &str) -> Option<PathBuf> {
    let safe = project_key
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .take(120)
        .collect::<String>();
    let safe = safe.trim_matches('.');
    (!safe.is_empty()).then(|| state_dir.join("projects").join(format!("{safe}.json")))
}

pub fn read_project_settings(
    state_dir: &Path,
    project_key: &str,
) -> AppResult<ProjectTranslationSettings> {
    let Some(path) = project_settings_path(state_dir, project_key) else {
        return Ok(ProjectTranslationSettings::default());
    };
    if !path.exists() {
        return Ok(ProjectTranslationSettings::default());
    }
    let content = fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?;
    serde_json::from_str(&content)
        .map_err(|error| AppError::InvalidCommand(format!("{}: {error}", path.display())))
}

pub fn write_project_settings(
    state_dir: &Path,
    project_key: &str,
    settings: &ProjectTranslationSettings,
) -> AppResult<()> {
    let Some(path) = project_settings_path(state_dir, project_key) else {
        return Err(AppError::InvalidCommand(
            "프로젝트 키가 없습니다.".to_string(),
        ));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    fs::write(&path, content).map_err(|source| AppError::io(&path, source))
}

pub struct PromptOptions<'a> {
    pub include_prompt: bool,
    pub max_chars: usize,
    pub only_empty: bool,
    pub include_keys: Option<&'a BTreeSet<String>>,
    pub source_language: &'a str,
    pub mod_name: &'a str,
    pub mod_description: &'a str,
    pub tone: &'a str,
    pub extra_instructions: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPart {
    pub text: String,
    pub rows: usize,
    pub chars: usize,
    pub terms: usize,
    /// A single entry is larger than the budget on its own.
    pub oversized: bool,
    /// `(compact file, slot id)` of every entry in the part, so pasted
    /// answers can be checked against it.
    pub slots: Vec<(String, String)>,
}

struct PromptRow<'a> {
    file: String,
    id: String,
    source: &'a str,
    terms: Vec<&'a GlossaryTerm>,
}

/// Splits the sheet into parts that each fit `max_chars` (counted in
/// characters), never splitting an entry.
pub fn build_prompt_parts(
    sheet: &JsonTranslationSheet,
    game: Option<&GameGlossary>,
    user_terms: &[GlossaryTerm],
    options: &PromptOptions<'_>,
) -> Vec<PromptPart> {
    let max_chars = options.max_chars.max(MIN_PROMPT_MAX_CHARS);
    let rows = translation_slot_entries(sheet)
        .into_iter()
        .filter(|slot| {
            options
                .include_keys
                .is_none_or(|keys| keys.contains(&slot.entry.key))
        })
        .filter(|slot| !options.only_empty || slot.entry.translated_value.is_empty())
        .map(|slot| PromptRow {
            terms: if options.include_prompt {
                entry_glossary_terms(&slot.entry.source_value, game, user_terms)
            } else {
                Vec::new()
            },
            file: slot.compact_file,
            id: slot.id,
            source: &slot.entry.source_value,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Vec::new();
    }

    let header_chars = render_part(sheet, options, &[], &[], PART_LABEL_RESERVE, true)
        .chars()
        .count();
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    let mut size = header_chars;
    let mut files = HashSet::new();
    let mut terms = HashSet::new();
    for (index, row) in rows.iter().enumerate() {
        let mut cost = row_chars(row);
        if !files.contains(row.file.as_str()) {
            cost += file_block_chars(&row.file);
        }
        let new_terms = row
            .terms
            .iter()
            .filter(|term| !terms.contains(&term.source.to_lowercase()))
            .collect::<Vec<_>>();
        if !new_terms.is_empty() && terms.is_empty() {
            cost += glossary_heading().chars().count();
        }
        cost += new_terms
            .iter()
            .map(|term| term_line(term).chars().count())
            .sum::<usize>();

        if index > start && size + cost > max_chars {
            groups.push(start..index);
            start = index;
            files.clear();
            terms.clear();
            size = header_chars + row_chars(row) + file_block_chars(&row.file);
            if !row.terms.is_empty() {
                size += glossary_heading().chars().count()
                    + row
                        .terms
                        .iter()
                        .map(|term| term_line(term).chars().count())
                        .sum::<usize>();
            }
        } else {
            size += cost;
        }
        files.insert(row.file.as_str());
        terms.extend(row.terms.iter().map(|term| term.source.to_lowercase()));
    }
    groups.push(start..rows.len());

    let total = groups.len();
    groups
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let part_rows = &rows[range];
            let mut seen = HashSet::new();
            let part_terms = part_rows
                .iter()
                .flat_map(|row| row.terms.iter().copied())
                .filter(|term| seen.insert(term.source.to_lowercase()))
                .collect::<Vec<_>>();
            let label = format!("{}/{total}", index + 1);
            let text = render_part(sheet, options, part_rows, &part_terms, &label, total > 1);
            let chars = text.chars().count();
            PromptPart {
                rows: part_rows.len(),
                chars,
                terms: part_terms.len(),
                oversized: part_rows.len() == 1 && chars > max_chars,
                slots: part_rows
                    .iter()
                    .map(|row| (row.file.clone(), row.id.clone()))
                    .collect(),
                text,
            }
        })
        .collect()
}

fn row_chars(row: &PromptRow<'_>) -> usize {
    // `    "id": "source",\n`
    json_string(&row.id).chars().count() + json_string(row.source).chars().count() + 8
}

fn file_block_chars(file: &str) -> usize {
    // `  "file": {\n` ... `  },\n`
    json_string(file).chars().count() + 12
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn glossary_heading() -> &'static str {
    "\n## 용어집 (반드시 이 번역어 사용)\n"
}

fn term_line(term: &GlossaryTerm) -> String {
    format!("- {} → {}\n", term.source, term.targets.join(" / "))
}

fn render_part(
    sheet: &JsonTranslationSheet,
    options: &PromptOptions<'_>,
    rows: &[PromptRow<'_>],
    terms: &[&GlossaryTerm],
    part_label: &str,
    multipart: bool,
) -> String {
    let mut data = BTreeMap::<&str, BTreeMap<&str, &str>>::new();
    for row in rows {
        data.entry(row.file.as_str())
            .or_default()
            .insert(row.id.as_str(), row.source);
    }
    let json = serde_json::to_string_pretty(&data).unwrap_or_default();
    if !options.include_prompt {
        return format!("{json}\n");
    }

    let target = sheet.target_language.trim();
    let korean = is_korean_language(target);
    let mut text = String::new();
    text.push_str("너는 Slay the Spire 2 모드 전문 번역가다.\n\n## 작업\n");
    text.push_str(&format!(
        "- 원문 언어 `{}` → 대상 언어 `{target}`.\n",
        options.source_language
    ));
    if multipart {
        text.push_str(&format!(
            "- 이 요청은 전체 중 {part_label} 파트다. 이 파트만 번역한다.\n"
        ));
    }
    text.push_str(
        "- 아래 JSON의 각 값(원문)을 번역해 **같은 구조의 JSON만** ```json 코드 블록으로 출력한다. 설명은 쓰지 않는다.\n\
- 파일 이름과 ID(키)는 바꾸거나 빼지 않는다.\n",
    );

    let mod_name = options.mod_name.trim();
    let description = options.mod_description.trim();
    if !mod_name.is_empty() || !description.is_empty() {
        text.push_str("\n## 모드 정보\n");
        if !mod_name.is_empty() {
            text.push_str(&format!("- 모드: {mod_name}\n"));
        }
        if !description.is_empty() {
            text.push_str(&format!("- 배경: {}\n", description.replace('\n', " ")));
        }
    }

    text.push_str("\n## 어조\n");
    text.push_str(&format!("- {}\n", tone_instruction(options.tone, korean)));

    text.push_str(
        "\n## 서식 규칙\n\
- `[gold]...[/gold]` 같은 대괄호 태그는 쌍과 개수를 유지하고 안의 텍스트만 번역한다.\n\
- `{Damage}`, `{Damage:diff()}` 같은 중괄호 값은 그대로 둔다. `{X:plural:a|b}`, `{X:show:a|b}`는 `|`로 나뉜 선택지 텍스트만 번역하고 개수를 유지한다.\n\
- `!Var!` 토큰, `<tag>`, `NL` 마커, 줄바꿈 수, 숫자는 그대로 둔다.\n",
    );
    if korean {
        text.push_str(
            "- 조사는 강조 태그 밖에 둔다: `[gold]취약[/gold]을` (O), `[gold]취약을[/gold]` (X).\n\
- 플레이스홀더 뒤 조사는 `을(를)`, `이(가)`처럼 쓴다.\n",
        );
    }

    let extra = options.extra_instructions.trim();
    if !extra.is_empty() {
        text.push_str(
            "
## 추가 지시
",
        );
        for line in extra.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if line.starts_with('-') {
                text.push_str(&format!(
                    "{line}
"
                ));
            } else {
                text.push_str(&format!(
                    "- {line}
"
                ));
            }
        }
    }

    if !terms.is_empty() {
        text.push_str(glossary_heading());
        for term in terms {
            text.push_str(&term_line(term));
        }
    }

    text.push_str("\n## 번역할 JSON\n```json\n");
    text.push_str(&json);
    text.push_str("\n```\n");
    text
}

/// One term to translate before the main pass, with a source line for context.
pub struct TermToTranslate<'a> {
    pub source: &'a str,
    pub example: &'a str,
}

/// Prompts that ask for translations of the given terms only, so they can be
/// fixed in the mod glossary before the sheet is translated. Split to the
/// character budget like the main prompts.
pub fn build_term_prompt_parts(
    terms: &[TermToTranslate<'_>],
    target_language: &str,
    options: &PromptOptions<'_>,
) -> Vec<PromptPart> {
    if terms.is_empty() {
        return Vec::new();
    }
    let max_chars = options.max_chars.max(MIN_PROMPT_MAX_CHARS);
    let header_chars = render_term_part(&[], target_language, options, PART_LABEL_RESERVE, true)
        .chars()
        .count();
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    let mut size = header_chars;
    for (index, term) in terms.iter().enumerate() {
        let cost = term_cost(term);
        if index > start && size + cost > max_chars {
            groups.push(start..index);
            start = index;
            size = header_chars + cost;
        } else {
            size += cost;
        }
    }
    groups.push(start..terms.len());

    let total = groups.len();
    groups
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let part_terms = &terms[range];
            let label = format!("{}/{total}", index + 1);
            let text = render_term_part(part_terms, target_language, options, &label, total > 1);
            let chars = text.chars().count();
            PromptPart {
                rows: part_terms.len(),
                chars,
                terms: part_terms.len(),
                oversized: part_terms.len() == 1 && chars > max_chars,
                slots: Vec::new(),
                text,
            }
        })
        .collect()
}

fn term_cost(term: &TermToTranslate<'_>) -> usize {
    // `- source: example\n` plus `  "source": "",\n` in the template.
    term.source.chars().count() * 2 + term.example.chars().count() + 14
}

fn render_term_part(
    terms: &[TermToTranslate<'_>],
    target_language: &str,
    options: &PromptOptions<'_>,
    part_label: &str,
    multipart: bool,
) -> String {
    let target = target_language.trim();
    let korean = is_korean_language(target);
    let mut text = String::from("너는 Slay the Spire 2 모드 전문 번역가다.\n\n## 작업\n");
    text.push_str(&format!(
        "- 아래 모드 용어를 원문 언어 `{}` → 대상 언어 `{target}`로 번역해 용어집을 만든다. 이 번역어는 이후 본문 번역 전체에 그대로 쓰인다.\n",
        options.source_language
    ));
    if multipart {
        text.push_str(&format!("- 이 요청은 전체 중 {part_label} 파트다.\n"));
    }
    text.push_str(
        "- 캐릭터·지명 같은 고유명사는 음역하고, 게임 메커니즘 키워드는 짧고 일관된 게임 용어로 옮긴다. 예문으로 뜻을 확인한다.\n\
- 서로 다른 원문 용어에는 서로 다른 번역어를 쓴다.\n\
- 아래 JSON 템플릿의 **값만 채워** ```json 코드 블록으로 출력한다. 키는 바꾸거나 빼지 않는다. 설명은 쓰지 않는다.\n",
    );

    let mod_name = options.mod_name.trim();
    let description = options.mod_description.trim();
    if !mod_name.is_empty() || !description.is_empty() {
        text.push_str("\n## 모드 정보\n");
        if !mod_name.is_empty() {
            text.push_str(&format!("- 모드: {mod_name}\n"));
        }
        if !description.is_empty() {
            text.push_str(&format!("- 배경: {}\n", description.replace('\n', " ")));
        }
    }
    text.push_str("\n## 어조\n");
    text.push_str(&format!("- {}\n", tone_instruction(options.tone, korean)));
    let extra = options.extra_instructions.trim();
    if !extra.is_empty() {
        text.push_str("\n## 추가 지시\n");
        for line in extra.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if line.starts_with('-') {
                text.push_str(&format!("{line}\n"));
            } else {
                text.push_str(&format!("- {line}\n"));
            }
        }
    }

    text.push_str("\n## 용어와 예문\n");
    for term in terms {
        if term.example.trim().is_empty() {
            text.push_str(&format!("- {}\n", term.source));
        } else {
            text.push_str(&format!("- {}: {}\n", term.source, term.example.trim()));
        }
    }
    let template = terms
        .iter()
        .map(|term| (term.source, ""))
        .collect::<BTreeMap<_, _>>();
    text.push_str("\n## JSON 템플릿\n```json\n");
    text.push_str(&serde_json::to_string_pretty(&template).unwrap_or_default());
    text.push_str("\n```\n");
    text
}

pub(crate) fn tone_instruction(tone: &str, korean: bool) -> &'static str {
    match (tone, korean) {
        ("haeyo", true) => "부드러운 해요체로 쓴다. 카드·효과 설명도 해요체로 통일한다.",
        ("casual", true) => {
            "캐릭터 대사와 이벤트 텍스트는 캐릭터에 맞는 반말·구어체로 쓰고, 카드·효과 설명은 합니다체를 유지한다."
        }
        ("source", true) => {
            "원문의 어조와 말투를 최대한 살린다. 카드·효과 설명은 합니다체를 유지한다."
        }
        (_, true) => {
            "게임 공식 한국어 번역과 같은 합니다체로 간결하게 쓴다. 예: `피해를 6 줍니다.`, `[gold]방어도[/gold]를 5 얻습니다.`"
        }
        ("haeyo" | "casual", false) => "Use a friendly, casual register that fits the characters.",
        ("source", false) => "Preserve the tone and voice of the source text.",
        (_, false) => {
            "Match the style of the game's official localization for the target language."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_translation::{JsonTranslationEntry, JsonTranslationStatus};

    fn sheet(count: usize) -> JsonTranslationSheet {
        JsonTranslationSheet {
            source_path: "Z:/mods/sample/localization/eng".to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: (0..count)
                .map(|index| JsonTranslationEntry {
                    key: format!("file://cards.json#/CARD_{index}.description"),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: format!(
                        "Deal {{Damage}} damage to [gold]Artoria[/gold] number {index}."
                    ),
                    translated_value: if index % 2 == 0 {
                        String::new()
                    } else {
                        "번역".to_string()
                    },
                    status: JsonTranslationStatus::Ready,
                })
                .collect(),
        }
    }

    fn options(max_chars: usize) -> PromptOptions<'static> {
        PromptOptions {
            include_prompt: true,
            max_chars,
            only_empty: false,
            include_keys: None,
            source_language: "eng",
            mod_name: "Sample",
            mod_description: "Fate crossover",
            tone: "official",
            extra_instructions: "기술 이름은 원문 병기
- 존댓말 금지",
        }
    }

    fn artoria() -> Vec<GlossaryTerm> {
        vec![GlossaryTerm {
            source: "Artoria".to_string(),
            targets: vec!["아르토리아".to_string()],
        }]
    }

    #[test]
    fn splits_into_parts_within_the_budget_without_losing_rows() {
        let sheet = sheet(60);
        let parts = build_prompt_parts(&sheet, None, &artoria(), &options(1800));
        assert!(parts.len() > 1, "{}", parts.len());
        assert_eq!(parts.iter().map(|part| part.rows).sum::<usize>(), 60);
        assert_eq!(parts.iter().map(|part| part.slots.len()).sum::<usize>(), 60);
        for (index, part) in parts.iter().enumerate() {
            assert!(part.chars <= 1800, "part {index} has {} chars", part.chars);
            assert!(
                part.text
                    .contains(&format!("{}/{} 파트", index + 1, parts.len()))
            );
            assert!(part.text.contains("- Artoria → 아르토리아"));
            assert!(part.text.contains("Fate crossover"));
            assert!(part.text.contains("조사는 강조 태그 밖에"));
            assert!(part.text.contains(
                "## 추가 지시
- 기술 이름은 원문 병기
- 존댓말 금지
"
            ));
            let json = part
                .text
                .split("```json\n")
                .last()
                .unwrap()
                .trim_end()
                .trim_end_matches("```");
            let value: serde_json::Value = serde_json::from_str(json).expect("part json");
            assert_eq!(value["cards.json"].as_object().unwrap().len(), part.rows);
        }
    }

    #[test]
    fn data_only_and_only_empty_options() {
        let sheet = sheet(10);
        let mut options = options(DEFAULT_PROMPT_MAX_CHARS);
        options.include_prompt = false;
        options.only_empty = true;
        let parts = build_prompt_parts(&sheet, None, &artoria(), &options);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].rows, 5);
        assert_eq!(parts[0].terms, 0);
        assert!(parts[0].text.starts_with('{'));
        assert!(!parts[0].text.contains("번역가"));
    }

    #[test]
    fn oversized_entries_get_their_own_part() {
        let mut sheet = sheet(3);
        sheet.entries[1].source_value = "x".repeat(3000);
        let parts = build_prompt_parts(&sheet, None, &[], &options(1500));
        assert!(parts.iter().any(|part| part.oversized && part.rows == 1));
        assert_eq!(parts.iter().map(|part| part.rows).sum::<usize>(), 3);
    }

    #[test]
    fn project_settings_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "sts2-project-settings-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let settings = ProjectTranslationSettings {
            mod_description: "설명".to_string(),
            tone: "casual".to_string(),
            terms: vec![ProjectTerm {
                source: "Artoria".to_string(),
                targets: vec!["아르토리아".to_string()],
            }],
            extra_instructions: "대사는 반말".to_string(),
            include_prompt: Some(false),
            only_empty: Some(true),
            max_chars: Some(8000),
        };
        write_project_settings(&dir, "workshop-123/../x", &settings).expect("write");
        assert_eq!(
            read_project_settings(&dir, "workshop-123/../x").expect("read"),
            settings
        );
        assert_eq!(
            read_project_settings(&dir, "other").expect("read"),
            ProjectTranslationSettings::default()
        );
        assert!(project_settings_path(&dir, "../..").is_some_and(|path| path.starts_with(&dir)));
        assert_eq!(project_settings_path(&dir, "  "), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn term_prompts_list_examples_and_a_template() {
        let terms = (0..40)
            .map(|index| format!("Term {index}"))
            .collect::<Vec<_>>();
        let examples = terms
            .iter()
            .map(|term| format!("Gain [gold]{term}[/gold] when you play a card."))
            .collect::<Vec<_>>();
        let inputs = terms
            .iter()
            .zip(&examples)
            .map(|(source, example)| TermToTranslate { source, example })
            .collect::<Vec<_>>();
        let parts = build_term_prompt_parts(&inputs, "kor", &options(1500));
        assert!(parts.len() > 1);
        assert_eq!(parts.iter().map(|part| part.rows).sum::<usize>(), 40);
        for part in &parts {
            assert!(part.chars <= 1500, "{}", part.chars);
            assert!(part.text.contains("- Term "));
            let json = part
                .text
                .split("```json\n")
                .last()
                .unwrap()
                .trim_end()
                .trim_end_matches("```");
            let value: serde_json::Value = serde_json::from_str(json).expect("template json");
            assert_eq!(value.as_object().unwrap().len(), part.rows);
            assert!(value.as_object().unwrap().values().all(|value| value == ""));
        }
        assert!(build_term_prompt_parts(&[], "kor", &options(1500)).is_empty());
    }
}
