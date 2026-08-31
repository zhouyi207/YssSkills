use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use skill_core::{SkillId, SkillIdError};
use skill_harness::{
    DetectionStatus, HarnessAdapter, HarnessCapabilities, HarnessCategory, HarnessEnvironment,
    HarnessError, HarnessId, HarnessRegistry,
};
use skill_index::IndexState;
use skill_local::{
    copy_skill, link_skill, link_target, ExistingDestination, LocalError, ScanDiagnostic, ScanMode,
    ScanReport,
};
use skill_workspace::{
    choose_newest_local, resolve_workspace, CatalogFailure, CentralCatalogPort,
    CentralSkillSnapshot, DeploymentMode, DeploymentStatus, LocalCandidate, LocalSkillPort,
    ReconcileReport, SkillVersion, SystemLocalSkillPort, WorkspaceEngine, WorkspaceError,
    WorkspaceId, WorkspaceIdError, WorkspaceKind, WorkspaceReport, WorkspaceResolution,
};
use thiserror::Error;

use crate::agent_config::{AgentConfigError, AgentConfigStore, StoredAgentConfig};
use crate::persistence::{
    CatalogActivityKind, CatalogIndexWorkerConfig, PersistenceError, PersistentCatalog,
    StoredWorkspace,
};

const MAX_WORKSPACE_NAME_CHARS: usize = 120;
const DASHBOARD_WEEK_COUNT: i64 = 12;
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
const EXPORT_DIRECTORY_PREFIX: &str = "yss-export";
const AGENT_SKILLS_DIRECTORY_NAME: &str = "skills";

