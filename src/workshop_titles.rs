//! Steam Workshop page titles and preview images for subscribed items.
//!
//! The title on a Workshop page is set on Steam and is not stored anywhere in
//! the local install, so it often differs from the manifest id the app names
//! the mod by. Titles are looked up by item id through Steam's public
//! `GetPublishedFileDetails` endpoint (no API key) and cached in the state
//! folder so the mod list can show them offline. The same response names the
//! item's preview image, which is downloaded once as a small thumbnail.

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
const THUMBNAIL_DIR: &str = "workshop_thumbnails";
/// Rows show thumbnails at 36-56px and enlarge them to 256px on hover.
const THUMBNAIL_SIZE: u32 = 256;
const THUMBNAIL_TIMEOUT_SECS: &str = "30";
/// Larger files are not thumbnails worth keeping (the resize was ignored).
const MAX_THUMBNAIL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Default, Serialize, Deserialize)]
struct TitleCache {
    #[serde(default)]
    items: BTreeMap<String, CachedTitle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedTitle {
    /// `None` when Steam has no public title (hidden or removed item).
    title: Option<String>,
    /// Steam's preview image URL, when the item has one.
    #[serde(default)]
    preview_url: Option<String>,
    /// False for entries cached before preview URLs were recorded, so they
    /// are fetched again once.
    #[serde(default)]
    preview_known: bool,
    /// The preview URL the stored thumbnail was downloaded from.
    #[serde(default)]
    thumbnail_url: Option<String>,
    fetched_epoch: u64,
}

/// Titles and thumbnails known for a set of Workshop items.
#[derive(Debug, Default)]
pub struct WorkshopDetails {
    /// Item id → Workshop page title.
    pub titles: HashMap<String, String>,
    /// Item id → thumbnail as a `data:` URL.
    pub thumbnails: HashMap<String, String>,
}

/// Item details read from one Steam response.
#[derive(Debug, Default, Clone, PartialEq)]
struct FetchedItem {
    title: Option<String>,
    preview_url: Option<String>,
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
    let wanted = valid_item_ids(item_ids);

