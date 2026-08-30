use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use skill_core::{SkillId, SkillIdError};
use skill_harness::{
    DetectionStatus, HarnessCapabilities, HarnessCategory, HarnessEnvironment, HarnessError,
    HarnessId, HarnessRegistry,
};
use skill_workspace::{
    choose_newest_local, resolve_workspace, CentralCatalogPort, CentralSkillSnapshot,
    DeploymentMode, DeploymentStatus, LocalCandidate, LocalSkillPort, ReconcileReport,
    SkillVersion, SystemLocalSkillPort, WorkspaceEngine, WorkspaceError, WorkspaceId,
    WorkspaceIdError, WorkspaceKind, WorkspaceReport, WorkspaceResolution,
};
use thiserror::Error;

use crate::persistence::{CatalogActivityKind, PersistenceError, SqliteCatalog, StoredWorkspace};

const MAX_WORKSPACE_NAME_CHARS: usize = 120;
const DASHBOARD_WEEK_COUNT: i64 = 12;
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;

pub struct Application {
    catalog: SqliteCatalog,
    harnesses: HarnessRegistry,
    environment: HarnessEnvironment,
    local: SystemLocalSkillPort,
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Harness(#[from] HarnessError),
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
pub struct CatalogSkillDetail {
    pub summary: CatalogSkillSummary,
    pub body: String,
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
    pub deployment_count: usize,
    pub probe: Result<HarnessProbe, HarnessError>,
}

#[derive(Debug, Clone)]
pub struct HarnessProbe {
    pub detection_status: DetectionStatus,
    pub checked_paths: Vec<PathBuf>,
    pub global_skills_path: PathBuf,
}

#[derive(Debug)]
pub struct WorkspaceObservation {
    pub workspace: WorkspaceSummary,
    pub resolution: WorkspaceResolution,
    pub report: WorkspaceReport,
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
    pub result: Result<ReconcileReport, WorkspaceError>,
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
        let catalog = SqliteCatalog::open(database_path, default_catalog_root)?;
        let application = Self {
            catalog,
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::from_system()?,
            local: SystemLocalSkillPort,
        };
        application.validate_catalog_root_against_workspaces(application.catalog.catalog_root())?;
        Ok(application)
    }

    pub fn dashboard_overview(&self) -> Result<DashboardOverview, ApplicationError> {
        let skills = self.catalog.list_catalog_skills()?;
        let bindings = self.catalog.all_bindings()?;
        let workspaces = self.catalog.list_workspaces()?;
        let mut detected_harnesses = 0;
        let mut diagnostics = Vec::new();
        for harness in self.harnesses.adapters() {
            match harness.detect(&self.environment) {
                Ok(detection) if detection.is_installed() => detected_harnesses += 1,
                Ok(_) => {}
                Err(error) => diagnostics.push(ApplicationDiagnostic {
                    code: "harness.probe_failed",
                    message: format!("{}: {error}", harness.display_name()),
                }),
            }
        }

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

    pub fn list_catalog_skills(&self) -> Result<Vec<CatalogSkillSummary>, ApplicationError> {
        let bindings = self.catalog.all_bindings()?;
        let mut deployment_counts = HashMap::<SkillId, usize>::new();
        for binding in bindings {
            *deployment_counts.entry(binding.key.skill_id).or_default() += 1;
        }
        Ok(self
            .catalog
            .list_catalog_skills()?
            .into_iter()
            .map(|snapshot| CatalogSkillSummary {
                deployment_count: deployment_counts
                    .get(&snapshot.installed.id)
                    .copied()
                    .unwrap_or_default(),
                snapshot,
            })
            .collect())
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

        let mut harnesses = Vec::new();
        for adapter in self.harnesses.adapters() {
            let probe = (|| {
                let detection = adapter.detect(&self.environment)?;
                let locations = adapter.resolve_locations(&self.environment, None)?;
                Ok(HarnessProbe {
                    detection_status: detection.status,
                    checked_paths: detection.checked_paths,
                    global_skills_path: locations.global_skills_dir,
                })
            })();
            harnesses.push(HarnessOverview {
                id: adapter.id().clone(),
                display_name: adapter.display_name().to_owned(),
                category: adapter.category(),
                custom: adapter.is_custom(),
                capabilities: adapter.capabilities(),
                deployment_count: bindings
                    .iter()
                    .filter(|binding| {
                        binding.key.workspace_id == agents_workspace_id
                            && binding.key.harness_id == *adapter.id()
                    })
                    .count(),
                probe,
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
        let resolution = resolve_workspace(
            &stored.workspace,
            &self.harnesses,
            &self.environment,
            stored.deployment_mode,
        )?;
        let report = WorkspaceEngine::new(&self.local, &mut self.catalog).observe(
            &stored.workspace,
            &self.harnesses,
            &self.environment,
            stored.deployment_mode,
        )?;
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
        let initial_report = WorkspaceEngine::new(&self.local, &mut self.catalog).observe(
            &requested_workspace.workspace,
            &self.harnesses,
            &self.environment,
            requested_workspace.deployment_mode,
        )?;
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

        let mut requested = WorkspaceEngine::new(&self.local, &mut self.catalog).reconcile(
            &requested_workspace.workspace,
            &self.harnesses,
            &self.environment,
            requested_workspace.deployment_mode,
        )?;

        if !applies_plan_before_requested
            && (!requested.center_updated.is_empty() || !requested.imported.is_empty())
        {
            let updated_after_requested = self.apply_center_update_plan(&update_plan)?;
            if !updated_after_requested.is_empty() {
                let final_pass = WorkspaceEngine::new(&self.local, &mut self.catalog).reconcile(
                    &requested_workspace.workspace,
                    &self.harnesses,
                    &self.environment,
                    requested_workspace.deployment_mode,
                )?;
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
                let result = WorkspaceEngine::new(&self.local, &mut self.catalog).reconcile(
                    &workspace.workspace,
                    &self.harnesses,
                    &self.environment,
                    workspace.deployment_mode,
                );
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
            let report = WorkspaceEngine::new(&self.local, &mut self.catalog).observe(
                &stored.workspace,
                &self.harnesses,
                &self.environment,
                stored.deployment_mode,
            )?;
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
            catalog: SqliteCatalog::open(app_data.join("state.sqlite3"), app_data.join("catalog"))
                .unwrap(),
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::new(home, None),
            local: SystemLocalSkillPort,
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
    fn reconcile_selects_newest_candidate_across_workspaces() {
        let root = tempdir().unwrap();
        let app_data = root.path().join("app-data");
        let home = root.path().join("home");
        let first_root = root.path().join("first");
        let second_root = root.path().join("second");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&home).unwrap();

        let catalog =
            SqliteCatalog::open(app_data.join("state.sqlite3"), app_data.join("catalog")).unwrap();
        let mut application = Application {
            catalog,
            harnesses: HarnessRegistry::with_builtins(),
            environment: HarnessEnvironment::new(home, None),
            local: SystemLocalSkillPort,
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