pub struct Application {
    catalog: PersistentCatalog,
    harnesses: HarnessRegistry,
    environment: HarnessEnvironment,
    local: SystemLocalSkillPort,
    agent_configs: AgentConfigStore,
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Harness(#[from] HarnessError),
    #[error(transparent)]
    Local(#[from] LocalError),
    #[error(transparent)]
    Catalog(#[from] CatalogFailure),
    #[error(transparent)]
    AgentConfig(#[from] AgentConfigError),
    #[error("invalid request field {field}: {reason}")]
    InvalidRequest {
        field: &'static str,
        reason: &'static str,
    },
    #[error("skill id is invalid")]
    InvalidSkillId(#[source] SkillIdError),
    #[error("workspace id is invalid")]
    InvalidWorkspaceId(#[source] WorkspaceIdError),
    #[error("workspace content changed while reconciliation was being prepared")]
    WorkspaceChangedDuringReconcile,
}

#[derive(Debug, Clone)]
pub struct DashboardOverview {
    pub counts: DashboardCounts,
    pub activity: Vec<DashboardActivityPeriod>,
    pub diagnostics: Vec<ApplicationDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
pub struct DashboardCounts {
    pub skills: usize,
    pub deployments: usize,
    pub detected_harnesses: usize,
    pub workspaces: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DashboardActivityPeriod {
    pub period_start_epoch_millis: i64,
    pub imported: usize,
    pub updated: usize,
}

#[derive(Debug, Clone)]
pub struct ApplicationDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CatalogSkillSummary {
    pub snapshot: CentralSkillSnapshot,
    pub deployment_count: usize,
}

#[derive(Debug, Clone)]
pub struct CatalogSkillList {
    pub skills: Vec<CatalogSkillSummary>,
    pub diagnostics: Vec<CatalogSkillIndexDiagnostic>,
    pub freshness: CatalogIndexFreshness,
    pub revision: i64,
    pub last_reconciled_at_epoch_millis: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CatalogSkillIndexDiagnostic {
    pub skill_id: SkillId,
    pub path: PathBuf,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogIndexFreshness {
    Fresh,
    Revalidating,
    Stale,
}

#[derive(Debug, Clone)]
pub struct CatalogIndexRebuildOutcome {
    pub inserted: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub invalid: usize,
    pub revision: i64,
}

#[derive(Debug, Clone)]
pub struct CatalogSkillDetail {
    pub summary: CatalogSkillSummary,
    pub body: String,
}

#[derive(Debug)]
pub struct ImportFolderPreview {
    pub root: PathBuf,
    pub candidates: Vec<ImportCandidate>,
    pub diagnostics: Vec<ImportFolderDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
}

#[derive(Debug)]
pub struct ImportFolderDiagnostic {
    pub path: PathBuf,
    pub error: LocalError,
}

#[derive(Debug)]
pub struct ImportSkillsOutcome {
    pub imported: Vec<SkillId>,
    pub skipped: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct ExportSkillsOutcome {
    pub export_root: PathBuf,
    pub exported: Vec<SkillId>,
}

#[derive(Debug)]
pub struct SetAgentSkillsOutcome {
    pub skills_root: PathBuf,
    pub linked: Vec<SkillId>,
    pub removed: Vec<SkillId>,
}

#[derive(Debug, Clone)]
pub struct SaveAgentInput {
    pub agent_id: Option<String>,
    pub display_name: String,
    pub agent_root: PathBuf,
    pub skill_ids: Vec<String>,
}

#[derive(Debug)]
pub struct SaveAgentOutcome {
    pub agent_id: HarnessId,
    pub display_name: String,
    pub agent_root: PathBuf,
    pub skills: SetAgentSkillsOutcome,
}

#[derive(Debug)]
pub struct WorkspacesOverview {
    pub agents_workspace_id: WorkspaceId,
    pub harnesses: Vec<HarnessOverview>,
    pub workspaces: Vec<WorkspaceSummary>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    pub stored: StoredWorkspace,
    pub deployment_count: usize,
}

#[derive(Debug)]
pub struct HarnessOverview {
    pub id: HarnessId,
    pub display_name: String,
    pub category: HarnessCategory,
    pub custom: bool,
    pub capabilities: HarnessCapabilities,
    pub skill_count: usize,
    pub linked_skill_ids: Vec<SkillId>,
    pub probe: Result<HarnessProbe, HarnessError>,
    pub scan_error: Option<LocalError>,
}

#[derive(Debug, Clone)]
pub struct HarnessProbe {
    pub detection_status: DetectionStatus,
    pub checked_paths: Vec<PathBuf>,
    pub global_skills_path: PathBuf,
}

#[derive(Debug, Clone)]
struct AgentConfiguration {
    id: HarnessId,
    detector_id: Option<HarnessId>,
    display_name: String,
    agent_root: PathBuf,
    category: HarnessCategory,
    custom: bool,
    detection_status: DetectionStatus,
    checked_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct AgentDetectionOutcome {
    pub agents: Vec<DetectedAgent>,
    pub diagnostics: Vec<AgentDetectionDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct DetectedAgent {
    pub detector_id: HarnessId,
    pub display_name: String,
    pub agent_root: PathBuf,
    pub skill_count: usize,
    pub configured: bool,
}

#[derive(Debug)]
pub struct AgentDetectionDiagnostic {
    pub detector_id: HarnessId,
    pub display_name: String,
    pub error: AgentDetectionError,
}

#[derive(Debug, Error)]
pub enum AgentDetectionError {
    #[error(transparent)]
    Harness(#[from] HarnessError),
    #[error(transparent)]
    Local(#[from] LocalError),
}

#[derive(Debug)]
pub struct AddDetectedAgentsOutcome {
    pub added_agent_ids: Vec<HarnessId>,
}

#[derive(Debug)]
pub struct DeleteAgentsOutcome {
    pub deleted_agent_ids: Vec<HarnessId>,
    pub deleted_skill_count: usize,
}

#[derive(Debug, Clone)]
pub struct CopyProjectAgentSkillsInput {
    pub workspace_id: String,
    pub agent_root: PathBuf,
    pub skill_ids: Vec<String>,
}

#[derive(Debug)]
pub struct CopyProjectAgentSkillsOutcome {
    pub skills_root: PathBuf,
    pub copied_skill_ids: Vec<SkillId>,
}

#[derive(Debug)]
pub struct DeleteProjectAgentsOutcome {
    pub deleted_agent_ids: Vec<HarnessId>,
    pub deleted_skill_count: usize,
}

#[derive(Debug)]
pub struct WorkspaceObservation {
    pub workspace: WorkspaceSummary,
    pub resolution: WorkspaceResolution,
    pub report: WorkspaceReport,
    pub project_agents: Vec<ProjectAgentOverview>,
}

#[derive(Debug)]
pub struct ProjectAgentOverview {
    pub id: HarnessId,
    pub display_name: String,
    pub path: PathBuf,
    pub skill_count: usize,
    pub error: Option<LocalError>,
}

#[derive(Debug)]
pub struct WorkspaceReconcileOutcome {
    pub requested_workspace: StoredWorkspace,
    pub requested: ReconcileReport,
    pub propagated: Vec<PropagationOutcome>,
}

#[derive(Debug)]
pub struct PropagationOutcome {
    pub workspace: StoredWorkspace,
    pub result: Result<ReconcileReport, ApplicationError>,
}

#[derive(Debug, Clone)]
struct CenterUpdatePlan {
    skill_id: SkillId,
    candidate: LocalCandidate,
}

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub catalog_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CreateWorkspaceInput {
    pub name: String,
    pub kind: CreateWorkspaceKind,
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Clone)]
pub enum CreateWorkspaceKind {
    Project {
        root: PathBuf,
    },
    Linked {
        root: PathBuf,
        disabled_root: Option<PathBuf>,
    },
}

impl Application {
    pub fn open(
        database_path: PathBuf,
        default_catalog_root: PathBuf,
    ) -> Result<Self, ApplicationError> {
        let agent_config_path = database_path.with_file_name("agents.json");
        let catalog = PersistentCatalog::open(database_path, default_catalog_root)?;
        let application = Self {
            catalog,
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::from_system()?,
            local: SystemLocalSkillPort::for_current_platform(),
            agent_configs: AgentConfigStore::open(agent_config_path)?,
        };
        application.validate_catalog_root_against_workspaces(application.catalog.catalog_root())?;
        Ok(application)
    }

    pub(crate) fn catalog_index_worker_config(&self) -> CatalogIndexWorkerConfig {
        self.catalog.catalog_index_worker_config()
    }

    fn agent_configurations(&self) -> Result<Vec<AgentConfiguration>, ApplicationError> {
        let mut configurations = Vec::new();
        for stored in self.agent_configs.list() {
            let id = HarnessId::new(&stored.id).map_err(HarnessError::InvalidId)?;
            let detector_id = stored
                .detector_id
                .as_deref()
                .map(HarnessId::new)
                .transpose()
                .map_err(HarnessError::InvalidId)?;
            let agent_root = PathBuf::from(&stored.agent_root);
            if !agent_root.is_absolute()
                || agent_root
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(AgentConfigError::InvalidData { field: "agentRoot" }.into());
            }
            let detector = detector_id
                .as_ref()
                .and_then(|detector_id| self.harnesses.find(detector_id.as_str()));
            let category = detector
                .map(HarnessAdapter::category)
                .unwrap_or(HarnessCategory::Coding);
            let custom = detector.is_none();
            configurations.push(AgentConfiguration {
                id,
                detector_id,
                display_name: stored.display_name.trim().to_owned(),
                agent_root: agent_root.clone(),
                category,
                custom,
                detection_status: DetectionStatus::ExplicitlyConfigured,
                checked_paths: vec![agent_root.join(AGENT_SKILLS_DIRECTORY_NAME)],
            });
        }

        configurations.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(configurations)
    }

    fn agent_registry(&self) -> Result<HarnessRegistry, ApplicationError> {
        let mut registry = HarnessRegistry::empty();
        for configuration in self.agent_configurations()? {
            let adapter = self.agent_adapter(&configuration)?;
            registry.register(adapter)?;
        }
        Ok(registry)
    }

    fn agent_adapter(
        &self,
        configuration: &AgentConfiguration,
    ) -> Result<HarnessAdapter, ApplicationError> {
        let global_skills_path = configuration.agent_root.join(AGENT_SKILLS_DIRECTORY_NAME);
        Ok(
            match configuration
                .detector_id
                .as_ref()
                .and_then(|detector_id| self.harnesses.find(detector_id.as_str()))
            {
                Some(detector) => detector.with_agent_configuration(
                    configuration.id.clone(),
                    configuration.display_name.clone(),
                    global_skills_path,
                )?,
                None => HarnessAdapter::for_configured_agent(
                    configuration.id.clone(),
                    configuration.display_name.clone(),
                    global_skills_path,
                    configuration.category,
                )?,
            },
        )
    }

    pub fn detect_agents(&self) -> Result<AgentDetectionOutcome, ApplicationError> {
        let configured_agents = self.agent_configurations()?;
        let mut agents = Vec::new();
        let mut diagnostics = Vec::new();
        for adapter in self.harnesses.adapters() {
            let probe = match discover_harness(adapter, &self.environment) {
                Ok(Some(probe)) => probe,
                Ok(None) => continue,
                Err(error) => {
                    diagnostics.push(AgentDetectionDiagnostic {
                        detector_id: adapter.id().clone(),
                        display_name: adapter.display_name().to_owned(),
                        error: error.into(),
                    });
                    continue;
                }
            };
            let Some(agent_root) = probe.global_skills_path.parent().map(Path::to_path_buf) else {
                diagnostics.push(AgentDetectionDiagnostic {
                    detector_id: adapter.id().clone(),
                    display_name: adapter.display_name().to_owned(),
                    error: LocalError::InvalidPath {
                        path: probe.global_skills_path,
                    }
                    .into(),
                });
                continue;
            };
            let scan_mode = if adapter.capabilities().recursive_global_discovery {
                ScanMode::Recursive
            } else {
                ScanMode::Flat
            };
            let skill_count = match self.local.scan(&probe.global_skills_path, scan_mode) {
                Ok(report) => {
                    if let Some(diagnostic) = report.diagnostics.into_iter().next() {
                        diagnostics.push(AgentDetectionDiagnostic {
                            detector_id: adapter.id().clone(),
                            display_name: adapter.display_name().to_owned(),
                            error: diagnostic.error.into(),
                        });
                    }
                    report.skills.len()
                }
                Err(error) => {
                    diagnostics.push(AgentDetectionDiagnostic {
                        detector_id: adapter.id().clone(),
                        display_name: adapter.display_name().to_owned(),
                        error: error.into(),
                    });
                    0
                }
            };
            let configured = configured_agents.iter().any(|configuration| {
                configuration
                    .detector_id
                    .as_ref()
                    .is_some_and(|detector_id| detector_id == adapter.id())
                    || paths_refer_to_same_location(&configuration.agent_root, &agent_root)
            });
            agents.push(DetectedAgent {
                detector_id: adapter.id().clone(),
                display_name: adapter.display_name().to_owned(),
                agent_root,
                skill_count,
                configured,
            });
        }
        agents.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.detector_id.as_str().cmp(right.detector_id.as_str()))
        });
        diagnostics.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        });
        Ok(AgentDetectionOutcome {
            agents,
            diagnostics,
        })
    }

    pub fn add_detected_agents(
        &mut self,
        raw_detector_ids: Vec<String>,
    ) -> Result<AddDetectedAgentsOutcome, ApplicationError> {
        if raw_detector_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "detectorIds",
                reason: "must not be empty",
            });
        }
        let mut detector_ids = Vec::new();
        for raw_id in raw_detector_ids {
            let detector_id = HarnessId::new(&raw_id).map_err(HarnessError::InvalidId)?;
            if !detector_ids.contains(&detector_id) {
                detector_ids.push(detector_id);
            }
        }
        let detection = self.detect_agents()?;
        let mut records = Vec::new();
        let mut added_agent_ids = Vec::new();
        for detector_id in detector_ids {
            let candidate = detection
                .agents
                .iter()
                .find(|candidate| candidate.detector_id == detector_id)
                .ok_or_else(|| PersistenceError::NotFound {
                    entity: "detected_agent",
                    id: detector_id.to_string(),
                })?;
            if candidate.configured {
                continue;
            }
            let agent_root =
                candidate
                    .agent_root
                    .to_str()
                    .ok_or_else(|| LocalError::InvalidPathEncoding {
                        path: candidate.agent_root.clone(),
                    })?;
            records.push(StoredAgentConfig {
                id: detector_id.to_string(),
                detector_id: Some(detector_id.to_string()),
                display_name: candidate.display_name.clone(),
                agent_root: agent_root.to_owned(),
            });
            added_agent_ids.push(detector_id);
        }
        self.agent_configs.upsert_many(records)?;
        Ok(AddDetectedAgentsOutcome { added_agent_ids })
    }

    pub fn delete_agents(
        &mut self,
        raw_agent_ids: Vec<String>,
    ) -> Result<DeleteAgentsOutcome, ApplicationError> {
        if raw_agent_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "agentIds",
                reason: "must not be empty",
            });
        }
        let configurations = self.agent_configurations()?;
        let mut selected = Vec::new();
        for raw_id in raw_agent_ids {
            let agent_id = HarnessId::new(&raw_id).map_err(HarnessError::InvalidId)?;
            let configuration = configurations
                .iter()
                .find(|configuration| configuration.id == agent_id)
                .cloned()
                .ok_or_else(|| PersistenceError::NotFound {
                    entity: "agent_configuration",
                    id: raw_id,
                })?;
            if !selected
                .iter()
                .any(|selected: &AgentConfiguration| selected.id == agent_id)
            {
                selected.push(configuration);
            }
        }

        let agents_workspace_id = self
            .catalog
            .list_workspaces()?
            .into_iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .map(|stored| stored.workspace.id)
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: "agents".to_owned(),
            })?;
        let mut deleted_skill_count = 0;
        for configuration in &selected {
            deleted_skill_count += self.delete_agent_skills(&configuration.agent_root)?;
            self.catalog
                .delete_bindings_for_harness_workspace(&configuration.id, agents_workspace_id)?;
        }
        let stored_ids = selected
            .iter()
            .map(|configuration| configuration.id.to_string())
            .collect::<HashSet<_>>();
        self.agent_configs.remove(&stored_ids)?;
        Ok(DeleteAgentsOutcome {
            deleted_agent_ids: selected
                .into_iter()
                .map(|configuration| configuration.id)
                .collect(),
            deleted_skill_count,
        })
    }

    fn delete_agent_skills(&self, agent_root: &Path) -> Result<usize, ApplicationError> {
        let skills_root = agent_root.join(AGENT_SKILLS_DIRECTORY_NAME);
        self.delete_skills_root(&skills_root)
    }