    let stale = wanted
        .iter()
        .filter(|item_id| {
            cache.items.get(*item_id).is_none_or(|entry| {
                !entry.preview_known
                    || now.saturating_sub(entry.fetched_epoch) >= REFRESH_AFTER_SECS
            })
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut fetched_any = false;
    let mut failure = None;
    for batch in stale.chunks(MAX_ITEMS_PER_REQUEST) {
        let items = match fetch(batch).and_then(|text| parse_details(&text)) {
            Ok(items) => items,
            Err(error) => {
                failure = Some(error);
                break;
            }
        };
        for item_id in batch {
            let item = items.get(item_id).cloned().unwrap_or_default();
            let thumbnail_url = cache
                .items
                .get(item_id)
                .and_then(|entry| entry.thumbnail_url.clone());
            cache.items.insert(
                item_id.clone(),
                CachedTitle {
                    title: item.title,
                    preview_url: item.preview_url,
                    preview_known: true,
                    thumbnail_url,
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

/// Refreshes titles like [`refresh_titles`], downloads the thumbnails that
/// are missing or whose preview image changed, and returns both for
/// `item_ids`. A failed thumbnail download is retried on the next call.
pub fn refresh_details(state_dir: &Path, item_ids: &[String]) -> AppResult<WorkshopDetails> {
    refresh_details_with(
        state_dir,
        item_ids,
        epoch_now(),
        fetch_details,
        download_files,
    )
}

fn refresh_details_with(
    state_dir: &Path,
    item_ids: &[String],
    now: u64,
    fetch: impl Fn(&[String]) -> AppResult<String>,
    download: impl Fn(&[(String, PathBuf)]),
) -> AppResult<WorkshopDetails> {
    let titles = refresh_titles_with(state_dir, item_ids, now, fetch)?;
    let mut cache = read_cache(state_dir);
    let wanted = valid_item_ids(item_ids);

    let pending = wanted
        .iter()
        .filter_map(|item_id| {
            let entry = cache.items.get(item_id)?;
            let url = entry.preview_url.as_ref()?;
            let current = entry.thumbnail_url.as_ref() == Some(url)
                && thumbnail_path(state_dir, item_id).is_file();
            (!current).then(|| (item_id.clone(), url.clone()))
        })
        .collect::<Vec<_>>();
    if !pending.is_empty() {
        let dir = thumbnail_dir(state_dir);
        fs::create_dir_all(&dir).map_err(|source| AppError::io(&dir, source))?;
        // Ask Steam's image CDN for a small copy first and fall back to the
        // full image when the resized one does not come back.
        let mut remaining = pending.clone();
        for sized in [true, false] {
            if remaining.is_empty() {
                break;
            }
            let jobs = remaining
                .iter()
                .map(|(item_id, url)| {
                    let url = if sized {
                        sized_preview_url(url)
                    } else {
                        url.clone()
                    };
                    (url, partial_path(state_dir, item_id))
                })
                .collect::<Vec<_>>();
            download(&jobs);
            remaining.retain(|(item_id, _)| !keep_download(state_dir, item_id));
        }
        for (item_id, url) in &pending {
            if remaining.iter().any(|(failed, _)| failed == item_id) {
                continue;
            }
            if let Some(entry) = cache.items.get_mut(item_id) {
                entry.thumbnail_url = Some(url.clone());
            }
        }
        if remaining.len() < pending.len() {
            write_cache(state_dir, &cache)?;
        }
    }

    let thumbnails = wanted
        .iter()
        .filter(|item_id| {
            cache.items.get(*item_id).is_some_and(|entry| {
                entry.preview_url.is_some() && entry.thumbnail_url == entry.preview_url
            })
        })
        .filter_map(|item_id| {
            let bytes = fs::read(thumbnail_path(state_dir, item_id)).ok()?;
            Some((item_id.clone(), data_url(&bytes)?))
        })
        .collect();
    Ok(WorkshopDetails { titles, thumbnails })
}

fn valid_item_ids(item_ids: &[String]) -> Vec<String> {
    let mut wanted = item_ids
        .iter()
        .map(|item_id| item_id.trim())
        .filter(|item_id| is_item_id(item_id))
        .map(str::to_string)
        .collect::<Vec<_>>();
    wanted.sort();
    wanted.dedup();
    wanted
}

/// Moves a finished download into place when it is a usable image.
fn keep_download(state_dir: &Path, item_id: &str) -> bool {
    let partial = partial_path(state_dir, item_id);
    let usable = fs::metadata(&partial)
        .is_ok_and(|meta| meta.len() > 0 && meta.len() <= MAX_THUMBNAIL_BYTES)
        && fs::read(&partial).is_ok_and(|bytes| image_mime(&bytes).is_some());
    if usable && fs::rename(&partial, thumbnail_path(state_dir, item_id)).is_ok() {
        // Thumbnails saved at an earlier size are replaced, not kept beside.
        let _ = fs::remove_file(thumbnail_dir(state_dir).join(format!("{item_id}.img")));
        return true;
    }
    let _ = fs::remove_file(&partial);
    false
}

/// Steam's UGC image CDN resizes on request; other hosts get the URL as is.
fn sized_preview_url(url: &str) -> String {
    if !url.contains("steamusercontent.com") && !url.contains("steamuserimages") {
        return url.to_string();
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    format!(
        "{url}{separator}imw={THUMBNAIL_SIZE}&imh={THUMBNAIL_SIZE}&ima=fit&impolicy=Letterbox&imcolor=%23000000&letterbox=false"
    )
}

/// Downloads each URL to its path in one `curl.exe` run. A failed download
/// leaves no file, or a partial one the caller discards.
fn download_files(jobs: &[(String, PathBuf)]) {
    let mut command = hidden_command("curl");
    command.args([
        "--silent",
        "--fail",
        "--location",
        "--max-time",
        THUMBNAIL_TIMEOUT_SECS,
        "--parallel",
        "--parallel-max",
        "8",
    ]);
    for (url, path) in jobs {
        command.arg("--output").arg(path).arg(url);
    }
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn thumbnail_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(THUMBNAIL_DIR)
}

/// The size is in the name, so changing it downloads the thumbnails again.
fn thumbnail_path(state_dir: &Path, item_id: &str) -> PathBuf {
    thumbnail_dir(state_dir).join(format!("{item_id}-{THUMBNAIL_SIZE}.img"))
}

fn partial_path(state_dir: &Path, item_id: &str) -> PathBuf {
    thumbnail_dir(state_dir).join(format!("{item_id}.part"))
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn data_url(bytes: &[u8]) -> Option<String> {
    let mime = image_mime(bytes)?;
    Some(format!("data:{mime};base64,{}", base64_encode(bytes)))
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for position in 0..4 {
            if position <= chunk.len() {
                out.push(ALPHABET[((triple >> (18 - 6 * position)) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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
    #[serde(default)]
    preview_url: Option<String>,
}

fn parse_details(text: &str) -> AppResult<HashMap<String, FetchedItem>> {
    let response = serde_json::from_str::<DetailsResponse>(text).map_err(|error| {
        AppError::InvalidCommand(format!("Steam 응답 형식이 올바르지 않습니다: {error}"))
    })?;
    let non_empty = |value: Option<String>| {
        value
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    Ok(response
        .response
        .publishedfiledetails
        .into_iter()
        .filter_map(|item| {
            let fetched = FetchedItem {
                title: non_empty(item.title),
                preview_url: non_empty(item.preview_url).filter(|url| url.starts_with("https://")),
            };
            (fetched != FetchedItem::default()).then_some((item.publishedfileid, fetched))
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
        assert_eq!(
            titles["3747602295"].title.as_deref(),
            Some("RitsuLib - Shared Library")
        );
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

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00];

    fn preview_response(item_id: &str, preview_url: &str) -> String {
        serde_json::json!({"response": {"publishedfiledetails": [
            {"publishedfileid": item_id, "result": 1, "title": "T", "preview_url": preview_url}
        ]}})
        .to_string()
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn downloads_thumbnail_once_and_returns_data_url() {
        let dir = test_dir("workshop_thumbnails_once");
        let url = "https://images.steamusercontent.com/ugc/1/ABC/";
        let downloads = RefCell::new(Vec::<String>::new());
        let download = |jobs: &[(String, PathBuf)]| {
            for (job_url, path) in jobs {
                downloads.borrow_mut().push(job_url.clone());
                fs::write(path, JPEG).expect("write");
            }
        };
        let ids = vec!["555555".to_string()];

        let first = refresh_details_with(
            &dir,
            &ids,
            10,
            |_| Ok(preview_response("555555", url)),
            download,
        )
        .expect("first");
        let second = refresh_details_with(
            &dir,
            &ids,
            20,
            |_| Ok(preview_response("555555", url)),
            download,
        )
        .expect("second");

        assert_eq!(downloads.borrow().len(), 1);
        assert!(downloads.borrow()[0].starts_with(&format!("{url}?imw=")));
        assert_eq!(first.titles["555555"], "T");
        assert_eq!(
            first.thumbnails["555555"],
            "data:image/jpeg;base64,/9j/4AA="
        );
        assert_eq!(second.thumbnails, first.thumbnails);
    }

    #[test]
    fn falls_back_to_full_image_and_retries_failures() {
        let dir = test_dir("workshop_thumbnails_fallback");
        let url = "https://images.steamusercontent.com/ugc/2/DEF/";
        let fetch = |_: &[String]| Ok(preview_response("666666", url));
        let ids = vec!["666666".to_string()];

        // Nothing usable comes back: no thumbnail, and the next call tries again.
        let calls = RefCell::new(0);
        let broken = |jobs: &[(String, PathBuf)]| {
            *calls.borrow_mut() += 1;
            for (_, path) in jobs {
                fs::write(path, b"<html>").expect("write");
            }
        };
        let failed = refresh_details_with(&dir, &ids, 10, fetch, broken).expect("failed");
        assert!(failed.thumbnails.is_empty());
        assert_eq!(*calls.borrow(), 2);

        // The resized copy fails but the full image works.
        let full_only = |jobs: &[(String, PathBuf)]| {
            for (job_url, path) in jobs {
                if job_url == url {
                    fs::write(path, JPEG).expect("write");
                }
            }
        };
        let details = refresh_details_with(&dir, &ids, 20, fetch, full_only).expect("full");
        assert!(details.thumbnails.contains_key("666666"));
        assert!(!partial_path(&dir, "666666").exists());
    }

    #[test]
    fn refetches_entries_cached_before_preview_urls() {
        let dir = test_dir("workshop_thumbnails_upgrade");
        fs::write(
            cache_path(&dir),
            r#"{"items":{"777777":{"title":"Old","fetched_epoch":10}}}"#,
        )
        .expect("seed");
        let calls = RefCell::new(0);
        let fetch = |_: &[String]| {
            *calls.borrow_mut() += 1;
            Ok(preview_response("777777", "https://example.com/p.jpg"))
        };

        refresh_titles_with(&dir, &["777777".to_string()], 20, fetch).expect("upgrade");
        refresh_titles_with(&dir, &["777777".to_string()], 30, fetch).expect("cached");

        assert_eq!(*calls.borrow(), 1);
    }
}
