//! Errors from the game log (`godot.log`), each traced to the mod that most
//! likely caused it.
//!
//! The game writes one block per error: a header line (`[ERROR] ...` from the
//! game's logger, `ERROR: ...` from Godot) followed by indented stack frames.
//! A block names its mod in a few ways, checked in this order:
//!
//! 1. `Tried to load mod with id <id>, ...` from the mod loader;
//! 2. the logger tag mods use, `[ERROR] [<mod id or namespace>] ...`;
//! 3. a `res://<mod folder>/...` resource path or a mod install path;
//! 4. the first stack frame whose namespace is a mod's (DLL name or id).
//!
//! Frames of game methods Harmony rewrote (`Method_Patch1`) mean some mod
//! patched the code that failed, without saying which; such blocks are
//! flagged so the list can say so instead of blaming the game.

use std::collections::HashMap;

/// Lines kept per block; the rest of a long stack adds little.
const MAX_BLOCK_LINES: usize = 40;
/// Distinct errors kept; a broken mod can log the same block thousands of times.
const MAX_ERRORS: usize = 300;

/// Namespaces of the game and its runtime; frames in them name no mod.
const NON_MOD_NAMESPACES: &[&str] = &[
    "megacrit",
    "system",
    "godot",
    "steamworks",
    "harmonylib",
    "microsoft",
    "mono",
    "monomod",
    "newtonsoft",
    "unknown",
];

/// Shutdown and engine noise that is never a mod's fault.
const IGNORED_ERRORS: &[&str] = &[
    "leaked at exit",
    "never freed",
    "still in use at exit",
    "can't become transient",
    "objectdb instances leaked",
];

/// A mod the log may mention, by the names it can appear under.
#[derive(Debug, Clone)]
pub struct ModIdentity {
    pub key: String,
    /// Manifest id, name, folder, DLL names, Workshop item id, ...
    pub tokens: Vec<String>,
}

/// One distinct error of the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogError {
    pub mod_key: Option<String>,
    /// How the mod was found: `mod_id`, `log_tag`, `path` or `stack`.
    pub matched_by: Option<&'static str>,
    /// Header line without its level prefix.
    pub message: String,
    /// The block as logged (header and stack), trimmed to [`MAX_BLOCK_LINES`].
    pub text: String,
    /// How many times the same block was logged.
    pub count: usize,
    /// 1-based line of the first occurrence.
    pub first_line: usize,
    /// The stack runs through game code a mod patched with Harmony.
    pub patched_game_code: bool,
}

/// Distinct errors of `log`, in order of first occurrence.
pub fn collect_errors(log: &str, mods: &[ModIdentity]) -> Vec<LogError> {
    let index = TokenIndex::new(mods);
    let mut errors = Vec::<LogError>::new();
    let mut seen = HashMap::<(Option<String>, String), usize>::new();
    for block in error_blocks(log) {
        let header = block.lines[0];
        let lower = header.to_ascii_lowercase();
        if IGNORED_ERRORS.iter().any(|noise| lower.contains(noise)) {
            continue;
        }
        let message = strip_level(header).to_string();
        let (mod_key, matched_by) = match attribute(&block.lines, &index) {
            Some((key, how)) => (Some(key), Some(how)),
            None => (None, None),
        };
        let dedupe_key = (mod_key.clone(), message.clone());
        if let Some(&position) = seen.get(&dedupe_key) {
            errors[position].count += 1;
            continue;
        }
        if errors.len() >= MAX_ERRORS {
            continue;
        }
        seen.insert(dedupe_key, errors.len());
        errors.push(LogError {
            mod_key,
            matched_by,
            message,
            text: block_text(&block.lines),
            count: 1,
            first_line: block.first_line,
            patched_game_code: block.lines.iter().any(|line| is_patched_frame(line)),
        });
    }
    errors
}

struct ErrorBlock<'a> {
    first_line: usize,
    lines: Vec<&'a str>,
}

fn error_blocks(log: &str) -> Vec<ErrorBlock<'_>> {
    let mut blocks = Vec::new();
    let mut current: Option<ErrorBlock> = None;
    for (index, line) in log.lines().enumerate() {
        if is_error_header(line) {
            blocks.extend(current.take());
            current = Some(ErrorBlock {
                first_line: index + 1,
                lines: vec![line],
            });
        } else if let Some(block) = current.as_mut() {
            if is_continuation(line) {
                block.lines.push(line);
            } else {
                blocks.extend(current.take());
            }
        }
    }
    blocks.extend(current);
    blocks
}