    fn delete_skills_root(&self, skills_root: &Path) -> Result<usize, ApplicationError> {
        let skills_root = skills_root.to_path_buf();
        let entries = match fs::read_dir(&skills_root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(source) => {
                return Err(LocalError::Io {
                    path: skills_root,
                    source,
                }
                .into())
            }
        };
        let mut entry_paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| LocalError::Io {
                path: skills_root.clone(),
                source,
            })?;
            entry_paths.push(entry.path());
        }
        let deleted_skill_count = entry_paths.len();

        for path in entry_paths {
            Self::delete_skills_entry(&path)?;
        }
        Ok(deleted_skill_count)
    }

    fn delete_skills_entry(path: &Path) -> Result<(), ApplicationError> {
        // `remove_dir_all` removes directory links without following them. Regular files use the
        // file operation only when the directory operation reports that the entry is not a directory.
        match fs::remove_dir_all(path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) if source.kind() == io::ErrorKind::NotADirectory => {
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(LocalError::Io {
                            path: path.to_path_buf(),
                            source,
                        }
                        .into())
                    }
                }
            }
            Err(source) => {
                return Err(LocalError::Io {
                    path: path.to_path_buf(),
                    source,
                }
                .into())
            }
        }
        Ok(())
    }

    pub fn dashboard_overview(&self) -> Result<DashboardOverview, ApplicationError> {
        let skills = self.catalog.list_catalog_skills()?;
        let bindings = self.catalog.all_bindings()?;
        let workspaces = self.catalog.list_workspaces()?;
        let detection = self.detect_agents()?;
        let detected_harnesses = detection.agents.len();
        let diagnostics = detection
            .diagnostics
            .into_iter()
            .map(|diagnostic| ApplicationDiagnostic {
                code: "harness.probe_failed",
                message: format!("{}: {}", diagnostic.display_name, diagnostic.error),
            })
            .collect();

        let current_seconds = unix_timestamp()?;
        let current_week = week_start(current_seconds);
        let first_week = current_week - (DASHBOARD_WEEK_COUNT - 1) * SECONDS_PER_WEEK;
        let activity = self.catalog.activity_since(first_week)?;
        let mut buckets = BTreeMap::new();
        for offset in 0..DASHBOARD_WEEK_COUNT {
            buckets.insert(
                first_week + offset * SECONDS_PER_WEEK,
                DashboardActivityPeriod {
                    period_start_epoch_millis: (first_week + offset * SECONDS_PER_WEEK) * 1_000,
                    imported: 0,
                    updated: 0,
                },
            );
        }
        for event in activity {
            let bucket = week_start(event.occurred_at_epoch_seconds);
            let Some(period) = buckets.get_mut(&bucket) else {
                continue;
            };
            match event.kind {
                CatalogActivityKind::Imported => period.imported += 1,
                CatalogActivityKind::Updated => period.updated += 1,
            }
        }

        Ok(DashboardOverview {
            counts: DashboardCounts {
                skills: skills.len(),
                deployments: bindings.len(),
                detected_harnesses,
                workspaces: workspaces
                    .iter()
                    .filter(|stored| !matches!(stored.workspace.kind, WorkspaceKind::Agents))
                    .count(),
            },
            activity: buckets.into_values().collect(),
            diagnostics,
        })
    }

    pub fn list_catalog_skills_view(&self) -> Result<CatalogSkillList, ApplicationError> {
        let bindings = self.catalog.all_bindings()?;
        let mut deployment_counts = HashMap::<SkillId, usize>::new();
        for binding in bindings {
            *deployment_counts.entry(binding.key.skill_id).or_default() += 1;
        }
        let view = self.catalog.catalog_index_view()?;
        let skills = view
            .skills
            .into_iter()
            .map(|snapshot| CatalogSkillSummary {
                deployment_count: deployment_counts
                    .get(&snapshot.installed.id)
                    .copied()
                    .unwrap_or_default(),
                snapshot,
            })
            .collect();
        let diagnostics = view
            .diagnostics
            .into_iter()
            .map(|diagnostic| CatalogSkillIndexDiagnostic {
                skill_id: diagnostic.skill_id,
                path: diagnostic.path,
                kind: diagnostic.kind,
                message: diagnostic.message,
            })
            .collect();
        let freshness = match view.state {
            IndexState::Ready => CatalogIndexFreshness::Fresh,
            IndexState::Reconciling => CatalogIndexFreshness::Revalidating,
            IndexState::Uninitialized | IndexState::Stale => CatalogIndexFreshness::Stale,
        };
        Ok(CatalogSkillList {
            skills,
            diagnostics,
            freshness,
            revision: view.revision,
            last_reconciled_at_epoch_millis: view.last_reconciled_at_epoch_millis,
        })
    }

    pub fn rebuild_catalog_index(
        &mut self,
    ) -> Result<CatalogIndexRebuildOutcome, ApplicationError> {
        let cancellation = AtomicBool::new(false);
        let report = self.catalog.rebuild_catalog_index(&cancellation)?;
        let revision = self.catalog.catalog_index_view()?.revision;
        Ok(CatalogIndexRebuildOutcome {
            inserted: report.inserted.len(),
            updated: report.updated.len(),
            removed: report.removed.len(),
            unchanged: report.unchanged.len(),
            invalid: report.invalid.len(),
            revision,
        })
    }

    pub fn catalog_skill_detail(
        &self,
        raw_skill_id: &str,
    ) -> Result<CatalogSkillDetail, ApplicationError> {
        let skill_id = SkillId::parse(raw_skill_id).map_err(ApplicationError::InvalidSkillId)?;
        let bindings = self.catalog.all_bindings()?;
        let (snapshot, scanned) = self.catalog.catalog_skill(skill_id)?;
        Ok(CatalogSkillDetail {
            summary: CatalogSkillSummary {
                deployment_count: bindings
                    .iter()
                    .filter(|binding| binding.key.skill_id == skill_id)
                    .count(),
                snapshot,
            },
            body: scanned.document.body().to_owned(),
        })
    }

    pub fn scan_import_folder(
        &self,
        root: PathBuf,
    ) -> Result<ImportFolderPreview, ApplicationError> {
        validate_input_directory(&root, "root")?;
        let report = self.scan_import_root(&root)?;
        let candidates = report
            .skills
            .into_iter()
            .map(|scanned| {
                let metadata = scanned.document.metadata();
                ImportCandidate {
                    path: scanned.path,
                    name: metadata.name().to_owned(),
                    description: metadata.description().to_owned(),
                    version: metadata.version().map(ToOwned::to_owned),
                }
            })
            .collect();
        let diagnostics = report
            .diagnostics
            .into_iter()
            .map(|diagnostic| ImportFolderDiagnostic {
                path: diagnostic.path,
                error: diagnostic.error,
            })
            .collect();

        Ok(ImportFolderPreview {
            root,
            candidates,
            diagnostics,
        })
    }

    pub fn import_local_skills(
        &mut self,
        root: PathBuf,
        selected_paths: Vec<PathBuf>,
    ) -> Result<ImportSkillsOutcome, ApplicationError> {
        validate_input_directory(&root, "root")?;
        if selected_paths.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "paths",
                reason: "must not be empty",
            });
        }

        let report = self.scan_import_root(&root)?;
        let mut scanned_by_path = report
            .skills
            .into_iter()
            .map(|scanned| (scanned.path.clone(), scanned))
            .collect::<HashMap<_, _>>();
        let mut unique_paths = Vec::with_capacity(selected_paths.len());
        let mut seen_paths = HashSet::new();
        for path in selected_paths {
            if seen_paths.insert(path.clone()) {
                unique_paths.push(path);
            }
        }
        if unique_paths
            .iter()
            .any(|path| !scanned_by_path.contains_key(path))
        {
            return Err(ApplicationError::InvalidRequest {
                field: "paths",
                reason: "must contain only skills discovered under root",
            });
        }

        let mut occupied_directory_names = HashSet::new();
        let mut catalog_hashes = HashSet::new();
        for snapshot in self.catalog.list_catalog_skills()? {
            if let Some(directory_name) = snapshot.installed.location.file_name() {
                occupied_directory_names.insert(directory_name.to_owned());
            }
            catalog_hashes.insert(snapshot.version.content_hash);
        }
        let mut imported = Vec::new();
        let mut skipped = Vec::new();

        for path in unique_paths {
            let Some(scanned) = scanned_by_path.remove(&path) else {
                return Err(ApplicationError::InvalidRequest {
                    field: "paths",
                    reason: "must contain only skills discovered under root",
                });
            };
            let Some(directory_name) = scanned.path.file_name().map(ToOwned::to_owned) else {
                return Err(ApplicationError::InvalidRequest {
                    field: "paths",
                    reason: "skill path must have a directory name",
                });
            };

            if occupied_directory_names.contains(&directory_name)
                || catalog_hashes.contains(&scanned.content_hash)
            {
                skipped.push(path);
                continue;
            }

            match self.catalog.import_local(&scanned) {
                Ok(snapshot) => {
                    occupied_directory_names.insert(directory_name);
                    catalog_hashes.insert(snapshot.version.content_hash);
                    imported.push(snapshot.installed.id);
                }
                Err(CatalogFailure::Conflict { .. }) => skipped.push(path),
                Err(error) => return Err(error.into()),
            }
        }

        Ok(ImportSkillsOutcome { imported, skipped })
    }

    fn scan_import_root(&self, root: &Path) -> Result<ScanReport, LocalError> {
        match self.local.read(root) {
            Ok(scanned) => Ok(ScanReport {
                skills: vec![scanned],
                diagnostics: Vec::new(),
            }),
            Err(LocalError::MarkerNotFound { .. }) => self.local.scan(root, ScanMode::Recursive),
            Err(error) => Ok(ScanReport {
                skills: Vec::new(),
                diagnostics: vec![ScanDiagnostic {
                    path: root.to_path_buf(),
                    error,
                }],
            }),
        }
    }

    pub fn export_catalog_skills(
        &self,
        destination_root: PathBuf,
        raw_skill_ids: Vec<String>,
    ) -> Result<ExportSkillsOutcome, ApplicationError> {
        validate_input_directory(&destination_root, "destinationRoot")?;
        if raw_skill_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "skillIds",
                reason: "must not be empty",
            });
        }

        let mut skill_ids = Vec::with_capacity(raw_skill_ids.len());
        for raw_skill_id in raw_skill_ids {
            let skill_id =
                SkillId::parse(&raw_skill_id).map_err(ApplicationError::InvalidSkillId)?;
            if !skill_ids.contains(&skill_id) {
                skill_ids.push(skill_id);
            }
        }

        let mut sources = Vec::with_capacity(skill_ids.len());
        for skill_id in &skill_ids {
            let (snapshot, _) = self.catalog.catalog_skill(*skill_id)?;
            sources.push(snapshot.installed.location);
        }

        let export_directory_name = format!(
            "{EXPORT_DIRECTORY_PREFIX}-{}",
            Local::now().format("%Y%m%d%H%M")
        );
        let export_root = destination_root.join(&export_directory_name);
        let canonical_destination =
            fs::canonicalize(&destination_root).map_err(|source| LocalError::Io {
                path: destination_root.clone(),
                source,
            })?;
        let validation_root = canonical_destination.join(export_directory_name);
        reject_overlapping_paths(
            &validation_root,
            self.catalog.catalog_root(),
            "destinationRoot",
        )?;

        fs::create_dir(&export_root).map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                LocalError::DestinationExists {
                    path: export_root.clone(),
                }
            } else {
                LocalError::Io {
                    path: export_root.clone(),
                    source,
                }
            }
        })?;

        for source in sources {
            let directory_name = source.file_name().ok_or(PersistenceError::InvalidData {
                entity: "filesystem_catalog",
                field: "directory_name",
            })?;
            copy_skill(
                &source,
                &export_root.join(directory_name),
                ExistingDestination::Reject,
            )?;
        }

        Ok(ExportSkillsOutcome {
            export_root,
            exported: skill_ids,
        })
    }

    fn apply_agent_skills(
        &self,
        agent_root: PathBuf,
        raw_skill_ids: Vec<String>,
    ) -> Result<SetAgentSkillsOutcome, ApplicationError> {
        validate_input_directory(&agent_root, "agentRoot")?;
        reject_overlapping_paths(&agent_root, self.catalog.catalog_root(), "agentRoot")?;

        let mut selected_skill_ids = Vec::with_capacity(raw_skill_ids.len());
        for raw_skill_id in raw_skill_ids {
            let skill_id =
                SkillId::parse(&raw_skill_id).map_err(ApplicationError::InvalidSkillId)?;
            if !selected_skill_ids.contains(&skill_id) {
                selected_skill_ids.push(skill_id);
            }
        }
        let selected_skill_id_set = selected_skill_ids.iter().copied().collect::<HashSet<_>>();
        let catalog_skills = self.catalog.list_catalog_skills()?;
        for skill_id in &selected_skill_ids {
            if !catalog_skills
                .iter()
                .any(|snapshot| snapshot.installed.id == *skill_id)
            {
                return Err(PersistenceError::NotFound {
                    entity: "filesystem_catalog_skill",
                    id: skill_id.to_string(),
                }
                .into());
            }
        }

        let skills_root = agent_root.join(AGENT_SKILLS_DIRECTORY_NAME);
        match fs::metadata(&skills_root) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(ApplicationError::InvalidRequest {
                    field: "agentRoot",
                    reason: "skills path must be a directory",
                })
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&skills_root).map_err(|source| LocalError::Io {
                    path: skills_root.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(LocalError::Io {
                    path: skills_root.clone(),
                    source,
                }
                .into())
            }
        }
        reject_overlapping_paths(&skills_root, self.catalog.catalog_root(), "agentRoot")?;

        let mut linked = Vec::new();
        let mut removed = Vec::new();
        for snapshot in catalog_skills {
            let skill_id = snapshot.installed.id;
            let source = snapshot.installed.location;
            let directory_name = source.file_name().ok_or(PersistenceError::InvalidData {
                entity: "filesystem_catalog",
                field: "directory_name",
            })?;
            let target = skills_root.join(directory_name);

            if selected_skill_id_set.contains(&skill_id) {
                match link_target(&target) {
                    Ok(Some(existing_source))
                        if paths_refer_to_same_location(&existing_source, &source) => {}
                    Ok(Some(_)) => {
                        self.local.deploy(&source, &target, DeploymentMode::Link)?;
                    }
                    Ok(None) => return Err(LocalError::DestinationExists { path: target }.into()),
                    Err(LocalError::PathNotFound { .. }) => {
                        link_skill(&source, &target, ExistingDestination::Reject)?;
                    }
                    Err(error) => return Err(error.into()),
                }
                linked.push(skill_id);
                continue;
            }

            match link_target(&target) {
                Ok(Some(existing_source))
                    if paths_refer_to_same_location(&existing_source, &source) =>
                {
                    self.local.delete(&target)?;
                    removed.push(skill_id);
                }
                Ok(_) | Err(LocalError::PathNotFound { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }

        Ok(SetAgentSkillsOutcome {
            skills_root,
            linked,
            removed,
        })
    }

    pub fn save_agent(
        &mut self,
        input: SaveAgentInput,
    ) -> Result<SaveAgentOutcome, ApplicationError> {
        let display_name = normalize_workspace_name(&input.display_name)?;
        validate_input_directory(&input.agent_root, "agentRoot")?;
        let configurations = self.agent_configurations()?;
        let existing = match input.agent_id.as_deref() {
            Some(raw_id) => {
                let id = HarnessId::new(raw_id).map_err(HarnessError::InvalidId)?;
                Some(
                    configurations
                        .iter()
                        .find(|configuration| configuration.id == id)
                        .cloned()
                        .ok_or_else(|| PersistenceError::NotFound {
                            entity: "agent_configuration",
                            id: raw_id.to_owned(),
                        })?,
                )
            }
            None => configurations
                .iter()
                .find(|configuration| {
                    paths_refer_to_same_location(&configuration.agent_root, &input.agent_root)
                })
                .cloned(),
        };
        if configurations.iter().any(|configuration| {
            existing
                .as_ref()
                .is_none_or(|existing| configuration.id != existing.id)
                && paths_refer_to_same_location(&configuration.agent_root, &input.agent_root)
        }) {
            return Err(PersistenceError::Conflict {
                entity: "agent_root",
                id: input.agent_root.display().to_string(),
            }
            .into());
        }

        let agent_id = match &existing {
            Some(configuration) => configuration.id.clone(),
            None => HarnessId::new(&format!("agent-{}", uuid::Uuid::new_v4()))
                .map_err(HarnessError::InvalidId)?,
        };
        let detector_id = existing
            .as_ref()
            .and_then(|configuration| configuration.detector_id.clone());
        let skills = self.apply_agent_skills(input.agent_root.clone(), input.skill_ids)?;
        let agent_root = input
            .agent_root
            .to_str()
            .ok_or_else(|| LocalError::InvalidPathEncoding {
                path: input.agent_root.clone(),
            })?
            .to_owned();
        self.agent_configs.upsert(StoredAgentConfig {
            id: agent_id.to_string(),
            detector_id: detector_id.map(|id| id.to_string()),
            display_name: display_name.clone(),
            agent_root,
        })?;
        let agents_workspace_id = self
            .catalog
            .list_workspaces()?
            .into_iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .map(|stored| stored.workspace.id)
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: "agents".to_owned(),
            })?;
        self.catalog
            .delete_bindings_for_harness_workspace(&agent_id, agents_workspace_id)?;

        let mut skills = skills;
        if let Some(existing) = existing.filter(|existing| {
            !paths_refer_to_same_location(&existing.agent_root, &input.agent_root)
        }) {
            if existing
                .agent_root
                .join(AGENT_SKILLS_DIRECTORY_NAME)
                .is_dir()
            {
                let old = self.apply_agent_skills(existing.agent_root, Vec::new())?;
                skills.removed.extend(old.removed);
                sort_dedup_skill_ids(&mut skills.removed);
            }
        }

        Ok(SaveAgentOutcome {
            agent_id,
            display_name,
            agent_root: input.agent_root,
            skills,
        })
    }

    pub fn copy_project_agent_skills(
        &self,
        input: CopyProjectAgentSkillsInput,
    ) -> Result<CopyProjectAgentSkillsOutcome, ApplicationError> {
        let workspace_id = WorkspaceId::parse(&input.workspace_id)
            .map_err(ApplicationError::InvalidWorkspaceId)?;
        let workspace = self.catalog.workspace(workspace_id)?;
        let WorkspaceKind::Project { root: project_root } = workspace.workspace.kind else {
            return Err(ApplicationError::InvalidRequest {
                field: "workspaceId",
                reason: "must reference a Project workspace",
            });
        };
        validate_input_directory(&input.agent_root, "agentRoot")?;
        let canonical_project =
            fs::canonicalize(&project_root).map_err(|source| LocalError::Io {
                path: project_root.clone(),
                source,
            })?;
        let canonical_agent =
            fs::canonicalize(&input.agent_root).map_err(|source| LocalError::Io {
                path: input.agent_root.clone(),
                source,
            })?;
        if canonical_agent != canonical_project && !canonical_agent.starts_with(&canonical_project)
        {
            return Err(ApplicationError::InvalidRequest {
                field: "agentRoot",
                reason: "must be inside the selected Project",
            });
        }
        if input.skill_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "skillIds",
                reason: "must not be empty",
            });
        }
        let mut selected_ids = Vec::new();
        for raw_id in input.skill_ids {
            let skill_id = SkillId::parse(&raw_id).map_err(ApplicationError::InvalidSkillId)?;
            if !selected_ids.contains(&skill_id) {
                selected_ids.push(skill_id);
            }
        }
        let catalog_skills = self.catalog.list_catalog_skills()?;
        for skill_id in &selected_ids {
            if !catalog_skills
                .iter()
                .any(|snapshot| snapshot.installed.id == *skill_id)
            {
                return Err(PersistenceError::NotFound {
                    entity: "filesystem_catalog_skill",
                    id: skill_id.to_string(),
                }
                .into());
            }
        }

        let skills_root = input.agent_root.join(AGENT_SKILLS_DIRECTORY_NAME);
        match link_target(&skills_root) {
            Ok(Some(_)) => {
                return Err(ApplicationError::InvalidRequest {
                    field: "agentRoot",
                    reason: "Project skills path must be an ordinary directory",
                })
            }
            Ok(None) => {
                if !fs::metadata(&skills_root)
                    .map_err(|source| LocalError::Io {
                        path: skills_root.clone(),
                        source,
                    })?
                    .is_dir()
                {
                    return Err(ApplicationError::InvalidRequest {
                        field: "agentRoot",
                        reason: "Project skills path must be a directory",
                    });
                }
            }
            Err(LocalError::PathNotFound { .. }) => {
                fs::create_dir(&skills_root).map_err(|source| LocalError::Io {
                    path: skills_root.clone(),
                    source,
                })?;
            }
            Err(error) => return Err(error.into()),
        }

        for skill_id in &selected_ids {
            let snapshot = catalog_skills
                .iter()
                .find(|snapshot| snapshot.installed.id == *skill_id)
                .ok_or_else(|| PersistenceError::NotFound {
                    entity: "filesystem_catalog_skill",
                    id: skill_id.to_string(),
                })?;
            let source = &snapshot.installed.location;
            let directory_name = source.file_name().ok_or(PersistenceError::InvalidData {
                entity: "filesystem_catalog",
                field: "directory_name",
            })?;
            let target = skills_root.join(directory_name);
            match link_target(&target) {
                Ok(Some(_)) => {
                    copy_skill(source, &target, ExistingDestination::Replace)?;
                }
                Ok(None) => {
                    let existing = self.local.read(&target)?;
                    if existing.content_hash != snapshot.version.content_hash {
                        return Err(LocalError::DestinationExists { path: target }.into());
                    }
                }
                Err(LocalError::PathNotFound { .. }) => {
                    copy_skill(source, &target, ExistingDestination::Reject)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(CopyProjectAgentSkillsOutcome {
            skills_root,
            copied_skill_ids: selected_ids,
        })
    }

    pub fn delete_project_agents(
        &mut self,
        raw_workspace_id: &str,
        raw_agent_ids: Vec<String>,
    ) -> Result<DeleteProjectAgentsOutcome, ApplicationError> {
        if raw_agent_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "agentIds",
                reason: "must not be empty",
            });
        }
        let workspace_id =
            WorkspaceId::parse(raw_workspace_id).map_err(ApplicationError::InvalidWorkspaceId)?;
        let workspace = self.catalog.workspace(workspace_id)?;
        if !matches!(workspace.workspace.kind, WorkspaceKind::Project { .. }) {
            return Err(ApplicationError::InvalidRequest {
                field: "workspaceId",
                reason: "must reference a Project workspace",
            });
        }
        let project_agents = self.project_agents(&workspace)?;
        let mut selected = Vec::new();
        for raw_id in raw_agent_ids {
            let agent_id = HarnessId::new(&raw_id).map_err(HarnessError::InvalidId)?;
            let agent = project_agents
                .iter()
                .find(|agent| agent.id == agent_id)
                .ok_or_else(|| PersistenceError::NotFound {
                    entity: "project_agent",
                    id: raw_id,
                })?;
            if !selected
                .iter()
                .any(|selected: &&ProjectAgentOverview| selected.id == agent_id)
            {
                selected.push(agent);
            }
        }
        let mut deleted_skill_count = 0;
        for agent in &selected {
            deleted_skill_count += self.delete_skills_root(&agent.path)?;
            self.catalog
                .delete_bindings_for_harness_workspace(&agent.id, workspace_id)?;
        }
        Ok(DeleteProjectAgentsOutcome {
            deleted_agent_ids: selected.into_iter().map(|agent| agent.id.clone()).collect(),
            deleted_skill_count,
        })
    }

    pub fn delete_catalog_skills(
        &mut self,
        raw_skill_ids: Vec<String>,
    ) -> Result<Vec<SkillId>, ApplicationError> {
        if raw_skill_ids.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                field: "skillIds",
                reason: "must not be empty",
            });
        }
        let mut skill_ids = Vec::with_capacity(raw_skill_ids.len());
        for raw_skill_id in raw_skill_ids {
            let skill_id =
                SkillId::parse(&raw_skill_id).map_err(ApplicationError::InvalidSkillId)?;
            if !skill_ids.contains(&skill_id) {
                skill_ids.push(skill_id);
            }
        }

        let workspaces = self.catalog.list_workspaces()?;
        let agents = workspaces
            .iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: "agents".to_owned(),
            })?;
        let resolution = self.resolve_stored_workspace(agents)?;

        for skill_id in &skill_ids {
            let (snapshot, _) = self.catalog.catalog_skill(*skill_id)?;
            let directory_name =
                snapshot
                    .installed
                    .location
                    .file_name()
                    .ok_or(PersistenceError::InvalidData {
                        entity: "filesystem_catalog",
                        field: "directory_name",
                    })?;
            let mut candidate_paths = HashSet::new();
            for target in &resolution.targets {
                candidate_paths.insert(target.path.join(directory_name));
            }
            for discovery_root in &resolution.discovery_roots {
                candidate_paths.insert(discovery_root.path.join(directory_name));
            }

            let mut existing_paths = Vec::new();
            for path in candidate_paths {
                match self.local.read(&path) {
                    Ok(scanned) => existing_paths.push((scanned.link_target.is_some(), path)),
                    Err(LocalError::PathNotFound { .. }) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            existing_paths.sort_by(|left, right| {
                right
                    .0
                    .cmp(&left.0)
                    .then_with(|| left.1.as_os_str().cmp(right.1.as_os_str()))
            });
            for (_, path) in existing_paths {
                self.local.delete(&path)?;
            }

            self.catalog.delete_bindings_for_skill(*skill_id)?;
            let deleted_path = snapshot.installed.location;
            self.local.delete(&deleted_path)?;
            self.catalog
                .remove_catalog_skill_from_index(*skill_id, &deleted_path)?;
        }

        Ok(skill_ids)
    }

    pub fn workspaces_overview(&self) -> Result<WorkspacesOverview, ApplicationError> {
        let stored_workspaces = self.catalog.list_workspaces()?;
        let agents_workspace_id = stored_workspaces
            .iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .map(|stored| stored.workspace.id)
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: "agents".to_owned(),
            })?;
        let bindings = self.catalog.all_bindings()?;
        let catalog_link_targets = self
            .catalog
            .list_catalog_skills()?
            .into_iter()
            .map(|snapshot| {
                let location = fs::canonicalize(&snapshot.installed.location)
                    .unwrap_or(snapshot.installed.location);
                (location, snapshot.installed.id)
            })
            .collect::<HashMap<_, _>>();

        let mut harnesses = Vec::new();
        for configuration in self.agent_configurations()? {
            let global_skills_path = configuration.agent_root.join(AGENT_SKILLS_DIRECTORY_NAME);
            let adapter = self.agent_adapter(&configuration)?;
            let capabilities = adapter.capabilities();
            let probe = Ok(HarnessProbe {
                detection_status: configuration.detection_status,
                checked_paths: configuration.checked_paths,
                global_skills_path: global_skills_path.clone(),
            });
            let (skill_count, linked_skill_ids, scan_error) =
                match self.local.scan(&global_skills_path, ScanMode::Flat) {
                    Ok(report) => {
                        let mut linked_skill_ids = report
                            .skills
                            .iter()
                            .filter_map(|scanned| scanned.link_target.as_ref())
                            .filter_map(|target| {
                                let target =
                                    fs::canonicalize(target).unwrap_or_else(|_| target.clone());
                                catalog_link_targets.get(&target).copied()
                            })
                            .collect::<Vec<_>>();
                        linked_skill_ids.sort_by_key(ToString::to_string);
                        linked_skill_ids.dedup();
                        (
                            report.skills.len(),
                            linked_skill_ids,
                            report.diagnostics.into_iter().next().map(|item| item.error),
                        )
                    }
                    Err(error) => (0, Vec::new(), Some(error)),
                };
            harnesses.push(HarnessOverview {
                id: configuration.id,
                display_name: configuration.display_name,
                category: configuration.category,
                custom: configuration.custom,
                capabilities,
                skill_count,
                linked_skill_ids,
                probe,
                scan_error,
            });
        }
        harnesses.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });

        let workspaces = stored_workspaces
            .into_iter()
            .filter(|stored| !matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .map(|stored| WorkspaceSummary {
                deployment_count: bindings
                    .iter()
                    .filter(|binding| binding.key.workspace_id == stored.workspace.id)
                    .count(),
                stored,
            })
            .collect();

        Ok(WorkspacesOverview {
            agents_workspace_id,
            harnesses,
            workspaces,
        })
    }

    fn resolve_stored_workspace(
        &self,
        stored: &StoredWorkspace,
    ) -> Result<WorkspaceResolution, ApplicationError> {
        if matches!(stored.workspace.kind, WorkspaceKind::Agents) {
            let registry = self.agent_registry()?;
            return Ok(resolve_workspace(
                &stored.workspace,
                &registry,
                &self.environment,
                stored.deployment_mode,
            )?);
        }
        Ok(resolve_workspace(
            &stored.workspace,
            &self.harnesses,
            &self.environment,
            stored.deployment_mode,
        )?)
    }

    fn observe_stored_workspace(
        &mut self,
        stored: &StoredWorkspace,
    ) -> Result<WorkspaceReport, ApplicationError> {
        if matches!(stored.workspace.kind, WorkspaceKind::Agents) {
            let registry = self.agent_registry()?;
            return Ok(
                WorkspaceEngine::new(&self.local, &mut self.catalog).observe(
                    &stored.workspace,
                    &registry,
                    &self.environment,
                    stored.deployment_mode,
                )?,
            );
        }
        Ok(
            WorkspaceEngine::new(&self.local, &mut self.catalog).observe(
                &stored.workspace,
                &self.harnesses,
                &self.environment,
                stored.deployment_mode,
            )?,
        )
    }

    fn reconcile_stored_workspace(
        &mut self,
        stored: &StoredWorkspace,
    ) -> Result<ReconcileReport, ApplicationError> {
        if matches!(stored.workspace.kind, WorkspaceKind::Agents) {
            let registry = self.agent_registry()?;
            return Ok(
                WorkspaceEngine::new(&self.local, &mut self.catalog).reconcile(
                    &stored.workspace,
                    &registry,
                    &self.environment,
                    stored.deployment_mode,
                )?,
            );
        }
        Ok(
            WorkspaceEngine::new(&self.local, &mut self.catalog).reconcile(
                &stored.workspace,
                &self.harnesses,
                &self.environment,
                stored.deployment_mode,
            )?,
        )
    }

    fn project_agents(
        &self,
        stored: &StoredWorkspace,
    ) -> Result<Vec<ProjectAgentOverview>, ApplicationError> {
        let WorkspaceKind::Project { root } = &stored.workspace.kind else {
            return Ok(Vec::new());
        };
        let mut agents = Vec::new();
        for adapter in self.harnesses.adapters() {
            let Some(relative_path) = adapter.project_relative_skills_path() else {
                continue;
            };
            let path = root.join(relative_path);
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    agents.push(ProjectAgentOverview {
                        id: adapter.id().clone(),
                        display_name: adapter.display_name().to_owned(),
                        path: path.clone(),
                        skill_count: 0,
                        error: Some(LocalError::Io { path, source }),
                    });
                    continue;
                }
            };
            if !metadata.is_dir() {
                agents.push(ProjectAgentOverview {
                    id: adapter.id().clone(),
                    display_name: adapter.display_name().to_owned(),
                    path: path.clone(),
                    skill_count: 0,
                    error: Some(LocalError::NotDirectory { path }),
                });
                continue;
            }
            match self.local.scan(&path, ScanMode::Recursive) {
                Ok(report) => agents.push(ProjectAgentOverview {
                    id: adapter.id().clone(),
                    display_name: adapter.display_name().to_owned(),
                    path,
                    skill_count: report.skills.len(),
                    error: report.diagnostics.into_iter().next().map(|item| item.error),
                }),
                Err(error) => agents.push(ProjectAgentOverview {
                    id: adapter.id().clone(),
                    display_name: adapter.display_name().to_owned(),
                    path,
                    skill_count: 0,
                    error: Some(error),
                }),
            }
        }
        agents.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(agents)
    }

    pub fn create_workspace(
        &mut self,
        input: CreateWorkspaceInput,
    ) -> Result<WorkspaceSummary, ApplicationError> {
        let name = normalize_workspace_name(&input.name)?;
        let kind = match input.kind {
            CreateWorkspaceKind::Project { root } => {
                validate_input_directory(&root, "root")?;
                WorkspaceKind::Project { root }
            }
            CreateWorkspaceKind::Linked {
                root,
                disabled_root,
            } => {
                validate_input_directory(&root, "root")?;
                if let Some(disabled_root) = &disabled_root {
                    validate_input_directory(disabled_root, "disabledRoot")?;
                }
                WorkspaceKind::Linked {
                    root,
                    disabled_root,
                }
            }
        };
        let stored = StoredWorkspace {
            name,
            workspace: skill_workspace::Workspace {
                id: WorkspaceId::new(),
                kind,
            },
            deployment_mode: input.deployment_mode,
        };
        self.validate_workspace_against_catalog_root(&stored)?;
        resolve_workspace(
            &stored.workspace,
            &self.harnesses,
            &self.environment,
            stored.deployment_mode,
        )?;

        let existing = self.catalog.list_workspaces()?;
        if existing
            .iter()
            .any(|candidate| same_workspace_location(candidate, &stored))
        {
            return Err(PersistenceError::Conflict {
                entity: "workspace_root",
                id: workspace_root(&stored.workspace.kind)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| stored.workspace.id.to_string()),
            }
            .into());
        }
        self.catalog.insert_workspace(&stored)?;
        Ok(WorkspaceSummary {
            stored,
            deployment_count: 0,
        })
    }

    pub fn observe_workspace(
        &mut self,
        raw_workspace_id: &str,
    ) -> Result<WorkspaceObservation, ApplicationError> {
        let workspace_id =
            WorkspaceId::parse(raw_workspace_id).map_err(ApplicationError::InvalidWorkspaceId)?;
        let stored = self.catalog.workspace(workspace_id)?;
        let resolution = self.resolve_stored_workspace(&stored)?;
        let project_agents = self.project_agents(&stored)?;
        let report = self.observe_stored_workspace(&stored)?;
        let deployment_count = self
            .catalog
            .all_bindings()?
            .iter()
            .filter(|binding| binding.key.workspace_id == workspace_id)
            .count();
        Ok(WorkspaceObservation {
            workspace: WorkspaceSummary {
                stored,
                deployment_count,
            },
            resolution,
            report,
            project_agents,
        })
    }

    pub fn reconcile_workspace(
        &mut self,
        raw_workspace_id: &str,
    ) -> Result<WorkspaceReconcileOutcome, ApplicationError> {
        let workspace_id =
            WorkspaceId::parse(raw_workspace_id).map_err(ApplicationError::InvalidWorkspaceId)?;
        let workspaces = self.catalog.list_workspaces()?;
        let requested_workspace = workspaces
            .iter()
            .find(|stored| stored.workspace.id == workspace_id)
            .cloned()
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: workspace_id.to_string(),
            })?;
        let initial_report = self.observe_stored_workspace(&requested_workspace)?;
        let has_local_newer = initial_report
            .observations
            .iter()
            .any(|observation| observation.status == DeploymentStatus::LocalNewer);
        let has_center_newer = initial_report
            .observations
            .iter()
            .any(|observation| observation.status == DeploymentStatus::CenterNewer);
        let has_missing = initial_report
            .observations
            .iter()
            .any(|observation| observation.status == DeploymentStatus::Missing);
        let requires_global_plan = has_local_newer
            || has_center_newer
            || has_missing
            || !initial_report.unmatched_local.is_empty();
        let update_plan = if requires_global_plan {
            self.plan_center_updates(&workspaces)?
        } else {
            Vec::new()
        };
        let applies_plan_before_requested = has_local_newer || has_center_newer || has_missing;
        let mut globally_updated = if applies_plan_before_requested {
            self.apply_center_update_plan(&update_plan)?
        } else {
            Vec::new()
        };

        let mut requested = self.reconcile_stored_workspace(&requested_workspace)?;

        if !applies_plan_before_requested
            && (!requested.center_updated.is_empty() || !requested.imported.is_empty())
        {
            let updated_after_requested = self.apply_center_update_plan(&update_plan)?;
            if !updated_after_requested.is_empty() {
                let final_pass = self.reconcile_stored_workspace(&requested_workspace)?;
                merge_reconcile_reports(&mut requested, final_pass);
                globally_updated.extend(updated_after_requested);
            }
        }

        requested.center_updated.extend(globally_updated);
        sort_dedup_skill_ids(&mut requested.center_updated);

        let mut propagated = Vec::new();
        if !requested.center_updated.is_empty()
            || !requested.imported.is_empty()
            || !requested.propagated.is_empty()
        {
            for workspace in workspaces {
                if workspace.workspace.id == workspace_id {
                    continue;
                }
                let result = self.reconcile_stored_workspace(&workspace);
                propagated.push(PropagationOutcome { workspace, result });
            }
        }

        Ok(WorkspaceReconcileOutcome {
            requested_workspace,
            requested,
            propagated,
        })
    }

    fn plan_center_updates(
        &mut self,
        workspaces: &[StoredWorkspace],
    ) -> Result<Vec<CenterUpdatePlan>, ApplicationError> {
        let mut candidates = HashMap::<SkillId, (SkillVersion, Vec<LocalCandidate>)>::new();
        for stored in workspaces {
            let report = self.observe_stored_workspace(stored)?;
            for observation in report.observations {
                let (Some(center), Some(local)) = (observation.center, observation.local) else {
                    continue;
                };
                candidates
                    .entry(center.installed.id)
                    .or_insert_with(|| (center.version, Vec::new()))
                    .1
                    .push(LocalCandidate {
                        path: observation.target_path,
                        version: local,
                    });
            }
        }

        let mut skill_ids: Vec<SkillId> = candidates.keys().copied().collect();
        skill_ids.sort_by_key(ToString::to_string);
        let mut plan = Vec::new();
        for skill_id in skill_ids {
            let Some((center, local_candidates)) = candidates.get(&skill_id) else {
                continue;
            };
            let Some(selected) = choose_newest_local(local_candidates, center) else {
                continue;
            };
            let Some(candidate) = local_candidates.get(selected) else {
                continue;
            };
            plan.push(CenterUpdatePlan {
                skill_id,
                candidate: candidate.clone(),
            });
        }
        Ok(plan)
    }

    fn apply_center_update_plan(
        &mut self,
        plan: &[CenterUpdatePlan],
    ) -> Result<Vec<SkillId>, ApplicationError> {
        let mut updated = Vec::new();
        for item in plan {
            let scanned = self
                .local
                .read(&item.candidate.path)
                .map_err(WorkspaceError::Local)?;
            if scanned.content_hash != item.candidate.version.content_hash
                || scanned.marker_modified_at != item.candidate.version.marker_modified_at
            {
                return Err(ApplicationError::WorkspaceChangedDuringReconcile);
            }
            let (current, _) = self.catalog.catalog_skill(item.skill_id)?;
            if current.version.content_hash == scanned.content_hash {
                continue;
            }
            self.catalog
                .update_from_local(&item.skill_id, &scanned)
                .map_err(|source| WorkspaceError::Catalog {
                    operation: "update_from_local",
                    source,
                })?;
            updated.push(item.skill_id);
        }
        Ok(updated)
    }

    pub fn app_settings(&self) -> AppSettings {
        AppSettings {
            catalog_root: self.catalog.catalog_root().to_path_buf(),
        }
    }

    pub fn update_catalog_root(
        &mut self,
        catalog_root: PathBuf,
    ) -> Result<AppSettings, ApplicationError> {
        validate_input_directory(&catalog_root, "catalogRoot")?;
        self.validate_catalog_root_against_workspaces(&catalog_root)?;
        self.catalog.set_catalog_root(catalog_root)?;
        Ok(self.app_settings())
    }

    fn validate_workspace_against_catalog_root(
        &self,
        stored: &StoredWorkspace,
    ) -> Result<(), ApplicationError> {
        if let Some(root) = workspace_root(&stored.workspace.kind) {
            reject_overlapping_paths(root, self.catalog.catalog_root(), "root")?;
        }
        if let WorkspaceKind::Linked {
            disabled_root: Some(disabled_root),
            ..
        } = &stored.workspace.kind
        {
            reject_overlapping_paths(disabled_root, self.catalog.catalog_root(), "disabledRoot")?;
        }
        Ok(())
    }

    fn validate_catalog_root_against_workspaces(
        &self,
        catalog_root: &Path,
    ) -> Result<(), ApplicationError> {
        for workspace in self.catalog.list_workspaces()? {
            if let Some(root) = workspace_root(&workspace.workspace.kind) {
                reject_overlapping_paths(root, catalog_root, "catalogRoot")?;
            }
            if let WorkspaceKind::Linked {
                disabled_root: Some(disabled_root),
                ..
            } = &workspace.workspace.kind
            {
                reject_overlapping_paths(disabled_root, catalog_root, "catalogRoot")?;
            }
        }

        for adapter in self.harnesses.adapters() {
            let locations = adapter.resolve_locations(&self.environment, None)?;
            reject_overlapping_paths(&locations.global_skills_dir, catalog_root, "catalogRoot")?;
            for discovery in locations.additional_global_discovery_dirs {
                reject_overlapping_paths(&discovery, catalog_root, "catalogRoot")?;
            }
        }
        for configuration in self.agent_configurations()? {
            reject_overlapping_paths(
                &configuration.agent_root.join(AGENT_SKILLS_DIRECTORY_NAME),
                catalog_root,
                "catalogRoot",
            )?;
        }
        Ok(())
    }
}

