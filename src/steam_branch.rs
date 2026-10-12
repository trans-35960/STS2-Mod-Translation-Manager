//! Steam branch (`public` / `public-beta`) compatibility for Workshop mods.
//!
//! The game refuses to load a Workshop mod whose installed snapshot does not
//! list the current Steam branch in its `game_branch_min`/`game_branch_max`
//! range (`ModManager.CheckSteamBranchSupport`). Steam keeps both sides of that
//! check on disk: the mounted branch in `appmanifest_<appid>.acf` and each
//! item's snapshot ranges in `workshop/appworkshop_<appid>.acf`.

use crate::config::STEAM_APP_ID;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_BRANCH: &str = "public";

/// Branches whose order is known, oldest-facing first. Steam compares a
/// branch against an item's min/max by this order; anything else can only be
/// matched exactly.
const ORDERED_BRANCHES: [&str; 2] = ["public", "public-beta"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBranch {
    pub branch: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkshopSnapshot {
    pub manifest: String,
    pub branch_min: String,
    pub branch_max: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkshopItemBranches {
    pub installed_manifest: Option<String>,
    pub snapshots: Vec<WorkshopSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchSupport {
    Supported,
    /// No snapshot of the item supports the current branch.
    Unsupported { branch_min: String, branch_max: String },
    /// Another snapshot supports the current branch, but Steam still has the
    /// incompatible one installed.
    AwaitingDownload { branch_min: String, branch_max: String },
    Unknown,
}

/// Reads the mounted branch and game version for the install at `game_dir`
/// (`<library>/steamapps/common/<installdir>`).
pub fn read_game_branch(game_dir: &Path) -> Option<GameBranch> {
    let steamapps = game_dir.parent()?.parent()?;
    let manifest_path = steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf"));
    let text = fs::read_to_string(manifest_path).ok()?;
    Some(GameBranch {
        branch: current_branch_from_app_manifest(&text),
        version: read_game_version(game_dir),
    })
}

fn read_game_version(game_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(game_dir.join("release_info.json")).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// The branch Steam actually has installed. `MountedConfig` wins over
/// `UserConfig`, which only records what the user picked and may still be
/// downloading; an empty key means the default branch.
pub fn current_branch_from_app_manifest(text: &str) -> String {
    let root = parse_vdf(text);
    let state = root.child("AppState");
    ["MountedConfig", "UserConfig"]
        .iter()
        .find_map(|section| {
            state
                .and_then(|state| state.child(section))
                .and_then(|config| config.string("BetaKey"))
        })
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .unwrap_or(DEFAULT_BRANCH)
        .to_string()
}

/// Reads snapshot branch ranges for every Workshop item, given the
/// `steamapps/workshop/content/<appid>` folders the mods are scanned from.
pub fn read_workshop_branches(
    workshop_content_dirs: &[PathBuf],
) -> BTreeMap<String, WorkshopItemBranches> {
    let mut items = BTreeMap::new();
    for content_dir in workshop_content_dirs {
        let Some(workshop_dir) = content_dir.parent().and_then(Path::parent) else {
            continue;
        };
        let path = workshop_dir.join(format!("appworkshop_{STEAM_APP_ID}.acf"));
        if let Ok(text) = fs::read_to_string(path) {
            items.extend(parse_workshop_branches(&text));
        }
    }
    items
}

pub fn parse_workshop_branches(text: &str) -> BTreeMap<String, WorkshopItemBranches> {
    let root = parse_vdf(text);
    let Some(workshop) = root.child("AppWorkshop") else {
        return BTreeMap::new();
    };
    let mut items = BTreeMap::<String, WorkshopItemBranches>::new();
    if let Some(installed) = workshop.child("WorkshopItemsInstalled") {
        for (item_id, item) in installed.entries() {
            if let Some(manifest) = item.string("manifest") {
                items.entry(item_id.to_string()).or_default().installed_manifest =
                    Some(manifest.to_string());
            }
        }
    }
    if let Some(details) = workshop.child("WorkshopItemDetails") {
        for (item_id, item) in details.entries() {
            let entry = items.entry(item_id.to_string()).or_default();
            if entry.installed_manifest.is_none() {
                entry.installed_manifest = item.string("manifest").map(str::to_string);
            }
            let Some(snapshots) = item.child("author_snapshots") else {
                continue;
            };
            for (_, snapshot) in snapshots.entries() {
                let Some(manifest) = snapshot.string("manifest") else {
                    continue;
                };
                entry.snapshots.push(WorkshopSnapshot {
                    manifest: manifest.to_string(),
                    branch_min: snapshot.string("game_branch_min").unwrap_or("").to_string(),
                    branch_max: snapshot.string("game_branch_max").unwrap_or("").to_string(),
                });
            }
        }
    }
    items
}

pub fn branch_support(item: &WorkshopItemBranches, current_branch: &str) -> BranchSupport {
    if item.snapshots.is_empty() {
        return BranchSupport::Supported;
    }
    let Some(installed) = item.installed_manifest.as_deref().and_then(|manifest| {
        item.snapshots
            .iter()
            .find(|snapshot| snapshot.manifest == manifest)
    }) else {
        return BranchSupport::Unknown;
    };
    match branch_in_range(current_branch, &installed.branch_min, &installed.branch_max) {
        Some(true) => BranchSupport::Supported,
        None => BranchSupport::Unknown,
        Some(false) => {
            let branch_min = installed.branch_min.clone();
            let branch_max = installed.branch_max.clone();
            if item.snapshots.iter().any(|snapshot| {
                branch_in_range(current_branch, &snapshot.branch_min, &snapshot.branch_max)
                    == Some(true)
            }) {
                BranchSupport::AwaitingDownload {
                    branch_min,
                    branch_max,
                }
            } else {
                BranchSupport::Unsupported {
                    branch_min,
                    branch_max,
                }
            }
        }
    }
}

/// `Some(true)` when `current` lies within `min..=max` (empty bounds are
/// open), `None` when an unknown branch makes the order undecidable.
pub fn branch_in_range(current: &str, min: &str, max: &str) -> Option<bool> {
    let lower = bound_satisfied(current, min, |current, bound| current >= bound)?;
    let upper = bound_satisfied(current, max, |current, bound| current <= bound)?;
    Some(lower && upper)
}

fn bound_satisfied(
    current: &str,
    bound: &str,
    compare: impl Fn(usize, usize) -> bool,
) -> Option<bool> {
    let bound = bound.trim();
    if bound.is_empty() || bound.eq_ignore_ascii_case(current.trim()) {
        return Some(true);
    }
    Some(compare(branch_rank(current)?, branch_rank(bound)?))
}

fn branch_rank(branch: &str) -> Option<usize> {
    let branch = branch.trim();
    ORDERED_BRANCHES
        .iter()
        .position(|known| known.eq_ignore_ascii_case(branch))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Vdf {
    Text(String),
    Object(Vec<(String, Vdf)>),
}

impl Vdf {
    fn entries(&self) -> impl Iterator<Item = (&str, &Vdf)> {
        let entries: &[(String, Vdf)] = match self {
            Vdf::Object(entries) => entries,
            Vdf::Text(_) => &[],
        };
        entries.iter().map(|(key, value)| (key.as_str(), value))
    }

    fn child(&self, key: &str) -> Option<&Vdf> {
        self.entries()
            .find(|(name, value)| name.eq_ignore_ascii_case(key) && matches!(value, Vdf::Object(_)))
            .map(|(_, value)| value)
    }

    fn string(&self, key: &str) -> Option<&str> {
        self.entries().find_map(|(name, value)| match value {
            Vdf::Text(text) if name.eq_ignore_ascii_case(key) => Some(text.as_str()),
            _ => None,
        })
    }
}

/// Minimal parser for Steam's text KeyValues (`.acf`/`.vdf`): quoted keys
/// followed by a quoted value or a `{ ... }` block. Malformed input yields
/// whatever was parsed before the problem.
fn parse_vdf(text: &str) -> Vdf {
    let mut tokens = VdfTokens {
        chars: text.chars().peekable(),
    };
    Vdf::Object(parse_vdf_entries(&mut tokens))
}

fn parse_vdf_entries(tokens: &mut VdfTokens<'_>) -> Vec<(String, Vdf)> {
    let mut entries = Vec::new();
    loop {
        let key = match tokens.next() {
            Some(VdfToken::Text(key)) => key,
            Some(VdfToken::Open) => continue,
            Some(VdfToken::Close) | None => return entries,
        };
        match tokens.next() {
            Some(VdfToken::Text(value)) => entries.push((key, Vdf::Text(value))),
            Some(VdfToken::Open) => entries.push((key, Vdf::Object(parse_vdf_entries(tokens)))),
            Some(VdfToken::Close) | None => return entries,
        }
    }
}

enum VdfToken {
    Text(String),
    Open,
    Close,
}

struct VdfTokens<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Iterator for VdfTokens<'_> {
    type Item = VdfToken;

    fn next(&mut self) -> Option<VdfToken> {
        loop {
            match self.chars.next()? {
                '{' => return Some(VdfToken::Open),
                '}' => return Some(VdfToken::Close),
                '"' => {
                    let mut text = String::new();
                    while let Some(character) = self.chars.next() {
                        match character {
                            '"' => break,
                            '\\' => {
                                if let Some(escaped) = self.chars.next() {
                                    text.push(match escaped {
                                        'n' => '\n',
                                        't' => '\t',
                                        other => other,
                                    });
                                }
                            }
                            other => text.push(other),
                        }
                    }
                    return Some(VdfToken::Text(text));
                }
                '/' if self.chars.peek() == Some(&'/') => {
                    for character in self.chars.by_ref() {
                        if character == '\n' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_MANIFEST: &str = r#""AppState"
{
	"appid"		"2868840"
	"installdir"		"Slay the Spire 2"
	"UserConfig"
	{
		"language"		"english"
		"BetaKey"		"public-beta"
	}
	"MountedConfig"
	{
		"language"		"english"
		"BetaKey"		"public-beta"
	}
}"#;

    const APP_WORKSHOP: &str = r#""AppWorkshop"
{
	"appid"		"2868840"
	"WorkshopItemsInstalled"
	{
		"3747536026"
		{
			"size"		"591675947"
			"manifest"		"6775033776803984315"
		}
		"3780369635"
		{
			"manifest"		"6833475557123267806"
		}
		"3811028356"
		{
			"manifest"		"2791225091241475826"
		}
	}
	"WorkshopItemDetails"
	{
		"3747536026"
		{
			"manifest"		"6775033776803984315"
			"author_snapshots"
			{
				"0"
				{
					"manifest"		"6665820022070686930"
					"game_branch_min"		""
					"game_branch_max"		"public"
				}
				"1"
				{
					"manifest"		"6775033776803984315"
					"game_branch_min"		""
					"game_branch_max"		"public"
				}
			}
		}
		"3780369635"
		{
			"manifest"		"6833475557123267806"
			"author_snapshots"
			{
				"0"
				{
					"manifest"		"3249980422534851564"
					"game_branch_min"		"public"
					"game_branch_max"		"public"
				}
				"1"
				{
					"manifest"		"6833475557123267806"
					"game_branch_min"		"public-beta"
					"game_branch_max"		"public-beta"
				}
			}
		}
		"3811028356"
		{
			"manifest"		"2791225091241475826"
		}
	}
}"#;

    #[test]
    fn reads_mounted_branch_and_defaults_to_public() {
        assert_eq!(current_branch_from_app_manifest(APP_MANIFEST), "public-beta");
        let public = APP_MANIFEST.replace("\"public-beta\"", "\"\"");
        assert_eq!(current_branch_from_app_manifest(&public), "public");
        assert_eq!(current_branch_from_app_manifest("\"AppState\" {}"), "public");
    }

    #[test]
    fn mounted_branch_wins_over_user_selection() {
        let switching = APP_MANIFEST.replacen("\"public-beta\"", "\"public\"", 1);
        // UserConfig now says public, MountedConfig still has the beta files.
        assert_eq!(current_branch_from_app_manifest(&switching), "public-beta");
    }

    #[test]
    fn workshop_branch_support_matches_game_load_check() {
        let items = parse_workshop_branches(APP_WORKSHOP);

        assert_eq!(
            branch_support(&items["3747536026"], "public-beta"),
            BranchSupport::Unsupported {
                branch_min: String::new(),
                branch_max: "public".to_string(),
            }
        );
        assert_eq!(
            branch_support(&items["3747536026"], "public"),
            BranchSupport::Supported
        );
        assert_eq!(
            branch_support(&items["3780369635"], "public-beta"),
            BranchSupport::Supported
        );
        assert_eq!(
            branch_support(&items["3780369635"], "public"),
            BranchSupport::AwaitingDownload {
                branch_min: "public-beta".to_string(),
                branch_max: "public-beta".to_string(),
            }
        );
        // Items without author snapshots carry no branch restriction.
        assert_eq!(
            branch_support(&items["3811028356"], "public-beta"),
            BranchSupport::Supported
        );
    }

    #[test]
    fn unknown_branches_only_match_exactly() {
        assert_eq!(branch_in_range("experimental", "", "public"), None);
        assert_eq!(branch_in_range("experimental", "experimental", ""), Some(true));
        assert_eq!(branch_in_range("public", "public-beta", ""), Some(false));
        assert_eq!(branch_in_range("public-beta", "", ""), Some(true));
    }
}
