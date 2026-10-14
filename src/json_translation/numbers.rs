//! Advisory check that a translation keeps the source's numbers.
//!
//! Its main job is catching values an AI swapped between two entries
//! ("Deal 6 damage" ending up with the text for "Deal 8 damage"), so it only
//! reports numbers the translation lost; extra numbers are fine ("twice" may
//! become "2번"). Numbers that translations routinely spell out are skipped:
//! `1` ("a card" → "한 장"), ordinals (`2nd`), `#3`, and numbers glued to
//! letters (`STS2`, `v1`). Placeholders and markup are ignored.

use super::types::{
    JsonTranslationEntry, JsonTranslationSheet, JsonTranslationStatus, JsonValidationIssue,
};
use std::collections::BTreeMap;

pub const NUMBER_ISSUE_KIND: &str = "numbers";

pub fn number_mismatch_issues(sheet: &JsonTranslationSheet) -> Vec<JsonValidationIssue> {
    sheet
        .entries
        .iter()
        .filter(|entry| should_check(entry))
        .filter_map(|entry| {
            let missing = missing_numbers(&entry.source_value, &entry.translated_value);
            (!missing.is_empty() && !entry.acknowledges(NUMBER_ISSUE_KIND)).then(|| {
                JsonValidationIssue {
                    key: entry.key.clone(),
                    kind: NUMBER_ISSUE_KIND.to_string(),
                    message: format!("원문 숫자 {}이(가) 번역에 없습니다.", missing.join(", ")),
                }
            })
        })
        .collect()
}

fn should_check(entry: &JsonTranslationEntry) -> bool {
    entry.status != JsonTranslationStatus::Removed
        && !entry.translated_value.trim().is_empty()
        && entry.translated_value != entry.source_value
}

fn missing_numbers(source: &str, translated: &str) -> Vec<String> {
    let mut available = BTreeMap::<String, usize>::new();
    for number in numbers(translated) {
        *available.entry(number).or_default() += 1;
    }
    let mut missing = Vec::new();
    for number in numbers(source) {
        match available.get_mut(&number) {
            Some(count) if *count > 0 => *count -= 1,
            _ => missing.push(number),
        }
    }
    missing
}

fn numbers(value: &str) -> Vec<String> {
    let chars = strip_markup(value).chars().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        let mut digits = String::new();
        while index < chars.len() {
            let ch = chars[index];
            if ch.is_ascii_digit() {
                digits.push(ch);
                index += 1;
            } else if ch == ','
                && chars
                    .get(index + 1..index + 4)
                    .is_some_and(|group| group.iter().all(char::is_ascii_digit))
                && !chars.get(index + 4).is_some_and(char::is_ascii_digit)
            {
                // Thousands separator: "10,000" is the same number as "10000".
                index += 1;
            } else if ch == '.' && chars.get(index + 1).is_some_and(char::is_ascii_digit) {
                digits.push('.');
                index += 1;
            } else {
                break;
            }
        }
        let before = start.checked_sub(1).map(|position| chars[position]);
        let after = chars.get(index).copied();
        let glued = before.is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '#')
            || after.is_some_and(|ch| ch.is_ascii_alphabetic());
        if !glued && digits != "1" {
            output.push(digits);
        }
    }
    output
}

/// Replaces `{...}`, `[...]`, `<...>` and `!Token!` with spaces.
fn strip_markup(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let close = match ch {
            '{' => Some('}'),
            '[' => Some(']'),
            '<' => Some('>'),
            _ => None,
        };
        if let Some(close) = close {
            let mut depth = 0;
            let mut end = None;
            for (offset, inner) in chars[index..].iter().enumerate() {
                if *inner == ch {
                    depth += 1;
                } else if *inner == close {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index + offset);
                        break;
                    }
                }
            }
            if let Some(end) = end {
                output.push(' ');
                index = end + 1;
                continue;
            }
        }
        if ch == '!' {
            let name_len = chars[index + 1..]
                .iter()
                .take_while(|inner| inner.is_ascii_alphanumeric() || **inner == '_')
                .count();
            if name_len > 0 && chars.get(index + 1 + name_len) == Some(&'!') {
                output.push(' ');
                index += name_len + 2;
                continue;
            }
        }
        output.push(ch);
        index += 1;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, translated: &str) -> JsonTranslationEntry {
        JsonTranslationEntry {
            key: format!("file://cards.json#/{source}"),
            slot_id: None,
            previous_source_value: None,
            source_value: source.to_string(),
            translated_value: translated.to_string(),
            status: JsonTranslationStatus::Ready,
            acknowledged_issues: Vec::new(),
        }
    }

    #[test]
    fn flags_numbers_lost_in_translation() {
        assert_eq!(
            missing_numbers("Deal 6 damage.", "피해를 6 줍니다."),
            Vec::<String>::new()
        );
        assert_eq!(
            missing_numbers("Deal 6 damage.", "피해를 8 줍니다."),
            vec!["6"]
        );
        assert_eq!(
            missing_numbers("Draw 2 cards.", "카드를 2.0장 뽑습니다."),
            vec!["2"]
        );
        assert_eq!(
            missing_numbers("Hit 3 times for 3.", "3번 공격합니다."),
            vec!["3"]
        );
    }

    #[test]
    fn skips_spelled_out_markup_and_identifier_numbers() {
        let cases = [
            ("Draw 1 card.", "카드를 한 장 뽑습니다."),
            (
                "At the start of your [blue]2nd[/blue] turn.",
                "[blue]두 번째[/blue] 턴 시작 시.",
            ),
            ("Select Card #3", "세 번째 카드 선택"),
            ("Requires STS2 v1.2", "필요 버전"),
            (
                "Gain {Block:diff()} [gold]Block[/gold] and {Amount:plural:1 card|{} cards}.",
                "방어도를 얻습니다.",
            ),
            ("Spend !M! and 10,000 gold.", "골드 10000을 씁니다."),
            ("Deal 5 damage twice.", "피해를 5 2번 줍니다."),
        ];
        for (source, translated) in cases {
            assert_eq!(
                missing_numbers(source, translated),
                Vec::<String>::new(),
                "{source}"
            );
        }
    }

    #[test]
    fn reports_swapped_entries() {
        let sheet = JsonTranslationSheet {
            source_path: String::new(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries: vec![
                entry("Deal 6 damage.", "피해를 8 줍니다."),
                entry("Deal 8 damage.", "피해를 6 줍니다."),
                entry("Deal 9 damage.", ""),
                entry("Deal 9 damage.", "Deal 9 damage."),
            ],
        };
        let issues = number_mismatch_issues(&sheet);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].kind, NUMBER_ISSUE_KIND);
        assert!(issues[0].message.contains('6'));
    }
}