fn merge_reconcile_reports(current: &mut ReconcileReport, mut next: ReconcileReport) {
    current.imported.append(&mut next.imported);
    sort_dedup_skill_ids(&mut current.imported);
    current.center_updated.append(&mut next.center_updated);
    sort_dedup_skill_ids(&mut current.center_updated);
    current.propagated.append(&mut next.propagated);
    current.propagated.sort_by(|left, right| {
        left.skill_id
            .to_string()
            .cmp(&right.skill_id.to_string())
            .then_with(|| left.harness_id.as_str().cmp(right.harness_id.as_str()))
            .then_with(|| {
                left.workspace_id
                    .to_string()
                    .cmp(&right.workspace_id.to_string())
            })
    });
    current.propagated.dedup();

    let previous_diagnostics = std::mem::take(&mut current.final_report.diagnostics);
    for diagnostic in previous_diagnostics {
        if !next.final_report.diagnostics.iter().any(|existing| {
            existing.path == diagnostic.path
                && existing.status == diagnostic.status
                && existing.error.to_string() == diagnostic.error.to_string()
        }) {
            next.final_report.diagnostics.push(diagnostic);
        }
    }
    next.final_report.diagnostics.sort_by(|left, right| {
        left.path
            .as_os_str()
            .cmp(right.path.as_os_str())
            .then_with(|| left.error.to_string().cmp(&right.error.to_string()))
    });
    current.final_report = next.final_report;
}

