use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
};

use skill_core::SkillId;
use skill_harness::{HarnessEnvironment, HarnessError, HarnessId, HarnessRegistry};
use skill_local::{LocalError, ScanMode, ScannedSkill};

use crate::{
    error::{CatalogFailure, WorkspaceError},
    model::{
        choose_newest_local, classify_deployment, compare_paths, normalized_path_key, path_key,
        DeploymentBinding, DeploymentKey, DeploymentMode, DeploymentObservation, DeploymentStatus,
        DiscoveryRoot, LocalCandidate, ReconcileReport, SkillVersion, TargetRole,
        UnmatchedLocalSkill, UnsupportedWorkspaceTarget, Workspace, WorkspaceDiagnostic,
        WorkspaceId, WorkspaceKind, WorkspaceReport, WorkspaceResolution, WorkspaceTarget,
    },
    ports::{CentralCatalogPort, CentralMatch, LocalSkillPort},
};

const PROJECT_SCOPE_UNSUPPORTED_REASON: &str = "project scope is not supported";
const EMPTY_LINKED_ROOT_REASON: &str = "linked workspace root must not be empty";
const PARENT_LINKED_ROOT_REASON: &str =
    "linked workspace root must not contain parent directory segments";
const OVERLAPPING_LINKED_ROOTS_REASON: &str =
    "linked workspace roots must be distinct and non-nested";

pub fn resolve_workspace(
    workspace: &Workspace,
    harnesses: &HarnessRegistry,
    environment: &HarnessEnvironment,
    deployment_mode: DeploymentMode,
) -> Result<WorkspaceResolution, WorkspaceError> {
    match &workspace.kind {
        WorkspaceKind::Agents => {
            resolve_agents(workspace.id, harnesses, environment, deployment_mode)
        }
        WorkspaceKind::Project { root } => {
            resolve_project(workspace.id, root, harnesses, environment, deployment_mode)
        }
        WorkspaceKind::Linked {
            root,
            disabled_root,
        } => resolve_linked(
            workspace.id,
            root,
            disabled_root.as_deref(),
            deployment_mode,
        ),
    }
}

const BINDING_PARENT_DIR_REASON: &str =
    "deployment binding target must not contain parent directory segments";
const BINDING_WORKSPACE_REASON: &str = "deployment binding belongs to another workspace";
const BINDING_OUTSIDE_TARGET_REASON: &str =
    "deployment binding target is outside the workspace target root";
const BINDING_LINKED_PARENT_REASON: &str =
    "deployment binding parent path must not contain symbolic links or junctions";

pub struct WorkspaceEngine<L, C> {
    local: L,
    catalog: C,
}

impl<L, C> WorkspaceEngine<L, C> {
    pub fn new(local: L, catalog: C) -> Self {
        Self { local, catalog }
    }
}

