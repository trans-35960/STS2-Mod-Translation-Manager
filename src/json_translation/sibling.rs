//! One mod's text exists once per source language (`localization/eng/…`,
//! `localization/zhs/…`), and each gets its own sheet. The game text behind
//! `…/localization/eng/cards.json#/X` and `…/localization/zhs/cards.json#/X`
//! is the same, so their translations are shared between those sheets.

use super::types::{JsonTranslationSheet, JsonTranslationStatus};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The key with its language folder left out, so the same text matches
/// across source languages. `None` for keys outside `localization/<lang>/`
/// (DLL strings, other files).
pub fn language_neutral_key(key: &str) -> Option<String> {
    let (path, pointer) = key.strip_prefix("file://")?.split_once('#')?;
    let parts = path.split('/').collect::<Vec<_>>();
    let localization = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("localization"))?;
    if localization + 2 >= parts.len() {
        return None;
    }
    let mut neutral = parts
        .iter()
        .enumerate()
        .map(|(index, part)| if index == localization + 1 { "*" } else { part })
        .collect::<Vec<_>>()
        .join("/");
    neutral.make_ascii_lowercase();
    Some(format!("{neutral}#{pointer}"))
}

/// A sheet of the same mod and target language made from another source
/// language, and when it was last saved.
pub struct SiblingSheet {
    pub sheet: JsonTranslationSheet,
    pub saved: SystemTime,
}

/// Takes translations from `siblings` into `sheet`. Empty values are filled;
/// a different value is taken only from a sibling saved after
/// `sheet_saved` (`None` for a sheet that did not exist before), so the last
/// edit wins whichever source language it was made in. Returns how many
/// entries changed.
pub fn carry_over_sibling_translations(
    sheet: &mut JsonTranslationSheet,
    sheet_saved: Option<SystemTime>,
    siblings: &[SiblingSheet],
) -> usize {
    let mut newest = BTreeMap::<String, (&str, SystemTime)>::new();
    for sibling in siblings {
        if !sibling
            .sheet
            .target_language
            .eq_ignore_ascii_case(&sheet.target_language)
        {
            continue;
        }
        for entry in &sibling.sheet.entries {
            if entry.translated_value.is_empty() || entry.status == JsonTranslationStatus::Removed {
                continue;
            }
            let Some(key) = language_neutral_key(&entry.key) else {
                continue;
            };
            let candidate = (entry.translated_value.as_str(), sibling.saved);
            newest
                .entry(key)
                .and_modify(|current| {
                    if candidate.1 > current.1 {
                        *current = candidate;
                    }
                })
                .or_insert(candidate);
        }
    }

    let mut changed = 0;
    for entry in &mut sheet.entries {
        if entry.status == JsonTranslationStatus::Removed {
            continue;
        }
        let Some((value, saved)) =
            language_neutral_key(&entry.key).and_then(|key| newest.get(&key))
        else {
            continue;
        };
        if entry.translated_value == *value {
            continue;
        }
        let newer = sheet_saved.is_none_or(|sheet_saved| *saved > sheet_saved);
        if !entry.translated_value.is_empty() && !newer {
            continue;
        }
        entry.translated_value = value.to_string();
        if matches!(
            entry.status,
            JsonTranslationStatus::New | JsonTranslationStatus::Missing
        ) {
            entry.status = JsonTranslationStatus::Ready;
        }
        changed += 1;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_translation::JsonTranslationEntry;
    use std::time::Duration;

    fn entry(key: &str, translated: &str, status: JsonTranslationStatus) -> JsonTranslationEntry {
        JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "source".to_string(),
            translated_value: translated.to_string(),
            status,
        }
    }

    fn sheet(entries: Vec<JsonTranslationEntry>) -> JsonTranslationSheet {
        JsonTranslationSheet {
            source_path: String::new(),
            target_language: "kor".to_string(),
            updated_epoch: 0,
            entries,
        }
    }

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn neutral_key_ignores_only_the_language_folder() {
        assert_eq!(
            language_neutral_key("file://Mod/localization/eng/cards.json#/A.title"),
            language_neutral_key("file://Mod/localization/zhs/cards.json#/A.title"),
        );
        assert_ne!(
            language_neutral_key("file://Mod/localization/eng/cards.json#/A.title"),
            language_neutral_key("file://Mod/localization/eng/relics.json#/A.title"),
        );
        assert_eq!(language_neutral_key("dll://1a2b:4"), None);
        assert_eq!(
            language_neutral_key("file://Mod/data/trials.json#/name"),
            None
        );
    }

    #[test]
    fn fills_empty_entries_from_another_source_language() {
        let zhs = SiblingSheet {
            sheet: sheet(vec![
                entry(
                    "file://Mod/localization/zhs/cards.json#/A",
                    "절제",
                    JsonTranslationStatus::Ready,
                ),
                entry(
                    "file://Mod/localization/zhs/cards.json#/B",
                    "",
                    JsonTranslationStatus::Missing,
                ),
            ]),
            saved: at(10),
        };
        let mut eng = sheet(vec![
            entry(
                "file://Mod/localization/eng/cards.json#/A",
                "",
                JsonTranslationStatus::Missing,
            ),
            entry(
                "file://Mod/localization/eng/cards.json#/B",
                "",
                JsonTranslationStatus::Missing,
            ),
            entry(
                "file://Mod/localization/eng/cards.json#/C",
                "",
                JsonTranslationStatus::New,
            ),
        ]);

        // A sheet older than the sibling, but its empty values are filled either way.
        let changed = carry_over_sibling_translations(&mut eng, Some(at(20)), &[zhs]);

        assert_eq!(changed, 1);
        assert_eq!(eng.entries[0].translated_value, "절제");
        assert_eq!(eng.entries[0].status, JsonTranslationStatus::Ready);
        assert_eq!(eng.entries[1].status, JsonTranslationStatus::Missing);
        assert_eq!(eng.entries[2].status, JsonTranslationStatus::New);
    }

    #[test]
    fn a_different_value_is_taken_only_from_a_later_save() {
        let eng = SiblingSheet {
            sheet: sheet(vec![entry(
                "file://Mod/localization/eng/cards.json#/A",
                "새 번역",
                JsonTranslationStatus::Ready,
            )]),
            saved: at(20),
        };
        let zhs = || {
            sheet(vec![entry(
                "file://Mod/localization/zhs/cards.json#/A",
                "옛 번역",
                JsonTranslationStatus::Ready,
            )])
        };

        let mut older = zhs();
        assert_eq!(
            carry_over_sibling_translations(&mut older, Some(at(10)), std::slice::from_ref(&eng)),
            1
        );
        assert_eq!(older.entries[0].translated_value, "새 번역");

        let mut newer = zhs();
        assert_eq!(
            carry_over_sibling_translations(&mut newer, Some(at(30)), &[eng]),
            0
        );
        assert_eq!(newer.entries[0].translated_value, "옛 번역");
    }
}