fn sort_dedup_skill_ids(skill_ids: &mut Vec<SkillId>) {
    skill_ids.sort_by_key(ToString::to_string);
    skill_ids.dedup();
}

fn discover_harness(
    adapter: &HarnessAdapter,
    environment: &HarnessEnvironment,
) -> Result<Option<HarnessProbe>, HarnessError> {
    let detection = adapter.detect(environment)?;
    if !detection.is_installed() {
        return Ok(None);
    }
    let Some(global_skills_path) = adapter.existing_global_skills_dir(environment)? else {
        return Ok(None);
    };

    Ok(Some(HarnessProbe {
        detection_status: detection.status,
        checked_paths: detection.checked_paths,
        global_skills_path,
    }))
}

fn normalize_workspace_name(value: &str) -> Result<String, ApplicationError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApplicationError::InvalidRequest {
            field: "name",
            reason: "must not be empty",
        });
    }
    if value.chars().count() > MAX_WORKSPACE_NAME_CHARS {
        return Err(ApplicationError::InvalidRequest {
            field: "name",
            reason: "is too long",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ApplicationError::InvalidRequest {
            field: "name",
            reason: "must not contain control characters",
        });
    }
    Ok(value.to_owned())
}

fn validate_input_directory(path: &Path, field: &'static str) -> Result<(), ApplicationError> {
    if !path.is_absolute() {
        return Err(ApplicationError::InvalidRequest {
            field,
            reason: "must be an absolute path",
        });
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApplicationError::InvalidRequest {
            field,
            reason: "must not contain parent directory segments",
        });
    }
    let metadata = fs::metadata(path).map_err(|_| ApplicationError::InvalidRequest {
        field,
        reason: "must reference an accessible directory",
    })?;
    if !metadata.is_dir() {
        return Err(ApplicationError::InvalidRequest {
            field,
            reason: "must reference a directory",
        });
    }
    Ok(())
}