impl<L, C> WorkspaceEngine<L, C>
where
    L: LocalSkillPort,
    C: CentralCatalogPort,
{
    pub fn observe(
        &self,
        workspace: &Workspace,
        harnesses: &HarnessRegistry,
        environment: &HarnessEnvironment,
        deployment_mode: DeploymentMode,
    ) -> Result<WorkspaceReport, WorkspaceError> {
        let resolution = resolve_workspace(workspace, harnesses, environment, deployment_mode)?;
        let mut diagnostics = Vec::new();

        for unsupported in &resolution.unsupported {
            diagnostics.push(WorkspaceDiagnostic {
                path: unsupported.path.clone(),
                status: DeploymentStatus::Unsupported,
                error: WorkspaceError::Unsupported {
                    workspace_id: workspace.id,
                    harness_id: unsupported.harness_id.clone(),
                    reason: unsupported.reason,
                },
            });
        }

        let center_snapshots = match self.catalog.list() {
            Ok(snapshots) => Some(snapshots),
            Err(source) => {
                diagnostics.push(catalog_diagnostic("list", source));
                None
            }
        };
        let bindings = match self.catalog.bindings(workspace.id) {
            Ok(bindings) => Some(bindings),
            Err(source) => {
                diagnostics.push(catalog_diagnostic("bindings", source));
                None
            }
        };

        let inventory = self.scan_inventory(&resolution, &mut diagnostics);
        let mut matched_local = Vec::new();
        let mut unmatched_local = Vec::new();
        let mut match_failures = HashSet::new();

        if center_snapshots.is_some() && bindings.is_some() {
            for item in inventory.skills {
                let target_path = item.scanned.path.clone();
                if let Some(target) = item.target.as_ref() {
                    if let Err(error) = validate_target_path(workspace, target, &target_path) {
                        match_failures.insert(normalized_path_key(&target_path));
                        diagnostics.push(WorkspaceDiagnostic {
                            path: target_path,
                            status: DeploymentStatus::Error,
                            error,
                        });
                        continue;
                    }
                }
                match self.catalog.resolve_match(&item.scanned, &target_path) {
                    Ok(CentralMatch::Unique(skill_id)) => {
                        matched_local.push(MatchedLocalSkill {
                            skill_id,
                            scanned: item.scanned,
                            target: item.target,
                        });
                    }
                    Ok(CentralMatch::None) => {
                        unmatched_local.push(UnmatchedLocalSkill {
                            scanned: item.scanned,
                            target: item.target,
                        });
                    }
                    Ok(CentralMatch::Ambiguous(mut candidates)) => {
                        candidates.sort_by_key(|candidate| candidate.to_string());
                        match_failures.insert(normalized_path_key(&target_path));
                        diagnostics.push(WorkspaceDiagnostic {
                            path: target_path.clone(),
                            status: DeploymentStatus::Error,
                            error: WorkspaceError::AmbiguousMatch {
                                path: target_path,
                                candidates,
                            },
                        });
                    }
                    Err(source) => {
                        match_failures.insert(normalized_path_key(&target_path));
                        diagnostics.push(WorkspaceDiagnostic {
                            path: target_path.clone(),
                            status: DeploymentStatus::Error,
                            error: WorkspaceError::Catalog {
                                operation: "resolve_match",
                                source,
                            },
                        });
                    }
                }
            }
        }

        let mut observations = Vec::new();
        for binding in bindings.into_iter().flatten() {
            let target = match target_for_binding(workspace, &resolution, &binding) {
                Ok(target) => target,
                Err(error) => {
                    diagnostics.push(WorkspaceDiagnostic {
                        path: binding.target_path.clone(),
                        status: DeploymentStatus::Error,
                        error,
                    });
                    continue;
                }
            };

            let local = local_candidate_for_binding(&matched_local, &binding, target);
            let local_version = local.map(|candidate| SkillVersion {
                content_hash: candidate.scanned.content_hash,
                marker_modified_at: candidate.scanned.marker_modified_at,
            });
            let center = center_snapshots
                .as_ref()
                .and_then(|snapshots| {
                    snapshots
                        .iter()
                        .find(|snapshot| snapshot.installed.id == binding.key.skill_id)
                })
                .cloned();
            let operation_failed = center_snapshots.is_none()
                || inventory
                    .failures
                    .iter()
                    .any(|failure| failure.affects(&binding, target))
                || match_failures.contains(&normalized_path_key(&binding.target_path));
            let status = if operation_failed {
                DeploymentStatus::Error
            } else {
                classify_deployment(
                    true,
                    center.as_ref().map(|snapshot| &snapshot.version),
                    local_version.as_ref(),
                )
            };

            observations.push(DeploymentObservation {
                key: binding.key,
                target_path: binding.target_path,
                role: target.role,
                center,
                local: local_version,
                status,
            });
        }

        sort_observations(&mut observations);
        sort_unmatched_local(&mut unmatched_local);
        sort_diagnostics(&mut diagnostics);

        Ok(WorkspaceReport {
            workspace_id: workspace.id,
            observations,
            unmatched_local,
            diagnostics,
        })
    }

    pub fn reconcile(
        &mut self,
        workspace: &Workspace,
        harnesses: &HarnessRegistry,
        environment: &HarnessEnvironment,
        deployment_mode: DeploymentMode,
    ) -> Result<ReconcileReport, WorkspaceError> {
        let resolution = resolve_workspace(workspace, harnesses, environment, deployment_mode)?;
        let mut operation_diagnostics = Vec::new();

        let initial_center_snapshots = match self.catalog.list() {
            Ok(snapshots) => Some(snapshots),
            Err(source) => {
                operation_diagnostics.push(catalog_diagnostic("list", source));
                None
            }
        };
        let initial_bindings = match self.catalog.bindings(workspace.id) {
            Ok(bindings) => Some(bindings),
            Err(source) => {
                operation_diagnostics.push(catalog_diagnostic("bindings", source));
                None
            }
        };

        if initial_center_snapshots.is_none() || initial_bindings.is_none() {
            let mut final_report =
                self.observe(workspace, harnesses, environment, deployment_mode)?;
            for diagnostic in operation_diagnostics {
                if !final_report
                    .diagnostics
                    .iter()
                    .any(|existing| same_diagnostic(existing, &diagnostic))
                {
                    final_report.diagnostics.push(diagnostic);
                }
            }
            sort_diagnostics(&mut final_report.diagnostics);

            return Ok(ReconcileReport {
                workspace_id: workspace.id,
                imported: Vec::new(),
                center_updated: Vec::new(),
                propagated: Vec::new(),
                final_report,
            });
        }

        let inventory = self.scan_inventory(&resolution, &mut operation_diagnostics);
        let scan_failures = inventory.failures;
        let mut matched_local = Vec::new();
        let mut unmatched_local = Vec::new();
        let mut match_failures = HashSet::new();
        let mut known_binding_keys: HashSet<DeploymentKey> = initial_bindings
            .iter()
            .flatten()
            .map(|binding| binding.key.clone())
            .collect();
        let mut bindings_changed = false;

        for item in inventory.skills {
            let target_path = item.scanned.path.clone();
            if let Some(target) = item.target.as_ref() {
                if let Err(error) = validate_target_path(workspace, target, &target_path) {
                    match_failures.insert(normalized_path_key(&target_path));
                    operation_diagnostics.push(WorkspaceDiagnostic {
                        path: target_path,
                        status: DeploymentStatus::Error,
                        error,
                    });
                    continue;
                }
            }
            match self.catalog.resolve_match(&item.scanned, &target_path) {
                Ok(CentralMatch::Unique(skill_id)) => {
                    if let Some(target) = item.target.as_ref() {
                        let key = DeploymentKey {
                            skill_id,
                            harness_id: target.harness_id.clone(),
                            workspace_id: target.workspace_id,
                        };
                        if !known_binding_keys.contains(&key) {
                            let binding = DeploymentBinding {
                                key: key.clone(),
                                target_path: item.scanned.path.clone(),
                                deployment_mode: target.deployment_mode,
                            };
                            match target_for_binding(workspace, &resolution, &binding) {
                                Ok(_) => match self.catalog.associate(binding) {
                                    Ok(()) => {
                                        known_binding_keys.insert(key);
                                        bindings_changed = true;
                                    }
                                    Err(source) => {
                                        operation_diagnostics.push(catalog_path_diagnostic(
                                            target_path.clone(),
                                            "associate",
                                            source,
                                        ));
                                    }
                                },
                                Err(error) => {
                                    operation_diagnostics.push(WorkspaceDiagnostic {
                                        path: target_path.clone(),
                                        status: DeploymentStatus::Error,
                                        error,
                                    });
                                }
                            }
                        }
                    }

                    matched_local.push(MatchedLocalSkill {
                        skill_id,
                        scanned: item.scanned,
                        target: item.target,
                    });
                }
                Ok(CentralMatch::None) => {
                    unmatched_local.push(UnmatchedLocalSkill {
                        scanned: item.scanned,
                        target: item.target,
                    });
                }
                Ok(CentralMatch::Ambiguous(mut candidates)) => {
                    candidates.sort_by_key(|candidate| candidate.to_string());
                    match_failures.insert(normalized_path_key(&target_path));
                    operation_diagnostics.push(WorkspaceDiagnostic {
                        path: target_path.clone(),
                        status: DeploymentStatus::Error,
                        error: WorkspaceError::AmbiguousMatch {
                            path: target_path,
                            candidates,
                        },
                    });
                }
                Err(source) => {
                    match_failures.insert(normalized_path_key(&target_path));
                    operation_diagnostics.push(catalog_path_diagnostic(
                        target_path,
                        "resolve_match",
                        source,
                    ));
                }
            }
        }
        sort_unmatched_local(&mut unmatched_local);

        let mut imported = Vec::new();
        let mut pending_bindings = Vec::new();
        for unmatched in unmatched_local {
            let scanned = unmatched.scanned;
            let target = unmatched.target;
            let target_path = scanned.path.clone();
            let matched_after_import = if imported.is_empty() {
                None
            } else {
                match self.catalog.resolve_match(&scanned, &target_path) {
                    Ok(CentralMatch::Unique(skill_id)) => Some(skill_id),
                    Ok(CentralMatch::None) => None,
                    Ok(CentralMatch::Ambiguous(mut candidates)) => {
                        candidates.sort_by_key(|candidate| candidate.to_string());
                        match_failures.insert(normalized_path_key(&target_path));
                        operation_diagnostics.push(WorkspaceDiagnostic {
                            path: target_path.clone(),
                            status: DeploymentStatus::Error,
                            error: WorkspaceError::AmbiguousMatch {
                                path: target_path,
                                candidates,
                            },
                        });
                        continue;
                    }
                    Err(source) => {
                        match_failures.insert(normalized_path_key(&target_path));
                        operation_diagnostics.push(catalog_path_diagnostic(
                            target_path,
                            "resolve_match",
                            source,
                        ));
                        continue;
                    }
                }
            };

            let skill_id = match matched_after_import {
                Some(skill_id) => skill_id,
                None => {
                    let imported_snapshot = match self.catalog.import_local(&scanned) {
                        Ok(snapshot) => snapshot,
                        Err(source) => {
                            operation_diagnostics.push(catalog_path_diagnostic(
                                scanned.path.clone(),
                                "import_local",
                                source,
                            ));
                            continue;
                        }
                    };
                    let skill_id = imported_snapshot.installed.id;
                    imported.push(skill_id);
                    skill_id
                }
            };

            let Some(target) = target else {
                if matched_after_import.is_some() {
                    matched_local.push(MatchedLocalSkill {
                        skill_id,
                        scanned,
                        target: None,
                    });
                }
                continue;
            };
            let binding = DeploymentBinding {
                key: DeploymentKey {
                    skill_id,
                    harness_id: target.harness_id.clone(),
                    workspace_id: target.workspace_id,
                },
                target_path: scanned.path.clone(),
                deployment_mode: target.deployment_mode,
            };
            if let Err(error) = target_for_binding(workspace, &resolution, &binding) {
                operation_diagnostics.push(WorkspaceDiagnostic {
                    path: binding.target_path.clone(),
                    status: DeploymentStatus::Error,
                    error,
                });
                continue;
            }

            pending_bindings.push((binding, skill_id, scanned, target));
        }

        let mut center_snapshots = initial_center_snapshots;
        let mut current_bindings = initial_bindings;
        if !imported.is_empty() {
            center_snapshots = match self.catalog.list() {
                Ok(snapshots) => Some(snapshots),
                Err(source) => {
                    operation_diagnostics.push(catalog_diagnostic("list", source));
                    None
                }
            };
            current_bindings = match self.catalog.bindings(workspace.id) {
                Ok(bindings) => Some(bindings),
                Err(source) => {
                    operation_diagnostics.push(catalog_diagnostic("bindings", source));
                    None
                }
            };

            if center_snapshots.is_some() && current_bindings.is_some() {
                for (binding, skill_id, scanned, target) in pending_bindings {
                    match self.catalog.associate(binding) {
                        Ok(()) => {
                            bindings_changed = true;
                            matched_local.push(MatchedLocalSkill {
                                skill_id,
                                scanned,
                                target: Some(target),
                            });
                        }
                        Err(source) => {
                            operation_diagnostics.push(catalog_path_diagnostic(
                                scanned.path.clone(),
                                "associate",
                                source,
                            ));
                        }
                    }
                }
            }
        }
        if bindings_changed {
            current_bindings = match self.catalog.bindings(workspace.id) {
                Ok(bindings) => Some(bindings),
                Err(source) => {
                    operation_diagnostics.push(catalog_diagnostic("bindings", source));
                    None
                }
            };
        }

        let mut center_updated = Vec::new();
        if let (Some(centers), Some(bindings)) =
            (center_snapshots.as_mut(), current_bindings.as_ref())
        {
            let associated_skill_ids: HashSet<SkillId> = bindings
                .iter()
                .map(|binding| binding.key.skill_id)
                .collect();
            let mut candidate_indices: HashMap<SkillId, Vec<usize>> = HashMap::new();
            for (index, candidate) in matched_local.iter().enumerate() {
                if associated_skill_ids.contains(&candidate.skill_id) {
                    candidate_indices
                        .entry(candidate.skill_id)
                        .or_default()
                        .push(index);
                }
            }

            let mut skill_ids: Vec<SkillId> = candidate_indices.keys().copied().collect();
            skill_ids.sort_by_key(|skill_id| skill_id.to_string());
            for skill_id in skill_ids {
                let Some(center) = centers
                    .iter()
                    .find(|snapshot| snapshot.installed.id == skill_id)
                    .cloned()
                else {
                    continue;
                };
                let Some(indices) = candidate_indices.get(&skill_id) else {
                    continue;
                };
                let candidates: Vec<(usize, LocalCandidate)> = indices
                    .iter()
                    .filter_map(|index| {
                        let candidate = &matched_local[*index];
                        (candidate.scanned.content_hash != center.version.content_hash).then(|| {
                            (
                                *index,
                                LocalCandidate {
                                    path: candidate.scanned.path.clone(),
                                    version: SkillVersion {
                                        content_hash: candidate.scanned.content_hash,
                                        marker_modified_at: candidate.scanned.marker_modified_at,
                                    },
                                },
                            )
                        })
                    })
                    .collect();
                let local_candidates: Vec<LocalCandidate> = candidates
                    .iter()
                    .map(|(_, candidate)| candidate.clone())
                    .collect();
                let Some(selected_index) = choose_newest_local(&local_candidates, &center.version)
                else {
                    continue;
                };
                let Some((source_index, _)) = candidates.get(selected_index) else {
                    continue;
                };
                let scanned = &matched_local[*source_index].scanned;
                match self.catalog.update_from_local(&skill_id, scanned) {
                    Ok(updated) => {
                        if let Some(current) = centers
                            .iter_mut()
                            .find(|snapshot| snapshot.installed.id == skill_id)
                        {
                            *current = updated;
                        }
                        center_updated.push(skill_id);
                    }
                    Err(source) => {
                        operation_diagnostics.push(catalog_path_diagnostic(
                            scanned.path.clone(),
                            "update_from_local",
                            source,
                        ));
                    }
                }
            }
        }

        let mut propagated = Vec::new();
        if let (Some(centers), Some(bindings)) =
            (center_snapshots.as_ref(), current_bindings.as_ref())
        {
            let mut bindings = bindings.clone();
            bindings.sort_by(|left, right| {
                compare_deployment_keys(&left.key, &right.key)
                    .then_with(|| compare_paths(&left.target_path, &right.target_path))
            });

            for binding in bindings {
                let Some(center) = centers
                    .iter()
                    .find(|snapshot| snapshot.installed.id == binding.key.skill_id)
                else {
                    continue;
                };
                let target = match target_for_binding(workspace, &resolution, &binding) {
                    Ok(target) => target,
                    Err(error) => {
                        operation_diagnostics.push(WorkspaceDiagnostic {
                            path: binding.target_path.clone(),
                            status: DeploymentStatus::Error,
                            error,
                        });
                        continue;
                    }
                };
                if scan_failures
                    .iter()
                    .any(|failure| failure.affects(&binding, target))
                    || match_failures.contains(&normalized_path_key(&binding.target_path))
                {
                    continue;
                }

                let target_matches_center =
                    local_candidate_for_binding(&matched_local, &binding, target).is_some_and(
                        |candidate| candidate.scanned.content_hash == center.version.content_hash,
                    );
                if target_matches_center {
                    continue;
                }

                match self.local.deploy(
                    &center.installed.location,
                    &binding.target_path,
                    binding.deployment_mode,
                ) {
                    Ok(_) => {
                        propagated.push(binding.key.clone());
                        if let Err(source) = self.catalog.associate(binding.clone()) {
                            operation_diagnostics.push(catalog_path_diagnostic(
                                binding.target_path,
                                "associate",
                                source,
                            ));
                        }
                    }
                    Err(source) => {
                        operation_diagnostics.push(WorkspaceDiagnostic {
                            path: binding.target_path.clone(),
                            status: DeploymentStatus::Error,
                            error: WorkspaceError::ReconcileFailed {
                                key: binding.key,
                                source: Box::new(WorkspaceError::Local(source)),
                            },
                        });
                    }
                }
            }
        }

        let mut final_report = self.observe(workspace, harnesses, environment, deployment_mode)?;
        for diagnostic in operation_diagnostics {
            if !final_report
                .diagnostics
                .iter()
                .any(|existing| same_diagnostic(existing, &diagnostic))
            {
                final_report.diagnostics.push(diagnostic);
            }
        }
        sort_diagnostics(&mut final_report.diagnostics);
        propagated.sort_by(compare_deployment_keys);

        Ok(ReconcileReport {
            workspace_id: workspace.id,
            imported,
            center_updated,
            propagated,
            final_report,
        })
    }

    fn scan_inventory(
        &self,
        resolution: &WorkspaceResolution,
        diagnostics: &mut Vec<WorkspaceDiagnostic>,
    ) -> ScanInventory {
        let mut scanned_roots = HashSet::new();
        let mut inventory = Vec::new();
        let mut failures = Vec::new();

        for target in &resolution.targets {
            if scanned_roots.insert(normalized_path_key(&target.path)) {
                self.scan_root(
                    &target.path,
                    target.scan_mode,
                    Some(target),
                    &mut inventory,
                    &mut failures,
                    diagnostics,
                );
            }
        }
        for root in &resolution.discovery_roots {
            if scanned_roots.insert(normalized_path_key(&root.path)) {
                self.scan_root(
                    &root.path,
                    root.scan_mode,
                    None,
                    &mut inventory,
                    &mut failures,
                    diagnostics,
                );
            }
        }

        ScanInventory {
            skills: inventory,
            failures,
        }
    }

    fn scan_root(
        &self,
        root: &Path,
        mode: ScanMode,
        target: Option<&WorkspaceTarget>,
        inventory: &mut Vec<ScannedInventory>,
        failures: &mut Vec<ScanFailure>,
        diagnostics: &mut Vec<WorkspaceDiagnostic>,
    ) {
        let report = match self.local.scan(root, mode) {
            Ok(report) => report,
            Err(LocalError::PathNotFound { path }) if path == root => return,
            Err(error) => {
                failures.push(ScanFailure {
                    root: root.to_path_buf(),
                    path: None,
                });
                diagnostics.push(WorkspaceDiagnostic {
                    path: root.to_path_buf(),
                    status: DeploymentStatus::Error,
                    error: WorkspaceError::Local(error),
                });
                return;
            }
        };

        for diagnostic in report.diagnostics {
            let diagnostic_path = diagnostic.path.clone();
            failures.push(ScanFailure {
                root: root.to_path_buf(),
                path: Some(diagnostic_path),
            });
            diagnostics.push(WorkspaceDiagnostic {
                path: diagnostic.path,
                status: DeploymentStatus::Error,
                error: WorkspaceError::Local(diagnostic.error),
            });
        }
        for scanned in report.skills {
            add_scanned_skill(inventory, scanned, target);
        }
    }
}

