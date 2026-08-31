use std::{
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HarnessId(String);

impl HarnessId {
    pub fn new(value: &str) -> Result<Self, HarnessIdError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(HarnessIdError::Empty);
        }
        if value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        {
            return Err(HarnessIdError::ContainsPathSeparatorOrControlCharacter);
        }
        Ok(Self(value.to_owned()))
    }

    fn from_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for HarnessId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for HarnessId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HarnessIdError {
    #[error("harness id must not be empty")]
    Empty,
    #[error("harness id must not contain path separators or control characters")]
    ContainsPathSeparatorOrControlCharacter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarnessCategory {
    #[default]
    Coding,
    Lobster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessCapabilities {
    pub supports_global_scope: bool,
    pub supports_project_scope: bool,
    pub recursive_global_discovery: bool,
    pub supports_configuration_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessEnvironment {
    home_dir: PathBuf,
    config_dir: Option<PathBuf>,
}

impl HarnessEnvironment {
    pub fn new(home_dir: impl Into<PathBuf>, config_dir: Option<PathBuf>) -> Self {
        Self {
            home_dir: home_dir.into(),
            config_dir,
        }
    }

    pub fn from_system() -> Result<Self, HarnessError> {
        let home_dir = dirs::home_dir().ok_or(HarnessError::HomeDirectoryUnavailable)?;
        Ok(Self {
            home_dir,
            config_dir: dirs::config_dir(),
        })
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionStatus {
    Installed,
    NotInstalled,
    ExplicitlyConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessDetection {
    pub status: DetectionStatus,
    pub checked_paths: Vec<PathBuf>,
}

impl HarnessDetection {
    pub fn is_installed(&self) -> bool {
        matches!(
            self.status,
            DetectionStatus::Installed | DetectionStatus::ExplicitlyConfigured
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessLocations {
    pub global_skills_dir: PathBuf,
    pub project_skills_dir: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub additional_global_discovery_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomHarnessDefinition {
    pub id: String,
    pub display_name: String,
    pub global_skills_path: String,
    #[serde(default)]
    pub project_skills_path: Option<String>,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default)]
    pub category: HarnessCategory,
}

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("home directory is unavailable")]
    HomeDirectoryUnavailable,
    #[error(transparent)]
    InvalidId(#[from] HarnessIdError),
    #[error("custom harness display name must not be empty")]
    EmptyDisplayName,
    #[error("custom global skills path must be absolute or start with '~/' (received {path})")]
    GlobalSkillsPathMustBeAbsolute { path: String },
    #[error("custom configuration path must be absolute or start with '~/' (received {path})")]
    ConfigurationPathMustBeAbsolute { path: String },
    #[error("project skills path must be relative to the project root: {path}")]
    ProjectSkillsPathMustBeRelative { path: String },
    #[error("project skills path cannot contain parent directory segments: {path}")]
    ProjectSkillsPathContainsParent { path: String },
    #[error("cannot inspect harness path {path}: {source}")]
    PathProbe {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("harness id is already registered: {id}")]
    DuplicateId { id: HarnessId },
}

fn portable_relative_path(raw: &str) -> PathBuf {
    // Built-in and serialized relative paths use `/`, but PathBuf preserves those
    // existing separators when rendered on Windows unless each component is pushed.
    raw.split('/').fold(PathBuf::new(), |mut path, component| {
        path.push(component);
        path
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathRule {
    HomeRelative {
        path: PathBuf,
        config_relative: Option<PathBuf>,
    },
    Absolute(PathBuf),
}

impl PathRule {
    fn home_relative(raw: &str) -> Self {
        Self::HomeRelative {
            path: portable_relative_path(raw),
            config_relative: raw
                .strip_prefix(".config/")
                .filter(|suffix| !suffix.is_empty())
                .map(portable_relative_path),
        }
    }

    fn custom(raw: &str, path_kind: CustomPathKind) -> Result<Self, HarnessError> {
        let raw = raw.trim();
        if raw == "~" {
            return Ok(Self::HomeRelative {
                path: PathBuf::new(),
                config_relative: None,
            });
        }
        if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
            if rest.is_empty() {
                return Ok(Self::HomeRelative {
                    path: PathBuf::new(),
                    config_relative: None,
                });
            }
            return Ok(Self::HomeRelative {
                path: portable_relative_path(rest),
                config_relative: None,
            });
        }

        let path = Path::new(raw);
        if path.is_absolute() {
            return Ok(Self::Absolute(path.to_path_buf()));
        }

        Err(match path_kind {
            CustomPathKind::GlobalSkills => HarnessError::GlobalSkillsPathMustBeAbsolute {
                path: raw.to_owned(),
            },
            CustomPathKind::Configuration => HarnessError::ConfigurationPathMustBeAbsolute {
                path: raw.to_owned(),
            },
        })
    }

    fn candidates(&self, environment: &HarnessEnvironment) -> Vec<PathBuf> {
        match self {
            Self::Absolute(path) => vec![path.clone()],
            Self::HomeRelative {
                path,
                config_relative,
            } => {
                let home_path = environment.home_dir().join(path);
                let mut candidates = vec![home_path.clone()];
                if let (Some(config_relative), Some(config_dir)) =
                    (config_relative, environment.config_dir())
                {
                    let config_path = config_dir.join(config_relative);
                    if config_path != home_path {
                        candidates.push(config_path);
                    }
                }
                candidates
            }
        }
    }

    fn first_candidate(&self, environment: &HarnessEnvironment) -> PathBuf {
        self.candidates(environment)
            .into_iter()
            .next()
            .unwrap_or_else(|| environment.home_dir().to_path_buf())
    }

    fn resolve(&self, environment: &HarnessEnvironment) -> Result<PathBuf, HarnessError> {
        let candidates = self.candidates(environment);
        for candidate in &candidates {
            if path_exists(candidate)? {
                return Ok(candidate.clone());
            }
        }
        Ok(self.first_candidate(environment))
    }

    fn existing(&self, environment: &HarnessEnvironment) -> Result<Vec<PathBuf>, HarnessError> {
        let mut existing = Vec::new();
        for candidate in self.candidates(environment) {
            if path_exists(&candidate)? {
                existing.push(candidate);
            }
        }
        Ok(existing)
    }

    fn first_existing_directory(
        &self,
        environment: &HarnessEnvironment,
    ) -> Result<Option<PathBuf>, HarnessError> {
        for candidate in self.candidates(environment) {
            match fs::metadata(&candidate) {
                Ok(metadata) if metadata.is_dir() => return Ok(Some(candidate)),
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(HarnessError::PathProbe {
                        path: candidate,
                        source,
                    });
                }
            }
        }
        Ok(None)
    }

    fn home_relative_path(&self) -> Option<&Path> {
        match self {
            Self::HomeRelative { path, .. } => Some(path),
            Self::Absolute(_) => None,
        }
    }
}

fn path_exists(path: &Path) -> Result<bool, HarnessError> {
    path.try_exists().map_err(|source| HarnessError::PathProbe {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug, Clone, Copy)]
enum CustomPathKind {
    GlobalSkills,
    Configuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessAdapter {
    id: HarnessId,
    display_name: String,
    category: HarnessCategory,
    global_skills_path: PathRule,
    detection_path: Option<PathRule>,
    project_skills_path: Option<PathBuf>,
    additional_global_discovery_paths: Vec<PathRule>,
    recursive_global_discovery: bool,
    custom: bool,
}

impl HarnessAdapter {
    pub fn with_agent_configuration(
        &self,
        id: HarnessId,
        display_name: impl Into<String>,
        global_skills_path: PathBuf,
    ) -> Result<Self, HarnessError> {
        let display_name = display_name.into();
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(HarnessError::EmptyDisplayName);
        }
        if !global_skills_path.is_absolute() {
            return Err(HarnessError::GlobalSkillsPathMustBeAbsolute {
                path: global_skills_path.to_string_lossy().into_owned(),
            });
        }
        let mut configured = self.clone();
        configured.id = id;
        configured.display_name = display_name.to_owned();
        configured.global_skills_path = PathRule::Absolute(global_skills_path);
        Ok(configured)
    }

    pub fn for_configured_agent(
        id: HarnessId,
        display_name: impl Into<String>,
        global_skills_path: PathBuf,
        category: HarnessCategory,
    ) -> Result<Self, HarnessError> {
        let display_name = display_name.into();
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(HarnessError::EmptyDisplayName);
        }
        if !global_skills_path.is_absolute() {
            return Err(HarnessError::GlobalSkillsPathMustBeAbsolute {
                path: global_skills_path.to_string_lossy().into_owned(),
            });
        }
        Ok(Self {
            id,
            display_name: display_name.to_owned(),
            category,
            global_skills_path: PathRule::Absolute(global_skills_path),
            detection_path: None,
            project_skills_path: None,
            additional_global_discovery_paths: Vec::new(),
            recursive_global_discovery: false,
            custom: true,
        })
    }

    pub fn from_custom(definition: CustomHarnessDefinition) -> Result<Self, HarnessError> {
        let id = HarnessId::new(&definition.id)?;
        let display_name = definition.display_name.trim();
        if display_name.is_empty() {
            return Err(HarnessError::EmptyDisplayName);
        }

        let global_skills_path =
            PathRule::custom(&definition.global_skills_path, CustomPathKind::GlobalSkills)?;
        let project_skills_path =
            normalize_project_path(definition.project_skills_path.as_deref())?;
        let detection_path = definition
            .config_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .map(|path| PathRule::custom(path, CustomPathKind::Configuration))
            .transpose()?;

        Ok(Self {
            id,
            display_name: display_name.to_owned(),
            category: definition.category,
            global_skills_path,
            detection_path,
            project_skills_path,
            additional_global_discovery_paths: Vec::new(),
            recursive_global_discovery: false,
            custom: true,
        })
    }

    fn from_builtin(spec: &BuiltinSpec) -> Self {
        let project_path = spec.project.unwrap_or(spec.global);
        Self {
            id: HarnessId::from_static(spec.id),
            display_name: spec.display_name.to_owned(),
            category: spec.category,
            global_skills_path: PathRule::home_relative(spec.global),
            detection_path: Some(PathRule::home_relative(spec.detect)),
            project_skills_path: Some(portable_relative_path(project_path)),
            additional_global_discovery_paths: spec
                .additional
                .iter()
                .map(|path| PathRule::home_relative(path))
                .collect(),
            recursive_global_discovery: spec.recursive,
            custom: false,
        }
    }

    pub fn id(&self) -> &HarnessId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn category(&self) -> HarnessCategory {
        self.category
    }

    pub fn is_custom(&self) -> bool {
        self.custom
    }

    pub fn capabilities(&self) -> HarnessCapabilities {
        HarnessCapabilities {
            supports_global_scope: true,
            supports_project_scope: self.project_skills_path.is_some(),
            recursive_global_discovery: self.recursive_global_discovery,
            supports_configuration_path: self.detection_path.is_some(),
        }
    }

    pub fn project_relative_skills_path(&self) -> Option<&Path> {
        self.project_skills_path.as_deref()
    }

    pub fn additional_global_discovery_paths(&self) -> impl Iterator<Item = &Path> {
        self.additional_global_discovery_paths
            .iter()
            .filter_map(PathRule::home_relative_path)
    }

    pub fn existing_global_skills_dir(
        &self,
        environment: &HarnessEnvironment,
    ) -> Result<Option<PathBuf>, HarnessError> {
        self.global_skills_path
            .first_existing_directory(environment)
    }

    pub fn resolve_locations(
        &self,
        environment: &HarnessEnvironment,
        project_root: Option<&Path>,
    ) -> Result<HarnessLocations, HarnessError> {
        let global_skills_dir = self.global_skills_path.resolve(environment)?;
        let config_dir = self
            .detection_path
            .as_ref()
            .map(|path| path.resolve(environment))
            .transpose()?;
        let project_skills_dir = match (project_root, self.project_skills_path.as_deref()) {
            (Some(project_root), Some(relative_path)) => Some(project_root.join(relative_path)),
            _ => None,
        };
        let mut additional_global_discovery_dirs = Vec::new();
        for path in &self.additional_global_discovery_paths {
            for existing in path.existing(environment)? {
                if !additional_global_discovery_dirs.contains(&existing) {
                    additional_global_discovery_dirs.push(existing);
                }
            }
        }

        Ok(HarnessLocations {
            global_skills_dir,
            project_skills_dir,
            config_dir,
            additional_global_discovery_dirs,
        })
    }

    pub fn detect(
        &self,
        environment: &HarnessEnvironment,
    ) -> Result<HarnessDetection, HarnessError> {
        let Some(detection_path) = self.detection_path.as_ref() else {
            return Ok(HarnessDetection {
                status: DetectionStatus::ExplicitlyConfigured,
                checked_paths: vec![self.global_skills_path.first_candidate(environment)],
            });
        };

        let candidates = detection_path.candidates(environment);
        let mut checked_paths = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let exists = path_exists(&candidate)?;
            checked_paths.push(candidate);
            if exists {
                return Ok(HarnessDetection {
                    status: DetectionStatus::Installed,
                    checked_paths,
                });
            }
        }

        Ok(HarnessDetection {
            status: DetectionStatus::NotInstalled,
            checked_paths,
        })
    }
}

fn normalize_project_path(raw: Option<&str>) -> Result<Option<PathBuf>, HarnessError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }

    let path = Path::new(raw);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(HarnessError::ProjectSkillsPathMustBeRelative {
            path: raw.to_owned(),
        });
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(HarnessError::ProjectSkillsPathContainsParent {
            path: raw.to_owned(),
        });
    }

    let normalized = raw.trim_matches(['/', '\\']);
    if normalized.is_empty() {
        return Err(HarnessError::ProjectSkillsPathMustBeRelative {
            path: raw.to_owned(),
        });
    }
    Ok(Some(portable_relative_path(normalized)))
}

#[derive(Debug, Clone, Copy)]
struct BuiltinSpec {
    id: &'static str,
    display_name: &'static str,
    global: &'static str,
    detect: &'static str,
    project: Option<&'static str>,
    additional: &'static [&'static str],
    category: HarnessCategory,
    recursive: bool,
}

const NO_ADDITIONAL: &[&str] = &[];
const SHARED_AGENTS: &[&str] = &[".agents/skills"];

const BUILTIN_SPECS: &[BuiltinSpec] = &[
    BuiltinSpec {
        id: "cursor",
        display_name: "Cursor",
        global: ".cursor/skills",
        detect: ".cursor",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "claude_code",
        display_name: "Claude Code",
        global: ".claude/skills",
        detect: ".claude",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "omp_agent",
        display_name: "OMP Agent",
        global: ".omp/agent/skills",
        detect: ".omp/agent",
        project: Some(".omp/skills"),
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "codex",
        display_name: "Codex",
        global: ".codex/skills",
        detect: ".codex",
        project: None,
        additional: SHARED_AGENTS,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "grok",
        display_name: "Grok",
        global: ".grok/skills",
        detect: ".grok",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "opencode",
        display_name: "OpenCode",
        global: ".config/opencode/skills",
        detect: ".config/opencode",
        project: Some(".opencode/skills"),
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "antigravity",
        display_name: "Antigravity",
        global: ".gemini/antigravity/skills",
        detect: ".gemini/antigravity",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "amp",
        display_name: "Amp",
        global: ".config/agents/skills",
        detect: ".config/agents",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "kilo_code",
        display_name: "Kilo Code",
        global: ".kilocode/skills",
        detect: ".kilocode",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "roo_code",
        display_name: "Roo Code",
        global: ".roo/skills",
        detect: ".roo",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "goose",
        display_name: "Goose",
        global: ".config/goose/skills",
        detect: ".config/goose",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "gemini_cli",
        display_name: "Gemini CLI",
        global: ".gemini/skills",
        detect: ".gemini",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "github_copilot",
        display_name: "GitHub Copilot",
        global: ".copilot/skills",
        detect: ".copilot",
        project: None,
        additional: SHARED_AGENTS,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "openclaw",
        display_name: "OpenClaw",
        global: ".openclaw/skills",
        detect: ".openclaw",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: false,
    },
    BuiltinSpec {
        id: "droid",
        display_name: "Droid",
        global: ".factory/skills",
        detect: ".factory",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "windsurf",
        display_name: "Windsurf",
        global: ".codeium/windsurf/skills",
        detect: ".codeium/windsurf",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "trae",
        display_name: "TRAE IDE",
        global: ".trae/skills",
        detect: ".trae",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "cline",
        display_name: "Cline",
        global: ".agents/skills",
        detect: ".cline",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "deepagents",
        display_name: "Deep Agents",
        global: ".deepagents/agent/skills",
        detect: ".deepagents",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "firebender",
        display_name: "Firebender",
        global: ".firebender/skills",
        detect: ".firebender",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "kimi",
        display_name: "Kimi Code CLI",
        global: ".kimi-code/skills",
        detect: ".kimi-code",
        project: Some(".kimi-code/skills"),
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "replit",
        display_name: "Replit",
        global: ".config/agents/skills",
        detect: ".replit",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "warp",
        display_name: "Warp",
        global: ".agents/skills",
        detect: ".warp",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "augment",
        display_name: "Augment",
        global: ".augment/skills",
        detect: ".augment",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "bob",
        display_name: "IBM Bob",
        global: ".bob/skills",
        detect: ".bob",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "codebuddy",
        display_name: "CodeBuddy",
        global: ".codebuddy/skills",
        detect: ".codebuddy",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "command_code",
        display_name: "Command Code",
        global: ".commandcode/skills",
        detect: ".commandcode",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "continue",
        display_name: "Continue",
        global: ".continue/skills",
        detect: ".continue",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "cortex",
        display_name: "Cortex Code",
        global: ".snowflake/cortex/skills",
        detect: ".snowflake/cortex",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "crush",
        display_name: "Crush",
        global: ".config/crush/skills",
        detect: ".config/crush",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "iflow",
        display_name: "iFlow CLI",
        global: ".iflow/skills",
        detect: ".iflow",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "junie",
        display_name: "Junie",
        global: ".junie/skills",
        detect: ".junie",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "kiro",
        display_name: "Kiro CLI",
        global: ".kiro/skills",
        detect: ".kiro",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "kode",
        display_name: "Kode",
        global: ".kode/skills",
        detect: ".kode",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "mcpjam",
        display_name: "MCPJam",
        global: ".mcpjam/skills",
        detect: ".mcpjam",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "mistral_vibe",
        display_name: "Mistral Vibe",
        global: ".vibe/skills",
        detect: ".vibe",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "mux",
        display_name: "Mux",
        global: ".mux/skills",
        detect: ".mux",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "neovate",
        display_name: "Neovate",
        global: ".neovate/skills",
        detect: ".neovate",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "openhands",
        display_name: "OpenHands",
        global: ".openhands/skills",
        detect: ".openhands",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "pi",
        display_name: "Pi",
        global: ".pi/agent/skills",
        detect: ".pi/agent",
        project: Some(".pi/skills"),
        additional: SHARED_AGENTS,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "pochi",
        display_name: "Pochi",
        global: ".pochi/skills",
        detect: ".pochi",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "qoder",
        display_name: "Qoder",
        global: ".qoder/skills",
        detect: ".qoder",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "qwen_code",
        display_name: "Qwen Code",
        global: ".qwen/skills",
        detect: ".qwen",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "trae_cn",
        display_name: "TRAE CN",
        global: ".trae-cn/skills",
        detect: ".trae-cn",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "zencoder",
        display_name: "Zencoder",
        global: ".zencoder/skills",
        detect: ".zencoder",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "zcode",
        display_name: "ZCode",
        global: ".zcode/skills",
        detect: ".zcode",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "adal",
        display_name: "AdaL",
        global: ".adal/skills",
        detect: ".adal",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Coding,
        recursive: false,
    },
    BuiltinSpec {
        id: "hermes",
        display_name: "Hermes Agent",
        global: ".hermes/skills",
        detect: ".hermes",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: true,
    },
    BuiltinSpec {
        id: "qclaw",
        display_name: "QClaw",
        global: ".qclaw/skills",
        detect: ".qclaw",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: false,
    },
    BuiltinSpec {
        id: "easyclaw",
        display_name: "EasyClaw",
        global: ".easyclaw/skills",
        detect: ".easyclaw",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: false,
    },
    BuiltinSpec {
        id: "autoclaw",
        display_name: "AutoClaw",
        global: ".openclaw-autoclaw/skills",
        detect: ".openclaw-autoclaw",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: false,
    },
    BuiltinSpec {
        id: "workbuddy",
        display_name: "WorkBuddy",
        global: ".workbuddy/skills",
        detect: ".workbuddy",
        project: None,
        additional: NO_ADDITIONAL,
        category: HarnessCategory::Lobster,
        recursive: false,
    },
    BuiltinSpec {
        id: "deepseek_harness",
        display_name: "DeepSeek Harness",
        global: ".dsh/skills",
        detect: ".dsh",
        project: None,
        additional: SHARED_AGENTS,
        category: HarnessCategory::Coding,
        recursive: false,
    },
];

pub fn default_harnesses() -> Vec<HarnessAdapter> {
    BUILTIN_SPECS
        .iter()
        .map(HarnessAdapter::from_builtin)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessRegistry {
    adapters: Vec<HarnessAdapter>,
}

impl HarnessRegistry {
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn with_builtins() -> Self {
        Self {
            adapters: default_harnesses(),
        }
    }

    pub fn register(&mut self, adapter: HarnessAdapter) -> Result<(), HarnessError> {
        if self.find(adapter.id().as_str()).is_some() {
            return Err(HarnessError::DuplicateId {
                id: adapter.id().clone(),
            });
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&HarnessAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.id().as_str() == id)
    }

    pub fn adapters(&self) -> impl Iterator<Item = &HarnessAdapter> {
        self.adapters.iter()
    }
}

impl Default for HarnessRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