fn reject_overlapping_paths(
    left: &Path,
    right: &Path,
    field: &'static str,
) -> Result<(), ApplicationError> {
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    if left == right || left.starts_with(&right) || right.starts_with(&left) {
        return Err(ApplicationError::InvalidRequest {
            field,
            reason: "must not overlap the central catalog or a workspace root",
        });
    }
    Ok(())
}

fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn same_workspace_location(left: &StoredWorkspace, right: &StoredWorkspace) -> bool {
    match (
        workspace_root(&left.workspace.kind),
        workspace_root(&right.workspace.kind),
    ) {
        (Some(left), Some(right)) => {
            let left = fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
            let right = fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
            left == right
        }
        _ => false,
    }
}

fn workspace_root(kind: &WorkspaceKind) -> Option<&Path> {
    match kind {
        WorkspaceKind::Agents => None,
        WorkspaceKind::Project { root } | WorkspaceKind::Linked { root, .. } => Some(root),
    }
}

fn unix_timestamp() -> Result<i64, ApplicationError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PersistenceError::Clock)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| ApplicationError::Persistence(PersistenceError::Clock))
}

fn week_start(epoch_seconds: i64) -> i64 {
    let days = epoch_seconds.div_euclid(SECONDS_PER_DAY);
    let monday_day = (days + 3).div_euclid(7) * 7 - 3;
    monday_day * SECONDS_PER_DAY
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use skill_workspace::{DeploymentBinding, DeploymentKey};
    use tempfile::tempdir;

    use super::*;

    fn write_test_skill(path: &Path, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n{name}\n"),
        )
        .unwrap();
    }

    fn test_application(root: &Path) -> Application {
        let app_data = root.join("app-data");
        let home = root.join("home");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&home).unwrap();
        Application {
            catalog: PersistentCatalog::open(
                app_data.join("state.sqlite3"),
                app_data.join("catalog"),
            )
            .unwrap(),
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::new(home, None),
            local: SystemLocalSkillPort::for_current_platform(),
            agent_configs: AgentConfigStore::open(app_data.join("agents.json")).unwrap(),
        }
    }

    fn create_linked_workspace(
        application: &mut Application,
        name: &str,
        root: PathBuf,
    ) -> WorkspaceSummary {
        application
            .create_workspace(CreateWorkspaceInput {
                name: name.to_owned(),
                kind: CreateWorkspaceKind::Linked {
                    root,
                    disabled_root: None,
                },
                deployment_mode: DeploymentMode::Copy,
            })
            .unwrap()
    }

    fn associate_test_skill(
        application: &mut Application,
        workspace: &StoredWorkspace,
        target_path: PathBuf,
        skill_id: SkillId,
    ) {
        let resolution = resolve_workspace(
            &workspace.workspace,
            &application.harnesses,
            &application.environment,
            workspace.deployment_mode,
        )
        .unwrap();
        application
            .catalog
            .associate(DeploymentBinding {
                key: DeploymentKey {
                    skill_id,
                    harness_id: resolution.targets[0].harness_id.clone(),
                    workspace_id: workspace.workspace.id,
                },
                target_path,
                deployment_mode: workspace.deployment_mode,
            })
            .unwrap();
    }

    #[test]
    fn dashboard_week_start_uses_monday() {
        assert_eq!(week_start(0), -3 * SECONDS_PER_DAY);
        assert_eq!(week_start(4 * SECONDS_PER_DAY), 4 * SECONDS_PER_DAY);
        assert_eq!(week_start(10 * SECONDS_PER_DAY), 4 * SECONDS_PER_DAY);
    }

    #[test]
    fn detected_agents_enter_workspaces_only_after_they_are_added() {
        let root = tempdir().unwrap();
        let mut application = test_application(root.path());
        let home = application.environment.home_dir().to_path_buf();
        fs::create_dir_all(home.join(".codex/skills")).unwrap();
        fs::create_dir_all(home.join(".agents/skills")).unwrap();
        fs::create_dir_all(home.join(".omp/agent/skills")).unwrap();
        fs::create_dir_all(home.join(".config/opencode/skills")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".cursor")).unwrap();
        fs::write(home.join(".cursor/skills"), "not a directory").unwrap();

        assert!(application
            .workspaces_overview()
            .unwrap()
            .harnesses
            .is_empty());
        let detector_ids = application
            .detect_agents()
            .unwrap()
            .agents
            .into_iter()
            .map(|agent| agent.detector_id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            detector_ids,
            vec!["agents", "codex", "omp_agent", "opencode"]
        );
        application.add_detected_agents(detector_ids).unwrap();

        let overview = application.workspaces_overview().unwrap();
        let harness_ids = overview
            .harnesses
            .iter()
            .map(|harness| harness.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            harness_ids,
            vec!["agents", "codex", "omp_agent", "opencode"]
        );
        assert_eq!(
            application
                .dashboard_overview()
                .unwrap()
                .counts
                .detected_harnesses,
            4
        );
        assert_eq!(
            overview.harnesses[0]
                .probe
                .as_ref()
                .unwrap()
                .global_skills_path,
            home.join(".agents/skills")
        );
    }

    #[test]
    fn dot_agents_skills_reconcile_through_the_agents_agent() {
        let root = tempdir().unwrap();
        let mut application = test_application(root.path());
        let home = application.environment.home_dir().to_path_buf();
        let skill_path = home.join(".agents/skills/shared-skill");
        write_test_skill(&skill_path, "shared-skill");

        let detected = application.detect_agents().unwrap();
        let agents = detected
            .agents
            .iter()
            .find(|agent| agent.detector_id.as_str() == "agents")
            .unwrap();
        assert_eq!(agents.display_name, "Agents");
        assert_eq!(agents.agent_root, home.join(".agents"));
        assert_eq!(agents.skill_count, 1);

        application
            .add_detected_agents(vec!["agents".to_owned()])
            .unwrap();
        let agents_workspace_id = application
            .catalog
            .list_workspaces()
            .unwrap()
            .into_iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
            .unwrap()
            .workspace
            .id;

        let outcome = application
            .reconcile_workspace(&agents_workspace_id.to_string())
            .unwrap();

        assert_eq!(outcome.requested.imported.len(), 1);
        let bindings = application.catalog.bindings(agents_workspace_id).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].key.harness_id.as_str(), "agents");
        assert_eq!(bindings[0].target_path, skill_path);
        assert_eq!(bindings[0].deployment_mode, DeploymentMode::Link);
    }

    #[test]
    fn delete_agents_clears_the_skills_root_without_following_links() {
        let root = tempdir().unwrap();
        let mut application = test_application(root.path());
        let home = application.environment.home_dir().to_path_buf();
        let skills_root = home.join(".agents/skills");
        let external_skill = home.join("external/linked-skill");
        write_test_skill(&external_skill, "linked-skill");
        write_test_skill(&skills_root.join("ordinary-skill"), "ordinary-skill");
        fs::write(skills_root.join("loose-file.txt"), "remove this too").unwrap();
        link_skill(
            &external_skill,
            &skills_root.join("linked-skill"),
            ExistingDestination::Reject,
        )
        .unwrap();
        application
            .add_detected_agents(vec!["agents".to_owned()])
            .unwrap();

        #[cfg(windows)]
        let skills_directory_handle = {
            use std::os::windows::fs::OpenOptionsExt;

            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(&skills_root)
                .unwrap()
        };

        let outcome = application
            .delete_agents(vec!["agents".to_owned()])
            .unwrap();

        #[cfg(windows)]
        drop(skills_directory_handle);
        assert_eq!(outcome.deleted_skill_count, 3);
        assert!(skills_root.is_dir());
        assert_eq!(fs::read_dir(&skills_root).unwrap().count(), 0);
        assert!(external_skill.exists());
        assert!(application
            .workspaces_overview()
            .unwrap()
            .harnesses
            .is_empty());
    }

    #[test]
    fn reconcile_selects_newest_candidate_across_workspaces() {
        let root = tempdir().unwrap();
        let app_data = root.path().join("app-data");
        let home = root.path().join("home");
        let first_root = root.path().join("first");
        let second_root = root.path().join("second");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&home).unwrap();

        let catalog =
            PersistentCatalog::open(app_data.join("state.sqlite3"), app_data.join("catalog"))
                .unwrap();
        let mut application = Application {
            catalog,
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::new(home, None),
            local: SystemLocalSkillPort::for_current_platform(),
            agent_configs: AgentConfigStore::open(app_data.join("agents.json")).unwrap(),
        };

        let original = root.path().join("original");
        write_test_skill(&original, "original");
        let imported = application
            .catalog
            .import_local(&application.local.read(&original).unwrap())
            .unwrap();
        let skill_id = imported.installed.id;

        std::thread::sleep(Duration::from_millis(30));
        let first_skill = first_root.join("shared");
        write_test_skill(&first_skill, "older-local");
        std::thread::sleep(Duration::from_millis(30));
        let second_skill = second_root.join("shared");
        write_test_skill(&second_skill, "newest-local");

        let first = application
            .create_workspace(CreateWorkspaceInput {
                name: "First".to_owned(),
                kind: CreateWorkspaceKind::Linked {
                    root: first_root,
                    disabled_root: None,
                },
                deployment_mode: DeploymentMode::Copy,
            })
            .unwrap();
        let second = application
            .create_workspace(CreateWorkspaceInput {
                name: "Second".to_owned(),
                kind: CreateWorkspaceKind::Linked {
                    root: second_root,
                    disabled_root: None,
                },
                deployment_mode: DeploymentMode::Copy,
            })
            .unwrap();

        for (workspace, target_path) in [
            (&first.stored, &first_skill),
            (&second.stored, &second_skill),
        ] {
            let resolution = resolve_workspace(
                &workspace.workspace,
                &application.harnesses,
                &application.environment,
                workspace.deployment_mode,
            )
            .unwrap();
            let target = &resolution.targets[0];
            application
                .catalog
                .associate(DeploymentBinding {
                    key: DeploymentKey {
                        skill_id,
                        harness_id: target.harness_id.clone(),
                        workspace_id: workspace.workspace.id,
                    },
                    target_path: target_path.clone(),
                    deployment_mode: workspace.deployment_mode,
                })
                .unwrap();
        }

        let outcome = application
            .reconcile_workspace(&first.stored.workspace.id.to_string())
            .unwrap();

        assert_eq!(outcome.requested.center_updated, vec![skill_id]);
        let (_, center) = application.catalog.catalog_skill(skill_id).unwrap();
        assert_eq!(center.document.metadata().name(), "newest-local");
        assert_eq!(
            application
                .local
                .read(&first_skill)
                .unwrap()
                .document
                .metadata()
                .name(),
            "newest-local"
        );
        assert_eq!(
            application
                .local
                .read(&second_skill)
                .unwrap()
                .document
                .metadata()
                .name(),
            "newest-local"
        );
    }

    #[test]
    fn missing_requested_target_still_uses_global_newest_candidate() {
        let root = tempdir().unwrap();
        let mut application = test_application(root.path());
        let original = root.path().join("original");
        write_test_skill(&original, "original");
        let imported = application
            .catalog
            .import_local(&application.local.read(&original).unwrap())
            .unwrap();
        let skill_id = imported.installed.id;

        let missing_root = root.path().join("a-missing");
        let older_root = root.path().join("b-older");
        let newest_root = root.path().join("c-newest");
        fs::create_dir_all(&missing_root).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let older_skill = older_root.join("shared");
        write_test_skill(&older_skill, "older-local");
        std::thread::sleep(Duration::from_millis(30));
        let newest_skill = newest_root.join("shared");
        write_test_skill(&newest_skill, "newest-local");
        let missing_skill = missing_root.join("shared");

        let missing = create_linked_workspace(&mut application, "A missing", missing_root);
        let older = create_linked_workspace(&mut application, "B older", older_root);
        let newest = create_linked_workspace(&mut application, "C newest", newest_root);
        associate_test_skill(
            &mut application,
            &missing.stored,
            missing_skill.clone(),
            skill_id,
        );
        associate_test_skill(
            &mut application,
            &older.stored,
            older_skill.clone(),
            skill_id,
        );
        associate_test_skill(
            &mut application,
            &newest.stored,
            newest_skill.clone(),
            skill_id,
        );

        let outcome = application
            .reconcile_workspace(&missing.stored.workspace.id.to_string())
            .unwrap();

        assert_eq!(outcome.requested.center_updated, vec![skill_id]);
        let (_, center) = application.catalog.catalog_skill(skill_id).unwrap();
        assert_eq!(center.document.metadata().name(), "newest-local");
        for path in [missing_skill, older_skill, newest_skill] {
            assert_eq!(
                application
                    .local
                    .read(&path)
                    .unwrap()
                    .document
                    .metadata()
                    .name(),
                "newest-local"
            );
        }
    }

    #[test]
    fn project_workspace_is_persisted_and_observable() {
        let root = tempdir().unwrap();
        let app_data = root.path().join("app-data");
        let project = root.path().join("project");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&project).unwrap();
        let mut application =
            Application::open(app_data.join("state.sqlite3"), app_data.join("catalog")).unwrap();

        let created = application
            .create_workspace(CreateWorkspaceInput {
                name: "Project".to_owned(),
                kind: CreateWorkspaceKind::Project {
                    root: project.clone(),
                },
                deployment_mode: DeploymentMode::Copy,
            })
            .unwrap();
        assert_eq!(created.stored.name, "Project");
        assert_eq!(
            workspace_root(&created.stored.workspace.kind),
            Some(project.as_path())
        );
        let workspace_id = created.stored.workspace.id;
        let observed = application
            .observe_workspace(&workspace_id.to_string())
            .unwrap();
        assert_eq!(observed.report.workspace_id, workspace_id);
        drop(application);

        let application =
            Application::open(app_data.join("state.sqlite3"), app_data.join("catalog")).unwrap();
        assert!(application
            .workspaces_overview()
            .unwrap()
            .workspaces
            .iter()
            .any(|workspace| workspace.stored.workspace.id == workspace_id));
    }
}