#[derive(Debug)]
struct ScanInventory {
    skills: Vec<ScannedInventory>,
    failures: Vec<ScanFailure>,
}

#[derive(Debug)]
struct ScanFailure {
    root: PathBuf,
    path: Option<PathBuf>,
}

impl ScanFailure {
    fn affects(&self, binding: &DeploymentBinding, target: &WorkspaceTarget) -> bool {
        match &self.path {
            Some(path) => normalized_path_key(path) == normalized_path_key(&binding.target_path),
            None => normalized_path_key(&self.root) == normalized_path_key(&target.path),
        }
    }
}

#[derive(Debug)]
struct ScannedInventory {
    scanned: ScannedSkill,
    target: Option<WorkspaceTarget>,
}

#[derive(Debug)]
struct MatchedLocalSkill {
    skill_id: SkillId,
    scanned: ScannedSkill,
    target: Option<WorkspaceTarget>,
}

fn add_scanned_skill(
    inventory: &mut Vec<ScannedInventory>,
    scanned: ScannedSkill,
    target: Option<&WorkspaceTarget>,
) {
    if let Some(existing) = inventory.iter_mut().find(|existing| {
        normalized_path_key(&existing.scanned.path) == normalized_path_key(&scanned.path)
    }) {
        if existing.target.is_none() {
            existing.target = target.cloned();
        }
        return;
    }

    inventory.push(ScannedInventory {
        scanned,
        target: target.cloned(),
    });
}

