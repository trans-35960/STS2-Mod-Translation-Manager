use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    GameMods,
    Vault,
    ExternalManager,
}

impl Display for ModSource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameMods => formatter.write_str("game mods"),
            Self::Vault => formatter.write_str("managed vault"),
            Self::ExternalManager => formatter.write_str("external manager"),
        }
    }
}

impl ModSource {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::GameMods => "game",
            Self::Vault => "vault",
            Self::ExternalManager => "external",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "game" => Some(Self::GameMods),
            "vault" => Some(Self::Vault),
            "external" => Some(Self::ExternalManager),
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

impl ModRecord {
    pub fn stable_key(&self) -> String {
        self.name
            .chars()
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationCandidate {
    pub path: PathBuf,
    pub extension: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    pub game_mods: Vec<ModRecord>,
    pub vault_mods: Vec<ModRecord>,
    pub external_manager_mods: Vec<ModRecord>,
}

impl ScanSummary {
    pub fn total_mods(&self) -> usize {
        self.game_mods.len() + self.vault_mods.len() + self.external_manager_mods.len()
    }

    pub fn is_vanilla_safe(&self) -> bool {
        self.game_mods.is_empty()
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
