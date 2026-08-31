use std::{path::Path, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use skill_core::{InstalledSkill, SkillMarker, SkillSource};
use skill_harness::{DetectionStatus, HarnessCapabilities, HarnessCategory};
use skill_index::SkillLockEntry;
use skill_local::ScanMode;
use skill_registry::{Leaderboard, LeaderboardResult, RemoteSkillSummary, SearchResult};
use skill_workspace::{
    CentralSkillSnapshot, DeploymentKey, DeploymentMode, DeploymentObservation, DeploymentStatus,
    DiscoveryRoot, ReconcileReport, TargetRole, UnmatchedLocalSkill, UnsupportedWorkspaceTarget,
    WorkspaceDiagnostic, WorkspaceKind, WorkspaceReport, WorkspaceResolution, WorkspaceTarget,
};

use crate::{
    application::{
        AddDetectedAgentsOutcome, AgentDetectionDiagnostic, AgentDetectionOutcome, AppSettings,
        CatalogIndexFreshness, CatalogIndexRebuildOutcome, CatalogSkillDetail, CatalogSkillList,
        CatalogSkillSummary, CopyProjectAgentSkillsInput, CopyProjectAgentSkillsOutcome,
        CreateWorkspaceInput, CreateWorkspaceKind, DashboardOverview, DeleteAgentsOutcome,
        DeleteProjectAgentsOutcome, DetectedAgent, ExportSkillsOutcome, HarnessOverview,
        HarnessProbe, ImportCandidate, ImportFolderDiagnostic, ImportFolderPreview,
        ImportSkillsOutcome, ProjectAgentOverview, PropagationOutcome, SaveAgentInput,
        SaveAgentOutcome, SkillSetSummary, WorkspaceObservation, WorkspaceReconcileOutcome,
        WorkspaceSummary, WorkspacesOverview,
    },
    ipc::IpcError,
    persistence::StoredWorkspace,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDto {
    pub value: Option<String>,
    pub display: String,
}

impl From<&Path> for PathDto {
    fn from(path: &Path) -> Self {
        Self {
            value: path.to_str().map(ToOwned::to_owned),
            display: path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardOverviewDto {
    pub counts: DashboardCountsDto,
    pub activity: Vec<DashboardActivityDto>,
    pub diagnostics: Vec<SimpleDiagnosticDto>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardCountsDto {
    pub skills: usize,
    pub deployments: usize,
    pub detected_harnesses: usize,
    pub workspaces: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardActivityDto {
    pub period_start_epoch_millis: i64,
    pub imported: usize,
    pub updated: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleDiagnosticDto {
    pub code: String,
    pub message: String,
}

impl From<DashboardOverview> for DashboardOverviewDto {
    fn from(value: DashboardOverview) -> Self {
        Self {
            counts: DashboardCountsDto {
                skills: value.counts.skills,
                deployments: value.counts.deployments,
                detected_harnesses: value.counts.detected_harnesses,
                workspaces: value.counts.workspaces,
            },
            activity: value
                .activity
                .into_iter()
                .map(|period| DashboardActivityDto {
                    period_start_epoch_millis: period.period_start_epoch_millis,
                    imported: period.imported,
                    updated: period.updated,
                })
                .collect(),
            diagnostics: value
                .diagnostics
                .into_iter()
                .map(|diagnostic| SimpleDiagnosticDto {
                    code: diagnostic.code.to_owned(),
                    message: diagnostic.message,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkillsResponseDto {
    pub skills: Vec<CatalogSkillSummaryDto>,
    pub sets: Vec<SkillSetDto>,
    pub diagnostics: Vec<CatalogSkillIndexDiagnosticDto>,
    pub index: CatalogIndexStatusDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSetDto {
    pub id: String,
    pub name: String,
    pub skill_ids: Vec<String>,
}

impl From<SkillSetSummary> for SkillSetDto {
    fn from(value: SkillSetSummary) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            skill_ids: value
                .skill_ids
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkillIndexDiagnosticDto {
    pub skill_id: String,
    pub path: PathDto,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogIndexStatusDto {
    pub freshness: CatalogIndexFreshnessDto,
    pub revision: i64,
    pub last_reconciled_at_epoch_millis: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogIndexFreshnessDto {
    Fresh,
    Revalidating,
    Stale,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildCatalogIndexResponseDto {
    pub inserted: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub invalid: usize,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkillSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: SkillSourceDto,
    pub source_metadata: Option<SkillSourceMetadataDto>,
    pub location: PathDto,
    pub updated_at_epoch_millis: Option<i64>,
    pub deployment_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSourceMetadataDto {
    pub source: Option<String>,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub skill_path: Option<String>,
    pub skill_folder_hash: Option<String>,
    pub plugin_name: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<SkillLockEntry> for SkillSourceMetadataDto {
    fn from(value: SkillLockEntry) -> Self {
        Self {
            source: value.source,
            source_type: value.source_type,
            source_url: value.source_url,
            skill_path: value.skill_path,
            skill_folder_hash: value.skill_folder_hash,
            plugin_name: value.plugin_name,
            reference: value.reference,
            installed_at: value.installed_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkillDetailDto {
    pub skill: CatalogSkillSummaryDto,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanImportFolderRequestDto {
    pub root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateDto {
    pub path: PathDto,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFolderDiagnosticDto {
    pub path: PathDto,
    pub error: IpcError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanImportFolderResponseDto {
    pub root: PathDto,
    pub candidates: Vec<ImportCandidateDto>,
    pub diagnostics: Vec<ImportFolderDiagnosticDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportLocalSkillsRequestDto {
    pub root: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLocalSkillsResponseDto {
    pub imported_skill_ids: Vec<String>,
    pub skipped_paths: Vec<PathDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportCatalogSkillsRequestDto {
    pub destination_root: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCatalogSkillsResponseDto {
    pub export_root: PathDto,
    pub exported_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkillSourceDto {
    Local {
        path: PathDto,
    },
    Registry {
        registry: String,
        skill: String,
        version: Option<String>,
    },
    Git {
        url: String,
        revision: Option<String>,
        subdirectory: Option<PathDto>,
    },
}

impl From<CatalogSkillSummary> for CatalogSkillSummaryDto {
    fn from(value: CatalogSkillSummary) -> Self {
        catalog_skill_summary(
            value.snapshot,
            value.deployment_count,
            value.source_metadata,
        )
    }
}

impl From<CatalogSkillList> for CatalogSkillsResponseDto {
    fn from(value: CatalogSkillList) -> Self {
        Self {
            skills: value.skills.into_iter().map(Into::into).collect(),
            sets: value.sets.into_iter().map(Into::into).collect(),
            diagnostics: value
                .diagnostics
                .into_iter()
                .map(|diagnostic| CatalogSkillIndexDiagnosticDto {
                    skill_id: diagnostic.skill_id.to_string(),
                    path: PathDto::from(diagnostic.path.as_path()),
                    kind: diagnostic.kind,
                    message: diagnostic.message,
                })
                .collect(),
            index: CatalogIndexStatusDto {
                freshness: value.freshness.into(),
                revision: value.revision,
                last_reconciled_at_epoch_millis: value.last_reconciled_at_epoch_millis,
            },
        }
    }
}

impl From<CatalogIndexFreshness> for CatalogIndexFreshnessDto {
    fn from(value: CatalogIndexFreshness) -> Self {
        match value {
            CatalogIndexFreshness::Fresh => Self::Fresh,
            CatalogIndexFreshness::Revalidating => Self::Revalidating,
            CatalogIndexFreshness::Stale => Self::Stale,
        }
    }
}

impl From<CatalogIndexRebuildOutcome> for RebuildCatalogIndexResponseDto {
    fn from(value: CatalogIndexRebuildOutcome) -> Self {
        Self {
            inserted: value.inserted,
            updated: value.updated,
            removed: value.removed,
            unchanged: value.unchanged,
            invalid: value.invalid,
            revision: value.revision,
        }
    }
}

impl From<CatalogSkillDetail> for CatalogSkillDetailDto {
    fn from(value: CatalogSkillDetail) -> Self {
        Self {
            skill: value.summary.into(),
            body: value.body,
        }
    }
}

impl From<ImportCandidate> for ImportCandidateDto {
    fn from(value: ImportCandidate) -> Self {
        Self {
            path: PathDto::from(value.path.as_path()),
            name: value.name,
            description: value.description,
            version: value.version,
        }
    }
}

impl From<ImportFolderDiagnostic> for ImportFolderDiagnosticDto {
    fn from(value: ImportFolderDiagnostic) -> Self {
        Self {
            path: PathDto::from(value.path.as_path()),
            error: value.error.into(),
        }
    }
}

impl From<ImportFolderPreview> for ScanImportFolderResponseDto {
    fn from(value: ImportFolderPreview) -> Self {
        Self {
            root: PathDto::from(value.root.as_path()),
            candidates: value.candidates.into_iter().map(Into::into).collect(),
            diagnostics: value.diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ImportSkillsOutcome> for ImportLocalSkillsResponseDto {
    fn from(value: ImportSkillsOutcome) -> Self {
        Self {
            imported_skill_ids: value
                .imported
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
            skipped_paths: value
                .skipped
                .into_iter()
                .map(|path| PathDto::from(path.as_path()))
                .collect(),
        }
    }
}

impl From<ExportSkillsOutcome> for ExportCatalogSkillsResponseDto {
    fn from(value: ExportSkillsOutcome) -> Self {
        Self {
            export_root: PathDto::from(value.export_root.as_path()),
            exported_skill_ids: value
                .exported
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
        }
    }
}

fn catalog_skill_summary(
    snapshot: CentralSkillSnapshot,
    deployment_count: usize,
    source_metadata: Option<SkillLockEntry>,
) -> CatalogSkillSummaryDto {
    let installed = snapshot.installed;
    CatalogSkillSummaryDto {
        id: installed.id.to_string(),
        name: installed.metadata.name().to_owned(),
        description: installed.metadata.description().to_owned(),
        version: installed.metadata.version().map(ToOwned::to_owned),
        source: installed.source.into(),
        source_metadata: source_metadata.map(Into::into),
        location: PathDto::from(installed.location.as_path()),
        updated_at_epoch_millis: snapshot
            .version
            .marker_modified_at
            .and_then(system_time_epoch_millis),
        deployment_count,
    }
}

impl From<SkillSource> for SkillSourceDto {
    fn from(value: SkillSource) -> Self {
        match value {
            SkillSource::Local { path } => Self::Local {
                path: PathDto::from(path.as_path()),
            },
            SkillSource::Registry {
                registry,
                skill,
                version,
            } => Self::Registry {
                registry,
                skill,
                version,
            },
            SkillSource::Git {
                url,
                revision,
                subdirectory,
            } => Self::Git {
                url,
                revision,
                subdirectory: subdirectory.as_deref().map(PathDto::from),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesOverviewDto {
    pub agents_workspace_id: String,
    pub harnesses: Vec<HarnessSummaryDto>,
    pub workspaces: Vec<WorkspaceSummaryDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSummaryDto {
    pub id: String,
    pub display_name: String,
    pub category: HarnessCategoryDto,
    pub custom: bool,
    pub capabilities: HarnessCapabilitiesDto,
    pub skill_count: usize,
    pub linked_skill_ids: Vec<String>,
    pub probe: Option<HarnessProbeDto>,
    pub error: Option<IpcError>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HarnessCategoryDto {
    Coding,
    Lobster,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCapabilitiesDto {
    pub global_scope: bool,
    pub project_scope: bool,
    pub recursive_global_discovery: bool,
    pub configuration_path: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessProbeDto {
    pub detection_status: DetectionStatusDto,
    pub checked_paths: Vec<PathDto>,
    pub agent_path: PathDto,
    pub global_skills_path: PathDto,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionStatusDto {
    Installed,
    NotInstalled,
    ExplicitlyConfigured,
}

impl From<WorkspacesOverview> for WorkspacesOverviewDto {
    fn from(value: WorkspacesOverview) -> Self {
        Self {
            agents_workspace_id: value.agents_workspace_id.to_string(),
            harnesses: value.harnesses.into_iter().map(Into::into).collect(),
            workspaces: value.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<HarnessOverview> for HarnessSummaryDto {
    fn from(value: HarnessOverview) -> Self {
        let (probe, mut error) = match value.probe {
            Ok(probe) => (Some(probe.into()), None),
            Err(error) => (None, Some(error.into())),
        };
        if error.is_none() {
            error = value.scan_error.map(Into::into);
        }
        Self {
            id: value.id.to_string(),
            display_name: value.display_name,
            category: value.category.into(),
            custom: value.custom,
            capabilities: value.capabilities.into(),
            skill_count: value.skill_count,
            linked_skill_ids: value
                .linked_skill_ids
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
            probe,
            error,
        }
    }
}

impl From<HarnessCategory> for HarnessCategoryDto {
    fn from(value: HarnessCategory) -> Self {
        match value {
            HarnessCategory::Coding => Self::Coding,
            HarnessCategory::Lobster => Self::Lobster,
        }
    }
}

impl From<HarnessCapabilities> for HarnessCapabilitiesDto {
    fn from(value: HarnessCapabilities) -> Self {
        Self {
            global_scope: value.supports_global_scope,
            project_scope: value.supports_project_scope,
            recursive_global_discovery: value.recursive_global_discovery,
            configuration_path: value.supports_configuration_path,
        }
    }
}

impl From<HarnessProbe> for HarnessProbeDto {
    fn from(value: HarnessProbe) -> Self {
        let agent_path = value
            .global_skills_path
            .parent()
            .unwrap_or(value.global_skills_path.as_path());
        Self {
            detection_status: value.detection_status.into(),
            checked_paths: value
                .checked_paths
                .iter()
                .map(|path| PathDto::from(path.as_path()))
                .collect(),
            agent_path: PathDto::from(agent_path),
            global_skills_path: PathDto::from(value.global_skills_path.as_path()),
        }
    }
}

impl From<DetectionStatus> for DetectionStatusDto {
    fn from(value: DetectionStatus) -> Self {
        match value {
            DetectionStatus::Installed => Self::Installed,
            DetectionStatus::NotInstalled => Self::NotInstalled,
            DetectionStatus::ExplicitlyConfigured => Self::ExplicitlyConfigured,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummaryDto {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKindDto,
    pub deployment_mode: DeploymentModeDto,
    pub deployment_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceKindDto {
    Agents,
    Project {
        root: PathDto,
    },
    Linked {
        root: PathDto,
        disabled_root: Option<PathDto>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentModeDto {
    Copy,
    Link,
}

impl From<WorkspaceSummary> for WorkspaceSummaryDto {
    fn from(value: WorkspaceSummary) -> Self {
        workspace_summary(value.stored, value.deployment_count)
    }
}

fn workspace_summary(stored: StoredWorkspace, deployment_count: usize) -> WorkspaceSummaryDto {
    WorkspaceSummaryDto {
        id: stored.workspace.id.to_string(),
        name: stored.name,
        kind: stored.workspace.kind.into(),
        deployment_mode: stored.deployment_mode.into(),
        deployment_count,
    }
}

impl From<WorkspaceKind> for WorkspaceKindDto {
    fn from(value: WorkspaceKind) -> Self {
        match value {
            WorkspaceKind::Agents => Self::Agents,
            WorkspaceKind::Project { root } => Self::Project {
                root: PathDto::from(root.as_path()),
            },
            WorkspaceKind::Linked {
                root,
                disabled_root,
            } => Self::Linked {
                root: PathDto::from(root.as_path()),
                disabled_root: disabled_root.as_deref().map(PathDto::from),
            },
        }
    }
}

impl From<DeploymentMode> for DeploymentModeDto {
    fn from(value: DeploymentMode) -> Self {
        match value {
            DeploymentMode::Copy => Self::Copy,
            DeploymentMode::Link => Self::Link,
        }
    }
}

impl From<DeploymentModeDto> for DeploymentMode {
    fn from(value: DeploymentModeDto) -> Self {
        match value {
            DeploymentModeDto::Copy => Self::Copy,
            DeploymentModeDto::Link => Self::Link,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateWorkspaceRequestDto {
    pub name: String,
    pub kind: CreateWorkspaceKindDto,
    pub deployment_mode: DeploymentModeDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum CreateWorkspaceKindDto {
    Project {
        root: String,
    },
    Linked {
        root: String,
        #[serde(default)]
        disabled_root: Option<String>,
    },
}

impl From<CreateWorkspaceRequestDto> for CreateWorkspaceInput {
    fn from(value: CreateWorkspaceRequestDto) -> Self {
        let kind = match value.kind {
            CreateWorkspaceKindDto::Project { root } => {
                CreateWorkspaceKind::Project { root: root.into() }
            }
            CreateWorkspaceKindDto::Linked {
                root,
                disabled_root,
            } => CreateWorkspaceKind::Linked {
                root: root.into(),
                disabled_root: disabled_root.map(Into::into),
            },
        };
        Self {
            name: value.name,
            kind,
            deployment_mode: value.deployment_mode.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceIdRequestDto {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAgentRequestDto {
    pub agent_id: Option<String>,
    pub display_name: String,
    pub agent_root: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAgentResponseDto {
    pub agent_id: String,
    pub display_name: String,
    pub agent_root: PathDto,
    pub skills_root: PathDto,
    pub linked_skill_ids: Vec<String>,
    pub removed_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedAgentDto {
    pub detector_id: String,
    pub display_name: String,
    pub agent_root: PathDto,
    pub skill_count: usize,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetectionDiagnosticDto {
    pub detector_id: String,
    pub display_name: String,
    pub error: IpcError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectAgentsResponseDto {
    pub agents: Vec<DetectedAgentDto>,
    pub diagnostics: Vec<AgentDetectionDiagnosticDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddDetectedAgentsRequestDto {
    pub detector_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDetectedAgentsResponseDto {
    pub added_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAgentsRequestDto {
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAgentsResponseDto {
    pub deleted_agent_ids: Vec<String>,
    pub deleted_skill_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyProjectAgentSkillsRequestDto {
    pub workspace_id: String,
    pub agent_root: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyProjectAgentSkillsResponseDto {
    pub skills_root: PathDto,
    pub copied_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteProjectAgentsRequestDto {
    pub workspace_id: String,
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProjectAgentsResponseDto {
    pub deleted_agent_ids: Vec<String>,
    pub deleted_skill_count: usize,
}

impl From<SaveAgentRequestDto> for SaveAgentInput {
    fn from(value: SaveAgentRequestDto) -> Self {
        Self {
            agent_id: value.agent_id,
            display_name: value.display_name,
            agent_root: value.agent_root.into(),
            skill_ids: value.skill_ids,
        }
    }
}

impl From<SaveAgentOutcome> for SaveAgentResponseDto {
    fn from(value: SaveAgentOutcome) -> Self {
        Self {
            agent_id: value.agent_id.to_string(),
            display_name: value.display_name,
            agent_root: PathDto::from(value.agent_root.as_path()),
            skills_root: PathDto::from(value.skills.skills_root.as_path()),
            linked_skill_ids: value
                .skills
                .linked
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
            removed_skill_ids: value
                .skills
                .removed
                .into_iter()
                .map(|skill_id| skill_id.to_string())
                .collect(),
        }
    }
}

impl From<DetectedAgent> for DetectedAgentDto {
    fn from(value: DetectedAgent) -> Self {
        Self {
            detector_id: value.detector_id.to_string(),
            display_name: value.display_name,
            agent_root: PathDto::from(value.agent_root.as_path()),
            skill_count: value.skill_count,
            configured: value.configured,
        }
    }
}

impl From<AgentDetectionDiagnostic> for AgentDetectionDiagnosticDto {
    fn from(value: AgentDetectionDiagnostic) -> Self {
        let error = match value.error {
            crate::application::AgentDetectionError::Harness(error) => error.into(),
            crate::application::AgentDetectionError::Local(error) => error.into(),
        };
        Self {
            detector_id: value.detector_id.to_string(),
            display_name: value.display_name,
            error,
        }
    }
}

impl From<AgentDetectionOutcome> for DetectAgentsResponseDto {
    fn from(value: AgentDetectionOutcome) -> Self {
        Self {
            agents: value.agents.into_iter().map(Into::into).collect(),
            diagnostics: value.diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AddDetectedAgentsOutcome> for AddDetectedAgentsResponseDto {
    fn from(value: AddDetectedAgentsOutcome) -> Self {
        Self {
            added_agent_ids: value
                .added_agent_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

impl From<DeleteAgentsOutcome> for DeleteAgentsResponseDto {
    fn from(value: DeleteAgentsOutcome) -> Self {
        Self {
            deleted_agent_ids: value
                .deleted_agent_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            deleted_skill_count: value.deleted_skill_count,
        }
    }
}

impl From<CopyProjectAgentSkillsRequestDto> for CopyProjectAgentSkillsInput {
    fn from(value: CopyProjectAgentSkillsRequestDto) -> Self {
        Self {
            workspace_id: value.workspace_id,
            agent_root: value.agent_root.into(),
            skill_ids: value.skill_ids,
        }
    }
}

impl From<CopyProjectAgentSkillsOutcome> for CopyProjectAgentSkillsResponseDto {
    fn from(value: CopyProjectAgentSkillsOutcome) -> Self {
        Self {
            skills_root: PathDto::from(value.skills_root.as_path()),
            copied_skill_ids: value
                .copied_skill_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

impl From<DeleteProjectAgentsOutcome> for DeleteProjectAgentsResponseDto {
    fn from(value: DeleteProjectAgentsOutcome) -> Self {
        Self {
            deleted_agent_ids: value
                .deleted_agent_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            deleted_skill_count: value.deleted_skill_count,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillIdRequestDto {
    pub skill_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteCatalogSkillsRequestDto {
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCatalogSkillsResponseDto {
    pub deleted_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSkillSetRequestDto {
    pub name: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSkillSetRequestDto {
    pub set_id: String,
    pub name: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteSkillSetsRequestDto {
    pub set_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSkillSetsResponseDto {
    pub deleted_set_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceObservationDto {
    pub workspace: WorkspaceSummaryDto,
    pub resolution: WorkspaceResolutionDto,
    pub report: WorkspaceReportDto,
    pub project_agents: Vec<ProjectAgentDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentDto {
    pub id: String,
    pub display_name: String,
    pub path: PathDto,
    pub skill_count: usize,
    pub error: Option<IpcError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResolutionDto {
    pub targets: Vec<WorkspaceTargetDto>,
    pub discovery_roots: Vec<DiscoveryRootDto>,
    pub unsupported: Vec<UnsupportedWorkspaceTargetDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTargetDto {
    pub harness_id: String,
    pub path: PathDto,
    pub role: TargetRoleDto,
    pub scan_mode: ScanModeDto,
    pub deployment_mode: DeploymentModeDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRootDto {
    pub path: PathDto,
    pub scan_mode: ScanModeDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedWorkspaceTargetDto {
    pub harness_id: String,
    pub path: PathDto,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetRoleDto {
    Primary,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanModeDto {
    Flat,
    Recursive,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReportDto {
    pub workspace_id: String,
    pub observations: Vec<DeploymentObservationDto>,
    pub unmatched_local: Vec<UnmatchedLocalSkillDto>,
    pub diagnostics: Vec<WorkspaceDiagnosticDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentObservationDto {
    pub key: DeploymentKeyDto,
    pub target_path: PathDto,
    pub role: TargetRoleDto,
    pub status: DeploymentStatusDto,
    pub center: Option<ObservedSkillDto>,
    pub local_modified_at_epoch_millis: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentKeyDto {
    pub skill_id: String,
    pub harness_id: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentStatusDto {
    NotDeployed,
    InSync,
    LocalNewer,
    CenterNewer,
    Missing,
    Unsupported,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedSkillDto {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchedLocalSkillDto {
    pub name: String,
    pub description: String,
    pub path: PathDto,
    pub marker: SkillMarkerDto,
    pub target: Option<WorkspaceTargetDto>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillMarkerDto {
    Canonical,
    Legacy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiagnosticDto {
    pub path: PathDto,
    pub status: DeploymentStatusDto,
    pub error: IpcError,
}

impl From<WorkspaceObservation> for WorkspaceObservationDto {
    fn from(value: WorkspaceObservation) -> Self {
        Self {
            workspace: value.workspace.into(),
            resolution: value.resolution.into(),
            report: value.report.into(),
            project_agents: value.project_agents.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ProjectAgentOverview> for ProjectAgentDto {
    fn from(value: ProjectAgentOverview) -> Self {
        Self {
            id: value.id.to_string(),
            display_name: value.display_name,
            path: PathDto::from(value.path.as_path()),
            skill_count: value.skill_count,
            error: value.error.map(Into::into),
        }
    }
}

impl From<WorkspaceResolution> for WorkspaceResolutionDto {
    fn from(value: WorkspaceResolution) -> Self {
        Self {
            targets: value.targets.into_iter().map(Into::into).collect(),
            discovery_roots: value.discovery_roots.into_iter().map(Into::into).collect(),
            unsupported: value.unsupported.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WorkspaceTarget> for WorkspaceTargetDto {
    fn from(value: WorkspaceTarget) -> Self {
        Self {
            harness_id: value.harness_id.to_string(),
            path: PathDto::from(value.path.as_path()),
            role: value.role.into(),
            scan_mode: value.scan_mode.into(),
            deployment_mode: value.deployment_mode.into(),
        }
    }
}

impl From<DiscoveryRoot> for DiscoveryRootDto {
    fn from(value: DiscoveryRoot) -> Self {
        Self {
            path: PathDto::from(value.path.as_path()),
            scan_mode: value.scan_mode.into(),
        }
    }
}

impl From<UnsupportedWorkspaceTarget> for UnsupportedWorkspaceTargetDto {
    fn from(value: UnsupportedWorkspaceTarget) -> Self {
        Self {
            harness_id: value.harness_id.to_string(),
            path: PathDto::from(value.path.as_path()),
            reason: value.reason.to_owned(),
        }
    }
}

impl From<TargetRole> for TargetRoleDto {
    fn from(value: TargetRole) -> Self {
        match value {
            TargetRole::Primary => Self::Primary,
            TargetRole::Disabled => Self::Disabled,
        }
    }
}

impl From<ScanMode> for ScanModeDto {
    fn from(value: ScanMode) -> Self {
        match value {
            ScanMode::Flat => Self::Flat,
            ScanMode::Recursive => Self::Recursive,
        }
    }
}

impl From<WorkspaceReport> for WorkspaceReportDto {
    fn from(value: WorkspaceReport) -> Self {
        Self {
            workspace_id: value.workspace_id.to_string(),
            observations: value.observations.into_iter().map(Into::into).collect(),
            unmatched_local: value.unmatched_local.into_iter().map(Into::into).collect(),
            diagnostics: value.diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<DeploymentObservation> for DeploymentObservationDto {
    fn from(value: DeploymentObservation) -> Self {
        Self {
            key: value.key.into(),
            target_path: PathDto::from(value.target_path.as_path()),
            role: value.role.into(),
            status: value.status.into(),
            center: value.center.map(|snapshot| snapshot.installed.into()),
            local_modified_at_epoch_millis: value
                .local
                .and_then(|version| version.marker_modified_at)
                .and_then(system_time_epoch_millis),
        }
    }
}

impl From<DeploymentKey> for DeploymentKeyDto {
    fn from(value: DeploymentKey) -> Self {
        Self {
            skill_id: value.skill_id.to_string(),
            harness_id: value.harness_id.to_string(),
            workspace_id: value.workspace_id.to_string(),
        }
    }
}

impl From<DeploymentStatus> for DeploymentStatusDto {
    fn from(value: DeploymentStatus) -> Self {
        match value {
            DeploymentStatus::NotDeployed => Self::NotDeployed,
            DeploymentStatus::InSync => Self::InSync,
            DeploymentStatus::LocalNewer => Self::LocalNewer,
            DeploymentStatus::CenterNewer => Self::CenterNewer,
            DeploymentStatus::Missing => Self::Missing,
            DeploymentStatus::Unsupported => Self::Unsupported,
            DeploymentStatus::Error => Self::Error,
        }
    }
}

impl From<InstalledSkill> for ObservedSkillDto {
    fn from(value: InstalledSkill) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.metadata.name().to_owned(),
            description: value.metadata.description().to_owned(),
        }
    }
}

impl From<UnmatchedLocalSkill> for UnmatchedLocalSkillDto {
    fn from(value: UnmatchedLocalSkill) -> Self {
        Self {
            name: value.scanned.document.metadata().name().to_owned(),
            description: value.scanned.document.metadata().description().to_owned(),
            path: PathDto::from(value.scanned.path.as_path()),
            marker: value.scanned.marker.into(),
            target: value.target.map(Into::into),
        }
    }
}

impl From<SkillMarker> for SkillMarkerDto {
    fn from(value: SkillMarker) -> Self {
        match value {
            SkillMarker::Canonical => Self::Canonical,
            SkillMarker::Legacy => Self::Legacy,
        }
    }
}

impl From<WorkspaceDiagnostic> for WorkspaceDiagnosticDto {
    fn from(value: WorkspaceDiagnostic) -> Self {
        Self {
            path: PathDto::from(value.path.as_path()),
            status: value.status.into(),
            error: value.error.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReconcileOutcomeDto {
    pub requested_workspace: WorkspaceSummaryDto,
    pub requested: ReconcileReportDto,
    pub propagated: Vec<PropagationOutcomeDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReportDto {
    pub workspace_id: String,
    pub imported: Vec<String>,
    pub center_updated: Vec<String>,
    pub propagated: Vec<DeploymentKeyDto>,
    pub final_report: WorkspaceReportDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropagationOutcomeDto {
    pub workspace: WorkspaceSummaryDto,
    pub report: Option<ReconcileReportDto>,
    pub error: Option<IpcError>,
}

impl From<WorkspaceReconcileOutcome> for WorkspaceReconcileOutcomeDto {
    fn from(value: WorkspaceReconcileOutcome) -> Self {
        Self {
            requested_workspace: workspace_summary(value.requested_workspace, 0),
            requested: value.requested.into(),
            propagated: value.propagated.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ReconcileReport> for ReconcileReportDto {
    fn from(value: ReconcileReport) -> Self {
        Self {
            workspace_id: value.workspace_id.to_string(),
            imported: value
                .imported
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            center_updated: value
                .center_updated
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            propagated: value.propagated.into_iter().map(Into::into).collect(),
            final_report: value.final_report.into(),
        }
    }
}

impl From<PropagationOutcome> for PropagationOutcomeDto {
    fn from(value: PropagationOutcome) -> Self {
        match value.result {
            Ok(report) => Self {
                workspace: workspace_summary(value.workspace, 0),
                report: Some(report.into()),
                error: None,
            },
            Err(error) => Self {
                workspace: workspace_summary(value.workspace, 0),
                report: None,
                error: Some(error.into()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub catalog_root: PathDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCatalogRootRequestDto {
    pub catalog_root: String,
}

impl From<AppSettings> for AppSettingsDto {
    fn from(value: AppSettings) -> Self {
        Self {
            catalog_root: PathDto::from(value.catalog_root.as_path()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrySearchRequestDto {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryLeaderboardRequestDto {
    pub leaderboard: LeaderboardDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeaderboardDto {
    AllTime,
    Trending,
    Hot,
}

impl From<LeaderboardDto> for Leaderboard {
    fn from(value: LeaderboardDto) -> Self {
        match value {
            LeaderboardDto::AllTime => Self::AllTime,
            LeaderboardDto::Trending => Self::Trending,
            LeaderboardDto::Hot => Self::Hot,
        }
    }
}

impl From<Leaderboard> for LeaderboardDto {
    fn from(value: Leaderboard) -> Self {
        match value {
            Leaderboard::AllTime => Self::AllTime,
            Leaderboard::Trending => Self::Trending,
            Leaderboard::Hot => Self::Hot,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResultDto {
    pub mode: RegistryResultModeDto,
    pub leaderboard: Option<LeaderboardDto>,
    pub query: Option<String>,
    pub skills: Vec<RegistrySkillSummaryDto>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RegistryResultModeDto {
    Leaderboard,
    Search,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkillSummaryDto {
    pub id: RegistrySkillIdDto,
    pub name: String,
    pub installs: u64,
    pub source_kind: Option<String>,
    pub official: Option<bool>,
    pub details_url: Option<String>,
    pub rank: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkillIdDto {
    pub source: String,
    pub skill_id: String,
}

impl RegistryResultDto {
    pub fn from_search(query: String, value: SearchResult) -> Self {
        Self {
            mode: RegistryResultModeDto::Search,
            leaderboard: None,
            query: Some(query),
            skills: value.skills.into_iter().map(Into::into).collect(),
        }
    }

    pub fn from_leaderboard(value: LeaderboardResult) -> Self {
        Self {
            mode: RegistryResultModeDto::Leaderboard,
            leaderboard: Some(value.leaderboard.into()),
            query: None,
            skills: value.skills.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RemoteSkillSummary> for RegistrySkillSummaryDto {
    fn from(value: RemoteSkillSummary) -> Self {
        Self {
            id: RegistrySkillIdDto {
                source: value.id.source,
                skill_id: value.id.skill_id,
            },
            name: value.name,
            installs: value.installs,
            source_kind: value.source_kind.map(|kind| kind.label().to_owned()),
            official: value.is_official,
            details_url: value.skills_sh_url,
            rank: value.rank,
        }
    }
}

fn system_time_epoch_millis(value: std::time::SystemTime) -> Option<i64> {
    let millis = value.duration_since(UNIX_EPOCH).ok()?.as_millis();
    i64::try_from(millis).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_workspace_request_uses_explicit_camel_case_contract() {
        let request: CreateWorkspaceRequestDto = serde_json::from_value(serde_json::json!({
            "name": "Project",
            "kind": { "kind": "project", "root": "C:/project" },
            "deploymentMode": "copy"
        }))
        .unwrap();

        assert_eq!(request.name, "Project");
        assert_eq!(request.deployment_mode, DeploymentModeDto::Copy);
        assert!(matches!(
            request.kind,
            CreateWorkspaceKindDto::Project { .. }
        ));
    }

    #[test]
    fn deployment_mode_contract_accepts_link() {
        let link: CreateWorkspaceRequestDto = serde_json::from_value(serde_json::json!({
            "name": "Linked project",
            "kind": { "kind": "project", "root": "C:/project" },
            "deploymentMode": "link"
        }))
        .unwrap();
        assert_eq!(link.deployment_mode, DeploymentModeDto::Link);
    }

    #[test]
    fn request_rejects_unknown_fields() {
        let result = serde_json::from_value::<UpdateCatalogRootRequestDto>(serde_json::json!({
            "catalogRoot": "C:/catalog",
            "unexpected": true
        }));

        assert!(result.is_err());
    }
}