fn catalog_diagnostic(operation: &'static str, source: CatalogFailure) -> WorkspaceDiagnostic {
    catalog_path_diagnostic(PathBuf::new(), operation, source)
}

fn catalog_path_diagnostic(
    path: PathBuf,
    operation: &'static str,
    source: CatalogFailure,
) -> WorkspaceDiagnostic {
    WorkspaceDiagnostic {
        path,
        status: DeploymentStatus::Error,
        error: WorkspaceError::Catalog { operation, source },
    }
}

fn same_diagnostic(left: &WorkspaceDiagnostic, right: &WorkspaceDiagnostic) -> bool {
    left.path == right.path
        && left.status == right.status
        && left.error.to_string() == right.error.to_string()
}

fn target_for_binding<'a>(
    workspace: &Workspace,
    resolution: &'a WorkspaceResolution,
    binding: &DeploymentBinding,
) -> Result<&'a WorkspaceTarget, WorkspaceError> {
    let normalized_target_path = normalize_binding_target(&binding.target_path)?;
    if binding.key.workspace_id != workspace.id {
        return Err(WorkspaceError::InvalidTarget {
            path: binding.target_path.clone(),
            reason: BINDING_WORKSPACE_REASON,
        });
    }

    let target = match &workspace.kind {
        WorkspaceKind::Agents | WorkspaceKind::Project { .. } => resolution
            .targets
            .iter()
            .filter(|target| {
                target.workspace_id == binding.key.workspace_id
                    && target.harness_id == binding.key.harness_id
                    && target.role == TargetRole::Primary
            })
            .find(|target| path_is_strict_descendant(&target.path, &normalized_target_path)),
        WorkspaceKind::Linked { .. } => resolution
            .targets
            .iter()
            .filter(|target| {
                target.workspace_id == binding.key.workspace_id
                    && target.harness_id == binding.key.harness_id
            })
            .find(|target| path_is_strict_descendant(&target.path, &normalized_target_path)),
    };

    let target = target.ok_or_else(|| WorkspaceError::InvalidTarget {
        path: binding.target_path.clone(),
        reason: BINDING_OUTSIDE_TARGET_REASON,
    })?;
    validate_target_path(workspace, target, &normalized_target_path)?;
    Ok(target)
}