fn is_error_header(line: &str) -> bool {
    [
        "[ERROR]",
        "[FATAL]",
        "[CRITICAL]",
        "ERROR:",
        "SCRIPT ERROR:",
        "USER ERROR:",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

fn is_continuation(line: &str) -> bool {
    !line.trim().is_empty() && (line.starts_with(char::is_whitespace) || line.starts_with("--- "))
}

fn strip_level(header: &str) -> &str {
    for prefix in [
        "[ERROR]",
        "[FATAL]",
        "[CRITICAL]",
        "SCRIPT ERROR:",
        "USER ERROR:",
        "ERROR:",
    ] {
        if let Some(rest) = header.strip_prefix(prefix) {
            return rest.trim();
        }
    }
    header.trim()
}

fn block_text(lines: &[&str]) -> String {
    let mut text = lines
        .iter()
        .take(MAX_BLOCK_LINES)
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    if lines.len() > MAX_BLOCK_LINES {
        text.push_str(&format!(
            "\n   ... ({}줄 생략)",
            lines.len() - MAX_BLOCK_LINES
        ));
    }
    text
}

/// Mod tokens, normalized, with the mod each belongs to.
struct TokenIndex {
    tokens: Vec<(String, usize)>,
    keys: Vec<String>,
}

impl TokenIndex {
    fn new(mods: &[ModIdentity]) -> Self {
        let mut tokens = Vec::new();
        for (index, identity) in mods.iter().enumerate() {
            for token in &identity.tokens {
                let normalized = normalize(token);
                // Short tokens ("ui", "mod") match too much by accident.
                if normalized.len() >= 3 && !NON_MOD_NAMESPACES.contains(&normalized.as_str()) {
                    tokens.push((normalized, index));
                }
            }
        }
        Self {
            tokens,
            keys: mods.iter().map(|identity| identity.key.clone()).collect(),
        }
    }

    fn exact(&self, value: &str) -> Option<String> {
        let normalized = normalize(value);
        self.tokens
            .iter()
            .find(|(token, _)| *token == normalized)
            .map(|(_, index)| self.keys[*index].clone())
    }

    /// `com.author.sts2-ModName` style tags name the mod in their last part.
    fn tag(&self, tag: &str) -> Option<String> {
        self.exact(tag)
            .or_else(|| {
                tag.rsplit(['.', '/', ':'])
                    .next()
                    .and_then(|last| self.exact(last))
            })
            .or_else(|| {
                let normalized = normalize(tag);
                self.tokens
                    .iter()
                    .filter(|(token, _)| token.len() >= 5 && normalized.ends_with(token.as_str()))
                    .max_by_key(|(token, _)| token.len())
                    .map(|(_, index)| self.keys[*index].clone())
            })
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn attribute(lines: &[&str], index: &TokenIndex) -> Option<(String, &'static str)> {
    let header = lines[0];
    if let Some(key) = loader_mod_id(header).and_then(|id| index.exact(id)) {
        return Some((key, "mod_id"));
    }
    if let Some(key) = logger_tag(header).and_then(|tag| index.tag(tag)) {
        return Some((key, "log_tag"));
    }
    for line in lines {
        if let Some(key) = path_segments(line).find_map(|segment| index.exact(segment)) {
            return Some((key, "path"));
        }
    }
    for line in &lines[1..] {
        if let Some(key) = frame_namespaces(line).find_map(|namespace| index.exact(&namespace)) {
            return Some((key, "stack"));
        }
    }
    None
}

/// `Tried to load mod with id <id>, but ...`
fn loader_mod_id(header: &str) -> Option<&str> {
    let start = header.find("mod with id ")? + "mod with id ".len();
    let rest = &header[start..];
    Some(rest[..rest.find(',')?].trim())
}

/// `[ERROR] [Tag] message` → `Tag`.
fn logger_tag(header: &str) -> Option<&str> {
    let rest = strip_level(header).strip_prefix('[')?;
    let tag = &rest[..rest.find(']')?];
    (!tag.trim().is_empty()).then(|| tag.trim())
}

/// Folders that name a mod: `res://<folder>/`, `workshop/content/<app>/<item>/`
/// and `mods/<folder>/` (with either slash).
fn path_segments(line: &str) -> impl Iterator<Item = &str> {
    let mut segments = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("res://") {
        rest = &rest[start + "res://".len()..];
        segments.push(first_segment(rest));
    }
    let lower = line.to_ascii_lowercase();
    for marker in ["workshop/content/", "workshop\\content\\"] {
        if let Some(start) = lower.find(marker) {
            let after_app = &line[start + marker.len()..];
            if let Some(slash) = after_app.find(['/', '\\']) {
                segments.push(first_segment(&after_app[slash + 1..]));
            }
        }
    }
    for marker in ["/mods/", "\\mods\\"] {
        if let Some(start) = lower.find(marker) {
            segments.push(first_segment(&line[start + marker.len()..]));
        }
    }
    segments.into_iter().filter(|segment| !segment.is_empty())
}

fn first_segment(path: &str) -> &str {
    let end = path
        .find(|ch: char| ch == '/' || ch == '\\' || ch == '\'' || ch == '"' || ch.is_whitespace())
        .unwrap_or(path.len());
    &path[..end]
}

/// The called method of a stack frame (`at A.B.C(...)` or Godot's
/// `[3] void A.B.C(...)`), without Harmony's `<unknown>.` prefix.
fn frame_method(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let frame = if let Some(rest) = trimmed.strip_prefix("at ") {
        rest
    } else if trimmed.starts_with('[') {
        &trimmed[trimmed.find(']')? + 1..]
    } else {
        return None;
    };
    let call = &frame[..frame.find('(')?];
    let method = call.rsplit(' ').next()?.trim();
    Some(method.strip_prefix("<unknown>.").unwrap_or(method))
}

/// The leading namespace parts of a frame: `Mod.Cards.Foo.Bar` gives `Mod`
/// and `ModCards`, skipping game and runtime namespaces.
fn frame_namespaces(line: &str) -> impl Iterator<Item = String> {
    let parts = frame_method(line)
        .map(|method| {
            method
                .split(['.', '+'])
                .take(2)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let skip = parts
        .first()
        .is_none_or(|first| NON_MOD_NAMESPACES.contains(&normalize(first).as_str()));
    let mut candidates = Vec::new();
    if !skip {
        candidates.push(parts[0].clone());
        if parts.len() > 1 {
            candidates.push(format!("{}{}", parts[0], parts[1]));
        }
    }
    candidates.into_iter()
}

/// Harmony renames a method it patched to `Method_Patch<N>`.
fn is_patched_frame(line: &str) -> bool {
    frame_method(line).is_some_and(|method| {
        method
            .rsplit('.')
            .next()
            .and_then(|name| name.rsplit_once("_Patch"))
            .is_some_and(|(_, number)| {
                !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods() -> Vec<ModIdentity> {
        vec![
            ModIdentity {
                key: "workshop:sakiko".to_string(),
                tokens: vec!["CuteSakikoMod".to_string(), "3747559888".to_string()],
            },
            ModIdentity {
                key: "workshop:ritsulib".to_string(),
                tokens: vec!["STS2-RitsuLib".to_string()],
            },
            ModIdentity {
                key: "workshop:manosaba".to_string(),
                tokens: vec!["Manosaba".to_string()],
            },
            ModIdentity {
                key: "game:ovelle".to_string(),
                tokens: vec!["Ovelle".to_string()],
            },
        ]
    }

    #[test]
    fn traces_errors_to_mods_by_id_tag_path_and_stack() {
        let log = "\
[INFO] Loading mods
[ERROR] Tried to load mod with id Manosaba, but the current Steam branch public-beta does not lie in the min/max steam branches the mod supports! Max supported: public
   at MegaCrit.Sts2.Core.Modding.ModManager.CheckSteamBranchSupport()

[ERROR] [com.ritsukage.sts2-RitsuLib] [RitsuLib-ModSettings] Could not load translation asset 'localization/settings/kor.json'
ERROR: No loader found for resource: res://CuteSakikoMod/images/ui/map/StarAnon_outline.png (expected type: unknown)
   at: _load (core/io/resource_loader.cpp:328)
   C# backtrace (most recent call first):
       [0] Godot.Resource <unknown>.MegaCrit.Sts2.Core.Assets.AssetCache.LoadAsset_Patch1(MegaCrit.Sts2.Core.Assets.AssetCache, string)
[ERROR] System.NullReferenceException: Object reference not set to an instance of an object.
   at Ovelle.Powers.ShieldPower.AfterTurn(Player player)
   at MegaCrit.Sts2.Core.Hooks.Hook.AfterEnergyReset(ICombatState combatState, Player player)
[INFO] done
";
        let errors = collect_errors(log, &mods());
        let found = errors
            .iter()
            .map(|error| (error.mod_key.as_deref(), error.matched_by))
            .collect::<Vec<_>>();
        assert_eq!(
            found,
            vec![
                (Some("workshop:manosaba"), Some("mod_id")),
                (Some("workshop:ritsulib"), Some("log_tag")),
                (Some("workshop:sakiko"), Some("path")),
                (Some("game:ovelle"), Some("stack")),
            ]
        );
        assert!(errors[2].patched_game_code);
        assert!(errors[3].text.contains("ShieldPower.AfterTurn"));
        assert!(!errors[3].text.contains("[INFO] done"));
    }

    #[test]
    fn counts_repeats_and_skips_shutdown_noise() {
        let log = "\
[ERROR] System.NullReferenceException: Object reference not set to an instance of an object.
   at MegaCrit.Sts2.Core.Hooks.Hook.AfterEnergyReset_Patch3(ICombatState combatState, Player player)
[ERROR] System.NullReferenceException: Object reference not set to an instance of an object.
   at MegaCrit.Sts2.Core.Hooks.Hook.AfterEnergyReset_Patch3(ICombatState combatState, Player player)
ERROR: 27 RID allocations of type 'N10RendererRD11MeshStorage4MeshE' were leaked at exit.
";
        let errors = collect_errors(log, &mods());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].count, 2);
        assert_eq!(errors[0].mod_key, None);
        assert!(errors[0].patched_game_code);
        assert_eq!(errors[0].first_line, 1);
    }

    #[test]
    fn matches_workshop_install_paths() {
        let log = "[ERROR] Failed to read G:\\Steam\\steamapps\\workshop\\content\\2868840\\3747559888\\CuteSakikoMod.dll\n";
        let errors = collect_errors(log, &mods());
        assert_eq!(errors[0].mod_key.as_deref(), Some("workshop:sakiko"));
    }
}
