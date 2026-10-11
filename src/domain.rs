use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    GameMods,
    Disabled,
    ExternalManager,
    Workshop,
}

impl Display for ModSource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameMods => formatter.write_str("game mods"),
            Self::Disabled => formatter.write_str("disabled mods"),
            Self::ExternalManager => formatter.write_str("external manager"),
            Self::Workshop => formatter.write_str("steam workshop"),
        }
    }
}

impl ModSource {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::GameMods => "game",
            Self::Disabled => "disabled",
            Self::ExternalManager => "external",
            Self::Workshop => "workshop",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "game" => Some(Self::GameMods),
            "disabled" => Some(Self::Disabled),
            "external" => Some(Self::ExternalManager),
            "workshop" => Some(Self::Workshop),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Directory,
    Archive,
    Package,
    UnknownFile,
}

impl Display for ModKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Directory => formatter.write_str("directory"),
            Self::Archive => formatter.write_str("archive"),
            Self::Package => formatter.write_str("package"),
            Self::UnknownFile => formatter.write_str("file"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModFingerprint {
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRecord {
    pub name: String,
    pub path: PathBuf,
    pub source: ModSource,
    pub kind: ModKind,
    pub version_hint: Option<String>,
    pub fingerprint: ModFingerprint,
}

/// Key prefix that keeps Steam Workshop mods apart from same-named local copies:
/// the game loads both, so they must stay separate rows with separate toggles.
pub const WORKSHOP_KEY_PREFIX: &str = "workshop-";

impl ModRecord {
    pub fn stable_key(&self) -> String {
        let key = normalized_key(&self.name);
        if self.source == ModSource::Workshop {
            format!("{WORKSHOP_KEY_PREFIX}{key}")
        } else {
            key
        }
    }

    pub fn is_workshop(&self) -> bool {
        self.source == ModSource::Workshop
    }

    /// Steam published file id, i.e. the `content/<appid>/<id>` folder above the manifest.
    pub fn workshop_item_id(&self) -> Option<String> {
        self.is_workshop()
            .then(|| workshop_item_id_for_path(&self.path))
            .flatten()
    }
}

pub fn workshop_item_id_for_path(path: &Path) -> Option<String> {
    path.ancestors()
        .filter_map(|ancestor| ancestor.file_name()?.to_str())
        .find(|name| name.len() >= 6 && name.chars().all(|character| character.is_ascii_digit()))
        .map(str::to_string)
}

fn normalized_key(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationCandidate {
    pub path: PathBuf,
    pub extension: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanSummary {
    pub game_mods: Vec<ModRecord>,
    pub disabled_mods: Vec<ModRecord>,
    pub external_manager_mods: Vec<ModRecord>,
    pub workshop_mods: Vec<ModRecord>,
    /// Workshop mod keys the game's settings.save marks `is_enabled: false`.
    pub workshop_disabled_in_game: BTreeSet<String>,
}

impl ScanSummary {
    pub fn total_mods(&self) -> usize {
        self.game_mods.len()
            + self.disabled_mods.len()
            + self.external_manager_mods.len()
            + self.workshop_mods.len()
    }

    pub fn is_vanilla_safe(&self) -> bool {
        self.game_mods.is_empty()
    }

    pub fn records(&self) -> impl Iterator<Item = &ModRecord> {
        self.game_mods
            .iter()
            .chain(self.disabled_mods.iter())
            .chain(self.external_manager_mods.iter())
            .chain(self.workshop_mods.iter())
    }

    pub fn into_records(self) -> impl Iterator<Item = ModRecord> {
        self.game_mods
            .into_iter()
            .chain(self.disabled_mods)
            .chain(self.external_manager_mods)
            .chain(self.workshop_mods)
    }

    pub fn workshop_enabled_in_game(&self, key: &str) -> bool {
        !self.workshop_disabled_in_game.contains(key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModChangeKind {
    New,
    Updated,
}

impl Display for ModChangeKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => formatter.write_str("new"),
            Self::Updated => formatter.write_str("updated"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModChange {
    pub kind: ModChangeKind,
    pub record: ModRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub summary: ScanSummary,
    pub changes: Vec<ModChange>,
}