fn validate_target_path(
    workspace: &Workspace,
    target: &WorkspaceTarget,
    path: &Path,
) -> Result<(), WorkspaceError> {
    let normalized_path = normalize_binding_target(path)?;
    if !path_is_strict_descendant(&target.path, &normalized_path) {
        return Err(WorkspaceError::InvalidTarget {
            path: path.to_path_buf(),
            reason: BINDING_OUTSIDE_TARGET_REASON,
        });
    }
    let safety_root = match &workspace.kind {
        WorkspaceKind::Project { root } => root.as_path(),
        WorkspaceKind::Agents | WorkspaceKind::Linked { .. } => target.path.as_path(),
    };
    validate_existing_parent_chain(safety_root, &normalized_path)
}

fn validate_existing_parent_chain(root: &Path, target: &Path) -> Result<(), WorkspaceError> {
    let Some(parent) = target.parent() else {
        return Err(WorkspaceError::InvalidTarget {
            path: target.to_path_buf(),
            reason: BINDING_OUTSIDE_TARGET_REASON,
        });
    };
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| WorkspaceError::InvalidTarget {
            path: target.to_path_buf(),
            reason: BINDING_OUTSIDE_TARGET_REASON,
        })?;
    let mut current = root.to_path_buf();
    inspect_parent_component(&current, target)?;
    for component in relative.components() {
        if let Component::Normal(component) = component {
            current.push(component);
            inspect_parent_component(&current, target)?;
        }
    }
    Ok(())
}

