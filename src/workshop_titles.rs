//! Steam Workshop page titles for subscribed items.
//!
//! The title on a Workshop page is set on Steam and is not stored anywhere in
//! the local install, so it often differs from the manifest id the app names
//! the mod by. Titles are looked up by item id through Steam's public
//! `GetPublishedFileDetails` endpoint (no API key) and cached in the state
//! folder so the mod list can show them offline.

use crate::error::{AppError, AppResult};
use crate::process::hidden_command;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_FILE: &str = "workshop_titles.json";
const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";
/// Authors rename items now and then; a week keeps titles fresh enough.
const REFRESH_AFTER_SECS: u64 = 7 * 24 * 60 * 60;
const MAX_ITEMS_PER_REQUEST: usize = 100;
const REQUEST_TIMEOUT_SECS: &str = "15";

#[derive(Debug, Default, Serialize, Deserialize)]
struct TitleCache {
    #[serde(default)]
    items: BTreeMap<String, CachedTitle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedTitle {
    /// `None` when Steam has no public title (hidden or removed item).
    title: Option<String>,
    fetched_epoch: u64,
}

/// Titles already cached, by item id.
pub fn cached_titles(state_dir: &Path) -> HashMap<String, String> {
    read_cache(state_dir)
        .items
        .into_iter()
        .filter_map(|(item_id, entry)| Some((item_id, entry.title?)))
        .collect()
}

/// Fetches titles for items not cached yet or cached too long ago, updates
/// the cache and returns the known titles of `item_ids`.
pub fn refresh_titles(state_dir: &Path, item_ids: &[String]) -> AppResult<HashMap<String, String>> {
    refresh_titles_with(state_dir, item_ids, epoch_now(), fetch_details)
}

fn refresh_titles_with(
    state_dir: &Path,
    item_ids: &[String],
    now: u64,
    fetch: impl Fn(&[String]) -> AppResult<String>,
) -> AppResult<HashMap<String, String>> {
    let mut cache = read_cache(state_dir);
    let mut wanted = item_ids
        .iter()
        .map(|item_id| item_id.trim())
        .filter(|item_id| is_item_id(item_id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    wanted.sort();
    wanted.dedup();

    let stale = wanted
        .iter()
        .filter(|item_id| {
            cache
                .items
                .get(*item_id)
                .is_none_or(|entry| now.saturating_sub(entry.fetched_epoch) >= REFRESH_AFTER_SECS)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut fetched_any = false;
    let mut failure = None;
    for batch in stale.chunks(MAX_ITEMS_PER_REQUEST) {
        let titles = match fetch(batch).and_then(|text| parse_details(&text)) {
            Ok(titles) => titles,
            Err(error) => {
                failure = Some(error);
                break;
            }
        };
        for item_id in batch {
            cache.items.insert(
                item_id.clone(),
                CachedTitle {
                    title: titles.get(item_id).cloned(),
                    fetched_epoch: now,
                },
            );
        }
        fetched_any = true;
    }
    if fetched_any {
        write_cache(state_dir, &cache)?;
    }
    if let Some(error) = failure {
        return Err(error);
    }

    Ok(wanted
        .into_iter()
        .filter_map(|item_id| {
            let title = cache.items.get(&item_id)?.title.clone()?;
            Some((item_id, title))
        })
        .collect())
}

fn is_item_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 20
        && value.chars().all(|character| character.is_ascii_digit())
}

/// Posts the item ids to Steam with the `curl.exe` that ships with Windows,
/// so the app needs no HTTP client of its own.
fn fetch_details(item_ids: &[String]) -> AppResult<String> {
    let mut body = format!("itemcount={}", item_ids.len());
    for (index, item_id) in item_ids.iter().enumerate() {
        body.push_str(&format!("&publishedfileids%5B{index}%5D={item_id}"));
    }
    let output = hidden_command("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            REQUEST_TIMEOUT_SECS,
            "--data",
            &body,
            DETAILS_URL,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| AppError::InvalidCommand(format!("curl을 실행할 수 없습니다: {error}")))?;
    if !output.status.success() {
        return Err(AppError::InvalidCommand(format!(
            "Steam 창작마당 제목을 가져오지 못했습니다: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| AppError::InvalidCommand("Steam 응답을 읽을 수 없습니다".to_string()))
}

#[derive(Deserialize)]
struct DetailsResponse {
    response: DetailsBody,
}

#[derive(Deserialize)]
struct DetailsBody {
    #[serde(default)]
    publishedfiledetails: Vec<ItemDetails>,
}

#[derive(Deserialize)]
struct ItemDetails {
    publishedfileid: String,
    #[serde(default)]
    title: Option<String>,
}

fn parse_details(text: &str) -> AppResult<HashMap<String, String>> {
    let response = serde_json::from_str::<DetailsResponse>(text).map_err(|error| {
        AppError::InvalidCommand(format!("Steam 응답 형식이 올바르지 않습니다: {error}"))
    })?;
    Ok(response
        .response
        .publishedfiledetails
        .into_iter()
        .filter_map(|item| {
            let title = item.title?.trim().to_string();
            (!title.is_empty()).then_some((item.publishedfileid, title))
        })
        .collect())
}

fn cache_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CACHE_FILE)
}

fn read_cache(state_dir: &Path) -> TitleCache {
    fs::read_to_string(cache_path(state_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_cache(state_dir: &Path, cache: &TitleCache) -> AppResult<()> {
    fs::create_dir_all(state_dir).map_err(|source| AppError::io(state_dir, source))?;
    let path = cache_path(state_dir);
    let text = serde_json::to_string_pretty(cache)
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
    fs::write(&path, text).map_err(|source| AppError::io(&path, source))
}

fn epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn test_dir(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-work");
        path.push(format!(
            "{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create test dir");
        path
    }

    fn response(items: &[(&str, Option<&str>)]) -> String {
        let details = items
            .iter()
            .map(|(item_id, title)| match title {
                Some(title) => {
                    serde_json::json!({"publishedfileid": item_id, "result": 1, "title": title})
                }
                None => serde_json::json!({"publishedfileid": item_id, "result": 9}),
            })
            .collect::<Vec<_>>();
        serde_json::json!({"response": {"result": 1, "resultcount": items.len(), "publishedfiledetails": details}})
            .to_string()
    }

    #[test]
    fn parses_titles_and_skips_hidden_items() {
        let titles = parse_details(&response(&[
            ("3747602295", Some(" RitsuLib - Shared Library ")),
            ("3780369635", None),
        ]))
        .expect("parse");

        assert_eq!(titles.len(), 1);
        assert_eq!(titles["3747602295"], "RitsuLib - Shared Library");
    }

    #[test]
    fn fetches_only_missing_or_stale_items() {
        let dir = test_dir("workshop_titles_refresh");
        let requests = RefCell::new(Vec::<Vec<String>>::new());
        let fetch = |ids: &[String]| {
            requests.borrow_mut().push(ids.to_vec());
            Ok(response(
                &ids.iter()
                    .map(|item_id| (item_id.as_str(), Some("Title")))
                    .collect::<Vec<_>>(),
            ))
        };
        let ids = vec![
            "111111".to_string(),
            "222222".to_string(),
            "bad id".to_string(),
        ];

        let first = refresh_titles_with(&dir, &ids, 1_000, fetch).expect("first");
        assert_eq!(first.len(), 2);
        refresh_titles_with(&dir, &ids, 2_000, fetch).expect("cached");
        refresh_titles_with(&dir, &ids[..1], 1_000 + REFRESH_AFTER_SECS, fetch).expect("stale");

        assert_eq!(
            *requests.borrow(),
            vec![
                vec!["111111".to_string(), "222222".to_string()],
                vec!["111111".to_string()],
            ]
        );
        assert_eq!(cached_titles(&dir)["222222"], "Title");
    }

    #[test]
    fn remembers_items_without_a_public_title() {
        let dir = test_dir("workshop_titles_hidden");
        let calls = RefCell::new(0);
        let fetch = |_: &[String]| {
            *calls.borrow_mut() += 1;
            Ok(response(&[("333333", None)]))
        };
        let ids = vec!["333333".to_string()];

        assert!(
            refresh_titles_with(&dir, &ids, 10, fetch)
                .expect("first")
                .is_empty()
        );
        refresh_titles_with(&dir, &ids, 20, fetch).expect("second");

        assert_eq!(*calls.borrow(), 1);
        assert!(cached_titles(&dir).is_empty());
    }

    #[test]
    fn keeps_cache_when_fetch_fails() {
        let dir = test_dir("workshop_titles_fail");
        let ids = vec!["444444".to_string()];
        refresh_titles_with(&dir, &ids, 10, |_| Ok(response(&[("444444", Some("Old"))])))
            .expect("seed");

        let result = refresh_titles_with(&dir, &ids, 10 + REFRESH_AFTER_SECS, |_| {
            Err(AppError::InvalidCommand("offline".to_string()))
        });

        assert!(result.is_err());
        assert_eq!(cached_titles(&dir)["444444"], "Old");
    }
}
