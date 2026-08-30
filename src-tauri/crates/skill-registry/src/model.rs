use std::{fmt, str::FromStr};

use thiserror::Error;

/// Identity assigned by a remote registry.
///
/// This is deliberately separate from `skill_core::SkillId`: a registry
/// identity is scoped by its remote source and is not a local installed-skill
/// identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegistrySkillId {
    pub source: String,
    pub skill_id: String,
}

impl RegistrySkillId {
    pub fn new(
        source: impl Into<String>,
        skill_id: impl Into<String>,
    ) -> Result<Self, RegistrySkillIdError> {
        let source = source.into();
        let skill_id = skill_id.into();

        if source.trim().is_empty() {
            return Err(RegistrySkillIdError::EmptySource);
        }
        if skill_id.trim().is_empty() {
            return Err(RegistrySkillIdError::EmptySkillId);
        }
        if source.chars().any(char::is_control) || skill_id.chars().any(char::is_control) {
            return Err(RegistrySkillIdError::ContainsControlCharacter);
        }

        Ok(Self {
            source: source.trim().to_owned(),
            skill_id: skill_id.trim().to_owned(),
        })
    }

    pub fn key(&self) -> String {
        format!("{}\u{0}{}", self.source, self.skill_id)
    }
}

impl fmt::Display for RegistrySkillId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.source, self.skill_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RegistrySkillIdError {
    #[error("registry skill source must not be empty")]
    EmptySource,
    #[error("registry skill id must not be empty")]
    EmptySkillId,
    #[error("registry skill identity must not contain control characters")]
    ContainsControlCharacter,
}

/// Explicit source classification supplied by a registry response or an
/// application-level source selection. Missing classification is represented
/// by `None` on `RemoteSkillSummary`; this enum's `Unknown` variant is useful
/// when a source explicitly reports that it does not know its kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceKind {
    GitHub,
    WellKnown,
    Unknown,
    /// Preserve a source kind introduced by a remote registry instead of
    /// silently treating it as a GitHub repository.
    Other(String),
}

impl SourceKind {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" | "git-hub" | "git_hub" => Self::GitHub,
            "well-known" | "well_known" | "wellknown" => Self::WellKnown,
            "unknown" | "" => Self::Unknown,
            _ => Self::Other(value.trim().to_owned()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::GitHub => "github",
            Self::WellKnown => "well-known",
            Self::Unknown => "unknown",
            Self::Other(value) => value,
        }
    }

    pub fn is_github(&self) -> bool {
        matches!(self, Self::GitHub)
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for SourceKind {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSkillSummary {
    pub id: RegistrySkillId,
    pub name: String,
    pub installs: u64,
    pub source_kind: Option<SourceKind>,
    pub install_url: Option<String>,
    pub is_official: Option<bool>,
    pub skills_sh_url: Option<String>,
    pub rank: Option<u64>,
}

impl RemoteSkillSummary {
    pub fn source(&self) -> &str {
        &self.id.source
    }

    pub fn skill_id(&self) -> &str {
        &self.id.skill_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchResult {
    pub skills: Vec<RemoteSkillSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Leaderboard {
    AllTime,
    Trending,
    Hot,
}

impl Leaderboard {
    pub const fn path(self) -> &'static str {
        match self {
            Self::AllTime => "",
            Self::Trending => "trending",
            Self::Hot => "hot",
        }
    }
}

/// Compatibility name used by some callers of the legacy skills.sh adapter.
pub type LeaderboardType = Leaderboard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderboardResult {
    pub leaderboard: Leaderboard,
    pub skills: Vec<RemoteSkillSummary>,
}