fn inspect_parent_component(path: &Path, target: &Path) -> Result<(), WorkspaceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(WorkspaceError::Local(LocalError::Io {
                path: path.to_path_buf(),
                source,
            }))
        }
    };
    if metadata_is_link(&metadata) {
        return Err(WorkspaceError::InvalidTarget {
            path: target.to_path_buf(),
            reason: BINDING_LINKED_PARENT_REASON,
        });
    }
    if !metadata.is_dir() {
        return Err(WorkspaceError::InvalidTarget {
            path: target.to_path_buf(),
            reason: BINDING_OUTSIDE_TARGET_REASON,
        });
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_link(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_link(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn local_candidate_for_binding<'a>(
    candidates: &'a [MatchedLocalSkill],
    binding: &DeploymentBinding,
    target: &WorkspaceTarget,
) -> Option<&'a MatchedLocalSkill> {
    candidates.iter().find(|candidate| {
        candidate.skill_id == binding.key.skill_id
            && normalized_path_key(&candidate.scanned.path)
                == normalized_path_key(&binding.target_path)
            && candidate.target.as_ref().is_some_and(|source| {
                source.workspace_id == binding.key.workspace_id
                    && source.harness_id == binding.key.harness_id
                    && normalized_path_key(&source.path) == normalized_path_key(&target.path)
            })
    })
}

fn normalize_binding_target(path: &Path) -> Result<PathBuf, WorkspaceError> {
    lexical_normalize_path(path).map_err(|_| WorkspaceError::InvalidTarget {
        path: path.to_path_buf(),
        reason: BINDING_PARENT_DIR_REASON,
    })
}

fn lexical_normalize_path(path: &Path) -> Result<PathBuf, ()> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return Err(()),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    Ok(normalized)
}

fn path_is_strict_descendant(root: &Path, path: &Path) -> bool {
    let Ok(normalized_root) = lexical_normalize_path(root) else {
        return false;
    };
    let Ok(normalized_path) = lexical_normalize_path(path) else {
        return false;
    };

    path_key(&normalized_path).is_strict_descendant_of(&path_key(&normalized_root))
}

fn sort_observations(observations: &mut [DeploymentObservation]) {
    observations.sort_by(|left, right| {
        compare_deployment_keys(&left.key, &right.key)
            .then_with(|| compare_paths(&left.target_path, &right.target_path))
            .then_with(|| target_role_rank(left.role).cmp(&target_role_rank(right.role)))
    });
}

fn sort_unmatched_local(unmatched: &mut [UnmatchedLocalSkill]) {
    unmatched.sort_by(|left, right| {
        compare_paths(&left.scanned.path, &right.scanned.path)
            .then_with(|| compare_optional_targets(left.target.as_ref(), right.target.as_ref()))
    });
}

fn sort_diagnostics(diagnostics: &mut [WorkspaceDiagnostic]) {
    diagnostics.sort_by(|left, right| {
        compare_paths(&left.path, &right.path)
            .then_with(|| status_sort_rank(left.status).cmp(&status_sort_rank(right.status)))
            .then_with(|| left.error.to_string().cmp(&right.error.to_string()))
    });
}

fn compare_deployment_keys(left: &DeploymentKey, right: &DeploymentKey) -> Ordering {
    left.skill_id
        .to_string()
        .cmp(&right.skill_id.to_string())
        .then_with(|| left.harness_id.as_str().cmp(right.harness_id.as_str()))
        .then_with(|| {
            left.workspace_id
                .to_string()
                .cmp(&right.workspace_id.to_string())
        })
}

fn compare_optional_targets(
    left: Option<&WorkspaceTarget>,
    right: Option<&WorkspaceTarget>,
) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left
            .workspace_id
            .to_string()
            .cmp(&right.workspace_id.to_string())
            .then_with(|| left.harness_id.as_str().cmp(right.harness_id.as_str()))
            .then_with(|| compare_paths(&left.path, &right.path))
            .then_with(|| target_role_rank(left.role).cmp(&target_role_rank(right.role))),
    }
}

fn target_role_rank(role: TargetRole) -> u8 {
    match role {
        TargetRole::Primary => 0,
        TargetRole::Disabled => 1,
    }
}

fn status_sort_rank(status: DeploymentStatus) -> u8 {
    match status {
        DeploymentStatus::Unsupported => 0,
        DeploymentStatus::Error => 1,
        DeploymentStatus::Missing => 2,
        DeploymentStatus::CenterNewer => 3,
        DeploymentStatus::LocalNewer => 4,
        DeploymentStatus::InSync => 5,
        DeploymentStatus::NotDeployed => 6,
    }
}

fn resolve_agents(
    workspace_id: WorkspaceId,
    harnesses: &HarnessRegistry,
    environment: &HarnessEnvironment,
    deployment_mode: DeploymentMode,
) -> Result<WorkspaceResolution, WorkspaceError> {
    let mut resolution = empty_resolution();

    for adapter in harnesses.adapters() {
        let detection = adapter.detect(environment)?;
        if !detection.is_installed() {
            continue;
        }

        let locations = adapter.resolve_locations(environment, None)?;
        let capabilities = adapter.capabilities();
        let scan_mode = global_scan_mode(capabilities.recursive_global_discovery);

        if capabilities.supports_global_scope {
            resolution.targets.push(WorkspaceTarget {
                workspace_id,
                harness_id: adapter.id().clone(),
                path: locations.global_skills_dir,
                role: TargetRole::Primary,
                scan_mode,
                deployment_mode,
            });
        }

        for path in locations.additional_global_discovery_dirs {
            add_discovery_root(&mut resolution, path, scan_mode);
        }
    }

    Ok(resolution)
}

fn resolve_project(
    workspace_id: WorkspaceId,
    project_root: &Path,
    harnesses: &HarnessRegistry,
    environment: &HarnessEnvironment,
    deployment_mode: DeploymentMode,
) -> Result<WorkspaceResolution, WorkspaceError> {
    let mut resolution = empty_resolution();

    for adapter in harnesses.adapters() {
        let detection = adapter.detect(environment)?;
        if !detection.is_installed() {
            continue;
        }

        let capabilities = adapter.capabilities();
        if !capabilities.supports_project_scope {
            add_unsupported(&mut resolution, adapter.id().clone(), project_root);
            continue;
        }

        let locations = adapter.resolve_locations(environment, Some(project_root))?;
        let Some(path) = locations.project_skills_dir else {
            add_unsupported(&mut resolution, adapter.id().clone(), project_root);
            continue;
        };

        resolution.targets.push(WorkspaceTarget {
            workspace_id,
            harness_id: adapter.id().clone(),
            path,
            role: TargetRole::Primary,
            scan_mode: ScanMode::Recursive,
            deployment_mode,
        });
    }

    Ok(resolution)
}

fn resolve_linked(
    workspace_id: WorkspaceId,
    root: &Path,
    disabled_root: Option<&Path>,
    deployment_mode: DeploymentMode,
) -> Result<WorkspaceResolution, WorkspaceError> {
    let harness_id =
        HarnessId::new(&format!("linked-{workspace_id}")).map_err(HarnessError::from)?;
    let root = normalize_linked_root(root)?;
    let disabled_root = disabled_root.map(normalize_linked_root).transpose()?;

    if let Some(disabled) = disabled_root.as_ref() {
        let root_key = path_key(&root);
        let disabled_key = path_key(disabled);
        if root_key.is_descendant_or_equal_of(&disabled_key)
            || disabled_key.is_descendant_or_equal_of(&root_key)
        {
            return Err(WorkspaceError::InvalidWorkspace {
                reason: OVERLAPPING_LINKED_ROOTS_REASON,
            });
        }
    }

    let mut targets = vec![WorkspaceTarget {
        workspace_id,
        harness_id: harness_id.clone(),
        path: root,
        role: TargetRole::Primary,
        scan_mode: ScanMode::Recursive,
        deployment_mode,
    }];

    if let Some(path) = disabled_root {
        targets.push(WorkspaceTarget {
            workspace_id,
            harness_id,
            path,
            role: TargetRole::Disabled,
            scan_mode: ScanMode::Recursive,
            deployment_mode,
        });
    }

    Ok(WorkspaceResolution {
        targets,
        discovery_roots: Vec::new(),
        unsupported: Vec::new(),
    })
}

fn normalize_linked_root(path: &Path) -> Result<PathBuf, WorkspaceError> {
    if path.as_os_str().is_empty() {
        return Err(WorkspaceError::InvalidTarget {
            path: path.to_path_buf(),
            reason: EMPTY_LINKED_ROOT_REASON,
        });
    }

    let normalized = lexical_normalize_path(path).map_err(|_| WorkspaceError::InvalidTarget {
        path: path.to_path_buf(),
        reason: PARENT_LINKED_ROOT_REASON,
    })?;

    if normalized.as_os_str().is_empty() {
        return Err(WorkspaceError::InvalidTarget {
            path: path.to_path_buf(),
            reason: EMPTY_LINKED_ROOT_REASON,
        });
    }

    Ok(normalized)
}

fn global_scan_mode(recursive: bool) -> ScanMode {
    if recursive {
        ScanMode::Recursive
    } else {
        ScanMode::Flat
    }
}

fn add_discovery_root(resolution: &mut WorkspaceResolution, path: PathBuf, scan_mode: ScanMode) {
    if resolution
        .discovery_roots
        .iter()
        .any(|root| normalized_path_key(&root.path) == normalized_path_key(&path))
    {
        return;
    }

    resolution
        .discovery_roots
        .push(DiscoveryRoot { path, scan_mode });
}

fn add_unsupported(
    resolution: &mut WorkspaceResolution,
    harness_id: HarnessId,
    project_root: &Path,
) {
    resolution.unsupported.push(UnsupportedWorkspaceTarget {
        harness_id,
        path: project_root.to_path_buf(),
        reason: PROJECT_SCOPE_UNSUPPORTED_REASON,
    });
}

fn empty_resolution() -> WorkspaceResolution {
    WorkspaceResolution {
        targets: Vec::new(),
        discovery_roots: Vec::new(),
        unsupported: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn physical_target_boundary_accepts_regular_parent_chain() {
        let root = tempdir().unwrap();
        let target_root = root.path().join("workspace");
        let nested = target_root.join("nested");
        fs::create_dir_all(&nested).unwrap();

        validate_existing_parent_chain(&target_root, &nested.join("skill")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn physical_target_boundary_rejects_linked_parent() {
        let root = tempdir().unwrap();
        let external = root.path().join("external");
        let linked = root.path().join("linked");
        fs::create_dir_all(&external).unwrap();
        std::os::unix::fs::symlink(&external, &linked).unwrap();

        let error = validate_existing_parent_chain(&linked, &linked.join("skill")).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceError::InvalidTarget {
                reason: BINDING_LINKED_PARENT_REASON,
                ..
            }
        ));
    }
}
