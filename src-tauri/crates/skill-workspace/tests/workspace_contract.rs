use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use skill_core::{ContentHash, InstalledSkill, SkillId, SkillSource};
use skill_harness::{
    CustomHarnessDefinition, HarnessAdapter, HarnessCategory, HarnessEnvironment, HarnessRegistry,
};
use skill_local::{LocalError, OperationResult, ScanMode, ScanReport, ScannedSkill};
use skill_workspace::*;
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

fn write_skill(path: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Description for {name}\n---\n\n# {name}\n"),
    )
    .unwrap();
    path.to_path_buf()
}

#[allow(dead_code)]
struct TestFixture {
    _temp: TempDir,
    workspace: Workspace,
    harnesses: HarnessRegistry,
    environment: HarnessEnvironment,
    center_root: PathBuf,
    target_root: PathBuf,
    disabled_root: Option<PathBuf>,
    center_skill: Option<PathBuf>,
    target_skills: Vec<PathBuf>,
    original_skill_path: PathBuf,
}

impl TestFixture {
    fn first_target(&self) -> PathBuf {
        self.target_skills[0].clone()
    }
}

fn new_fixture(
    temp: TempDir,
    target_root: PathBuf,
    disabled_root: Option<PathBuf>,
    center_skill: Option<PathBuf>,
    target_skills: Vec<PathBuf>,
) -> TestFixture {
    let original_skill_path = target_skills[0].clone();
    TestFixture {
        _temp: temp,
        workspace: Workspace {
            id: WorkspaceId::from_uuid(Uuid::from_u128(100)),
            kind: WorkspaceKind::Linked {
                root: target_root.clone(),
                disabled_root: disabled_root.clone(),
            },
        },
        harnesses: HarnessRegistry::with_builtins(),
        environment: HarnessEnvironment::new(
            target_root
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("home"),
            None,
        ),
        center_root: center_skill
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| target_root.parent().unwrap().join("center")),
        target_root,
        disabled_root,
        center_skill,
        target_skills,
        original_skill_path,
    }
}

fn fixture_with_matching_center_and_target() -> TestFixture {
    let temp = tempdir().unwrap();
    let center_root = temp.path().join("center");
    let target_root = temp.path().join("target");
    let center_skill = write_skill(&center_root.join("matching"), "matching");
    let target_skill = write_skill(&target_root.join("matching"), "matching");

    let mut fixture = new_fixture(
        temp,
        target_root,
        None,
        Some(center_skill),
        vec![target_skill],
    );
    fixture.center_root = center_root;
    fixture
}

#[allow(dead_code)]
fn fixture_with_unmatched_local_skill() -> TestFixture {
    let temp = tempdir().unwrap();
    let center_root = temp.path().join("center");
    let target_root = temp.path().join("target");
    fs::create_dir_all(&center_root).unwrap();
    let target_skill = write_skill(&target_root.join("unmatched"), "unmatched");

    let mut fixture = new_fixture(temp, target_root, None, None, vec![target_skill]);
    fixture.center_root = center_root;
    fixture
}

fn fixture_with_duplicate_unmatched_local_skills() -> TestFixture {
    let temp = tempdir().unwrap();
    let center_root = temp.path().join("center");
    let target_root = temp.path().join("target");
    let disabled_root = temp.path().join("disabled");
    fs::create_dir_all(&center_root).unwrap();
    let enabled_skill = write_skill(&target_root.join("shared"), "shared");
    let disabled_skill = write_skill(&disabled_root.join("shared"), "shared");

    let mut fixture = new_fixture(
        temp,
        target_root,
        Some(disabled_root),
        None,
        vec![enabled_skill, disabled_skill],
    );
    fixture.center_root = center_root;
    fixture
}

fn fixture_with_center_newer_skill_and_two_targets() -> TestFixture {
    let temp = tempdir().unwrap();
    let center_root = temp.path().join("center");
    let target_root = temp.path().join("target");
    let disabled_root = temp.path().join("disabled");
    let center_skill = write_skill(&center_root.join("shared"), "old");
    let enabled_skill = write_skill(&target_root.join("shared"), "old");
    let disabled_skill = write_skill(&disabled_root.join("shared"), "old");

    std::thread::sleep(Duration::from_millis(25));
    write_skill(&center_skill, "new");

    let mut fixture = new_fixture(
        temp,
        target_root,
        Some(disabled_root),
        Some(center_skill),
        vec![enabled_skill, disabled_skill],
    );
    fixture.center_root = center_root;
    fixture
}

#[allow(dead_code)]
fn fixture_with_two_local_newer_copies() -> TestFixture {
    let temp = tempdir().unwrap();
    let center_root = temp.path().join("center");
    let target_root = temp.path().join("target");
    let disabled_root = temp.path().join("disabled");
    let center_skill = write_skill(&center_root.join("shared"), "old");

    std::thread::sleep(Duration::from_millis(25));
    let enabled_skill = write_skill(&target_root.join("shared"), "enabled-new");
    let disabled_skill = write_skill(&disabled_root.join("shared"), "disabled-new");

    let mut fixture = new_fixture(
        temp,
        target_root,
        Some(disabled_root),
        Some(center_skill),
        vec![enabled_skill, disabled_skill],
    );
    fixture.center_root = center_root;
    fixture
}

#[derive(Clone, Default)]
struct RecordingLocal {
    inner: SystemLocalSkillPort,
    deploy_calls: Arc<Mutex<Vec<PathBuf>>>,
    successful_deploy_calls: Arc<Mutex<Vec<PathBuf>>>,
    failing_target: Option<PathBuf>,
    failing_scan_root: Option<PathBuf>,
    without_marker_time: bool,
}

impl RecordingLocal {
    #[allow(dead_code)]
    fn failing(target: PathBuf) -> Self {
        Self {
            failing_target: Some(target),
            ..Self::default()
        }
    }

    fn without_marker_time() -> Self {
        Self {
            without_marker_time: true,
            ..Self::default()
        }
    }

    fn failing_scan(root: PathBuf) -> Self {
        Self {
            failing_scan_root: Some(root),
            ..Self::default()
        }
    }

    fn deploy_calls(&self) -> Vec<PathBuf> {
        self.deploy_calls.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    fn successful_deploy_calls(&self) -> Vec<PathBuf> {
        self.successful_deploy_calls.lock().unwrap().clone()
    }

    fn remove_marker_time(&self, mut report: ScanReport) -> ScanReport {
        if self.without_marker_time {
            for skill in &mut report.skills {
                skill.marker_modified_at = None;
            }
        }
        report
    }

    fn remove_marker_time_from_skill(&self, mut skill: ScannedSkill) -> ScannedSkill {
        if self.without_marker_time {
            skill.marker_modified_at = None;
        }
        skill
    }
}

impl LocalSkillPort for RecordingLocal {
    fn scan(&self, root: &Path, mode: ScanMode) -> Result<ScanReport, LocalError> {
        if self.failing_scan_root.as_deref() == Some(root) {
            return Err(LocalError::Io {
                path: root.to_path_buf(),
                source: std::io::Error::other("recorded scan failure"),
            });
        }

        self.inner
            .scan(root, mode)
            .map(|report| self.remove_marker_time(report))
    }

    fn read(&self, path: &Path) -> Result<ScannedSkill, LocalError> {
        self.inner
            .read(path)
            .map(|skill| self.remove_marker_time_from_skill(skill))
    }

    fn deploy(
        &self,
        source: &Path,
        target: &Path,
        mode: DeploymentMode,
    ) -> Result<OperationResult, LocalError> {
        self.deploy_calls.lock().unwrap().push(target.to_path_buf());
        if self.failing_target.as_deref() == Some(target) {
            return Err(LocalError::Io {
                path: target.to_path_buf(),
                source: std::io::Error::other("recorded failure"),
            });
        }

        let result = self.inner.deploy(source, target, mode)?;
        self.successful_deploy_calls
            .lock()
            .unwrap()
            .push(target.to_path_buf());
        Ok(result)
    }

    fn delete(&self, target: &Path) -> Result<OperationResult, LocalError> {
        self.inner.delete(target)
    }
}

struct CatalogState {
    snapshots: Vec<CentralSkillSnapshot>,
    bindings: Vec<DeploymentBinding>,
    import_calls: Vec<PathBuf>,
    update_calls: Vec<SkillId>,
    associate_calls: Vec<DeploymentBinding>,
}

#[derive(Clone)]
struct FakeCatalog {
    state: Arc<Mutex<CatalogState>>,
    center_root: PathBuf,
}

impl FakeCatalog {
    fn empty(center_root: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(CatalogState {
                snapshots: Vec::new(),
                bindings: Vec::new(),
                import_calls: Vec::new(),
                update_calls: Vec::new(),
                associate_calls: Vec::new(),
            })),
            center_root,
        }
    }

    fn with_center_and_binding(fixture: &TestFixture) -> Self {
        Self::with_center_and_binding_at(fixture, fixture.first_target())
    }

    fn with_center_and_binding_at(fixture: &TestFixture, target_path: PathBuf) -> Self {
        let catalog = Self::empty(fixture.center_root.clone());
        let center = center_snapshot(fixture);
        let target_resolution = resolve_workspace(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();
        let target = target_resolution.targets[0].clone();
        let binding = DeploymentBinding {
            key: DeploymentKey {
                skill_id: center.installed.id,
                harness_id: target.harness_id,
                workspace_id: fixture.workspace.id,
            },
            target_path,
            deployment_mode: DeploymentMode::Copy,
        };
        catalog.set_center_and_bindings(vec![binding], center);
        catalog
    }

    fn with_center_and_bindings(fixture: &TestFixture) -> Self {
        let catalog = Self::empty(fixture.center_root.clone());
        let center = center_snapshot(fixture);
        let resolution = resolve_workspace(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();
        let bindings = fixture
            .target_skills
            .iter()
            .zip(resolution.targets.iter())
            .map(|(path, target)| DeploymentBinding {
                key: DeploymentKey {
                    skill_id: center.installed.id,
                    harness_id: target.harness_id.clone(),
                    workspace_id: fixture.workspace.id,
                },
                target_path: path.clone(),
                deployment_mode: DeploymentMode::Copy,
            })
            .collect();
        catalog.set_center_and_bindings(bindings, center);
        catalog
    }

    #[allow(dead_code)]
    fn with_ambiguous_hash(fixture: &TestFixture) -> Self {
        let catalog = Self::empty(fixture.center_root.clone());
        let center = center_snapshot(fixture);
        let mut second = center.clone();
        second.installed.id = SkillId::new();
        catalog.set_center_and_bindings(vec![], center);
        catalog.state.lock().unwrap().snapshots.push(second);
        catalog
    }

    fn set_center_and_bindings(
        &self,
        bindings: Vec<DeploymentBinding>,
        center: CentralSkillSnapshot,
    ) {
        let mut state = self.state.lock().unwrap();
        state.snapshots = vec![center];
        state.bindings = bindings;
    }

    fn import_calls(&self) -> Vec<PathBuf> {
        self.state.lock().unwrap().import_calls.clone()
    }

    fn update_calls(&self) -> Vec<SkillId> {
        self.state.lock().unwrap().update_calls.clone()
    }

    #[allow(dead_code)]
    fn associate_calls(&self) -> Vec<DeploymentBinding> {
        self.state.lock().unwrap().associate_calls.clone()
    }
}

fn scanned_version(scanned: &ScannedSkill) -> SkillVersion {
    let content_hash: ContentHash = scanned.content_hash;
    let marker_modified_at: Option<SystemTime> = scanned.marker_modified_at;
    SkillVersion {
        content_hash,
        marker_modified_at,
    }
}

fn center_snapshot(fixture: &TestFixture) -> CentralSkillSnapshot {
    let path = fixture.center_skill.as_ref().unwrap();
    let scanned = SystemLocalSkillPort.read(path).unwrap();
    CentralSkillSnapshot {
        installed: InstalledSkill {
            id: SkillId::new(),
            metadata: scanned.document.metadata().clone(),
            location: scanned.path.clone(),
            source: SkillSource::Local {
                path: scanned.path.clone(),
            },
            content_hash: scanned.content_hash,
        },
        version: scanned_version(&scanned),
    }
}

impl CentralCatalogPort for FakeCatalog {
    fn list(&self) -> Result<Vec<CentralSkillSnapshot>, CatalogFailure> {
        Ok(self.state.lock().unwrap().snapshots.clone())
    }

    fn bindings(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<DeploymentBinding>, CatalogFailure> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .bindings
            .iter()
            .filter(|binding| binding.key.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    fn resolve_match(
        &self,
        scanned: &ScannedSkill,
        target_path: &Path,
    ) -> Result<CentralMatch, CatalogFailure> {
        let state = self.state.lock().unwrap();
        if let Some(binding) = state
            .bindings
            .iter()
            .find(|binding| binding.target_path == target_path)
        {
            return Ok(CentralMatch::Unique(binding.key.skill_id));
        }

        let matches: Vec<SkillId> = state
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.version.content_hash == scanned.content_hash)
            .map(|snapshot| snapshot.installed.id)
            .collect();
        match matches.as_slice() {
            [] => Ok(CentralMatch::None),
            [skill_id] => Ok(CentralMatch::Unique(*skill_id)),
            _ => Ok(CentralMatch::Ambiguous(matches)),
        }
    }

    fn import_local(
        &mut self,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        let destination = self.center_root.join(
            scanned
                .path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("skill")),
        );
        skill_local::copy_skill(
            &scanned.path,
            &destination,
            skill_local::ExistingDestination::Replace,
        )
        .map_err(CatalogFailure::local_operation)?;
        let imported = SystemLocalSkillPort
            .read(&destination)
            .map_err(CatalogFailure::local_operation)?;
        let snapshot = CentralSkillSnapshot {
            installed: InstalledSkill {
                id: SkillId::new(),
                metadata: imported.document.metadata().clone(),
                location: imported.path.clone(),
                source: SkillSource::Local {
                    path: imported.path.clone(),
                },
                content_hash: imported.content_hash,
            },
            version: scanned_version(&imported),
        };
        let mut state = self.state.lock().unwrap();
        state.import_calls.push(scanned.path.clone());
        state.snapshots.push(snapshot.clone());
        Ok(snapshot)
    }

    fn update_from_local(
        &mut self,
        skill_id: &SkillId,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        let existing = self
            .state
            .lock()
            .unwrap()
            .snapshots
            .iter()
            .find(|snapshot| snapshot.installed.id == *skill_id)
            .cloned()
            .unwrap();
        skill_local::copy_skill(
            &scanned.path,
            &existing.installed.location,
            skill_local::ExistingDestination::Replace,
        )
        .map_err(CatalogFailure::local_operation)?;
        let updated = SystemLocalSkillPort
            .read(&existing.installed.location)
            .map_err(CatalogFailure::local_operation)?;
        let snapshot = CentralSkillSnapshot {
            installed: InstalledSkill {
                id: *skill_id,
                metadata: updated.document.metadata().clone(),
                location: updated.path.clone(),
                source: SkillSource::Local {
                    path: updated.path.clone(),
                },
                content_hash: updated.content_hash,
            },
            version: scanned_version(&updated),
        };
        let mut state = self.state.lock().unwrap();
        state.update_calls.push(*skill_id);
        if let Some(slot) = state
            .snapshots
            .iter_mut()
            .find(|current| current.installed.id == *skill_id)
        {
            *slot = snapshot.clone();
        }
        Ok(snapshot)
    }

    fn associate(&mut self, binding: DeploymentBinding) -> Result<(), CatalogFailure> {
        let mut state = self.state.lock().unwrap();
        if !state.associate_calls.contains(&binding) {
            state.associate_calls.push(binding.clone());
        }
        if !state.bindings.contains(&binding) {
            state.bindings.push(binding);
        }
        Ok(())
    }
}

struct FailingCatalog {
    inner: FakeCatalog,
    fail_list: bool,
    fail_bindings: bool,
    fail_list_after_import: bool,
    fail_bindings_after_import: bool,
    imported: bool,
    fail_match: Option<PathBuf>,
    fail_associate_once: bool,
}

impl FailingCatalog {
    fn list(inner: FakeCatalog) -> Self {
        Self {
            inner,
            fail_list: true,
            fail_bindings: false,
            fail_list_after_import: false,
            fail_bindings_after_import: false,
            imported: false,
            fail_match: None,
            fail_associate_once: false,
        }
    }

    fn bindings(inner: FakeCatalog) -> Self {
        Self {
            inner,
            fail_list: false,
            fail_bindings: true,
            fail_list_after_import: false,
            fail_bindings_after_import: false,
            imported: false,
            fail_match: None,
            fail_associate_once: false,
        }
    }

    fn list_after_import(inner: FakeCatalog) -> Self {
        Self {
            inner,
            fail_list: false,
            fail_bindings: false,
            fail_list_after_import: true,
            fail_bindings_after_import: false,
            imported: false,
            fail_match: None,
            fail_associate_once: false,
        }
    }

    fn bindings_after_import(inner: FakeCatalog) -> Self {
        Self {
            inner,
            fail_list: false,
            fail_bindings: false,
            fail_list_after_import: false,
            fail_bindings_after_import: true,
            imported: false,
            fail_match: None,
            fail_associate_once: false,
        }
    }

    fn match_at(inner: FakeCatalog, path: PathBuf) -> Self {
        Self {
            inner,
            fail_list: false,
            fail_bindings: false,
            fail_list_after_import: false,
            fail_bindings_after_import: false,
            imported: false,
            fail_match: Some(path),
            fail_associate_once: false,
        }
    }

    fn associate_once(inner: FakeCatalog) -> Self {
        Self {
            inner,
            fail_list: false,
            fail_bindings: false,
            fail_list_after_import: false,
            fail_bindings_after_import: false,
            imported: false,
            fail_match: None,
            fail_associate_once: true,
        }
    }
}

impl CentralCatalogPort for FailingCatalog {
    fn list(&self) -> Result<Vec<CentralSkillSnapshot>, CatalogFailure> {
        if self.fail_list || (self.fail_list_after_import && self.imported) {
            return Err(CatalogFailure::storage(std::io::Error::other(
                "recorded list failure",
            )));
        }
        self.inner.list()
    }

    fn bindings(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<DeploymentBinding>, CatalogFailure> {
        if self.fail_bindings || (self.fail_bindings_after_import && self.imported) {
            return Err(CatalogFailure::storage(std::io::Error::other(
                "recorded bindings failure",
            )));
        }
        self.inner.bindings(workspace_id)
    }

    fn resolve_match(
        &self,
        scanned: &ScannedSkill,
        target_path: &Path,
    ) -> Result<CentralMatch, CatalogFailure> {
        if self.fail_match.as_deref() == Some(target_path) {
            return Err(CatalogFailure::storage(std::io::Error::other(
                "recorded match failure",
            )));
        }
        self.inner.resolve_match(scanned, target_path)
    }

    fn import_local(
        &mut self,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        let result = self.inner.import_local(scanned);
        if result.is_ok() {
            self.imported = true;
        }
        result
    }

    fn update_from_local(
        &mut self,
        skill_id: &SkillId,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        self.inner.update_from_local(skill_id, scanned)
    }

    fn associate(&mut self, binding: DeploymentBinding) -> Result<(), CatalogFailure> {
        if self.fail_associate_once {
            self.fail_associate_once = false;
            return Err(CatalogFailure::conflict("recorded associate failure"));
        }

        self.inner.associate(binding)
    }
}

#[test]
fn agents_targets_are_independent_per_harness() {
    let root = tempdir().unwrap();
    let first_path = root.path().join("first/skills");
    let second_path = root.path().join("second/skills");
    fs::create_dir_all(&first_path).unwrap();
    fs::create_dir_all(&second_path).unwrap();

    let mut harnesses = HarnessRegistry::with_builtins();
    let first = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "first-test-harness".to_owned(),
        display_name: "First test harness".to_owned(),
        global_skills_path: first_path.to_string_lossy().into_owned(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .unwrap();
    let second = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "second-test-harness".to_owned(),
        display_name: "Second test harness".to_owned(),
        global_skills_path: second_path.to_string_lossy().into_owned(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .unwrap();
    harnesses.register(first).unwrap();
    harnesses.register(second).unwrap();

    let workspace_id = WorkspaceId::from_uuid(Uuid::from_u128(2));
    let workspace = Workspace {
        id: workspace_id,
        kind: WorkspaceKind::Agents,
    };
    let environment = HarnessEnvironment::new(root.path().join("home"), None);
    let resolution =
        resolve_workspace(&workspace, &harnesses, &environment, DeploymentMode::Copy).unwrap();
    let skill_id = SkillId::new();
    let keys: Vec<_> = resolution
        .targets
        .iter()
        .map(|target| DeploymentKey {
            skill_id,
            harness_id: target.harness_id.clone(),
            workspace_id,
        })
        .collect();

    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
    assert_ne!(resolution.targets[0].path, resolution.targets[1].path);
}

#[test]
fn project_without_project_scope_is_unsupported() {
    let root = tempdir().unwrap();
    let mut harnesses = HarnessRegistry::with_builtins();
    let adapter = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "projectless-test-harness".to_owned(),
        display_name: "Projectless test harness".to_owned(),
        global_skills_path: root.path().join("global").to_string_lossy().into_owned(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .unwrap();
    let harness_id = adapter.id().clone();
    harnesses.register(adapter).unwrap();

    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(3)),
        kind: WorkspaceKind::Project {
            root: root.path().join("project"),
        },
    };
    let environment = HarnessEnvironment::new(root.path().join("home"), None);
    let resolution =
        resolve_workspace(&workspace, &harnesses, &environment, DeploymentMode::Copy).unwrap();

    assert!(!resolution
        .targets
        .iter()
        .any(|target| target.harness_id == harness_id));
    assert!(resolution
        .unsupported
        .iter()
        .any(|unsupported| unsupported.harness_id == harness_id));
}

#[test]
fn linked_disabled_root_is_propagated_without_becoming_enabled() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    let successful_deploys = local_probe.successful_deploy_calls();
    assert_eq!(successful_deploys.len(), 2);
    assert!(successful_deploys.contains(&fixture.target_skills[0]));
    assert!(successful_deploys.contains(&fixture.target_skills[1]));

    let enabled_observation = report
        .final_report
        .observations
        .iter()
        .find(|observation| observation.target_path == fixture.target_skills[0])
        .expect("enabled target observation");
    assert_eq!(enabled_observation.role, TargetRole::Primary);

    let disabled_observation = report
        .final_report
        .observations
        .iter()
        .find(|observation| observation.target_path == fixture.target_skills[1])
        .expect("disabled target observation");
    assert_eq!(disabled_observation.role, TargetRole::Disabled);
}

#[test]
fn binding_roots_are_rejected_without_deploying() {
    for index in 0..2 {
        let fixture = fixture_with_center_newer_skill_and_two_targets();
        let target_path = match index {
            0 => fixture.target_root.clone(),
            _ => fixture.target_root.join("."),
        };
        let local = RecordingLocal::default();
        let local_probe = local.clone();
        let catalog = FakeCatalog::with_center_and_binding_at(&fixture, target_path.clone());
        let mut engine = WorkspaceEngine::new(local, catalog);

        let report = engine
            .reconcile(
                &fixture.workspace,
                &fixture.harnesses,
                &fixture.environment,
                DeploymentMode::Copy,
            )
            .unwrap();

        assert!(
            local_probe.deploy_calls().is_empty(),
            "invalid binding target was deployed: {target_path:?}"
        );
        assert!(report.final_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.path == target_path
                && matches!(diagnostic.error, WorkspaceError::InvalidTarget { .. })
        }));
    }
}

#[test]
fn same_marker_time_uses_platform_path_order_and_is_input_order_independent() {
    let center = SkillVersion {
        content_hash: ContentHash::from_bytes([0; 32]),
        marker_modified_at: Some(SystemTime::UNIX_EPOCH),
    };
    let candidates = vec![
        LocalCandidate {
            path: PathBuf::from("a\\skill"),
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([1; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
        LocalCandidate {
            path: PathBuf::from("a0skill"),
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([2; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
    ];

    #[cfg(windows)]
    let expected = PathBuf::from("a\\skill");
    #[cfg(unix)]
    let expected = PathBuf::from("a0skill");

    let selected = choose_newest_local(&candidates, &center).unwrap();
    assert_eq!(candidates[selected].path, expected);

    let reversed = vec![candidates[1].clone(), candidates[0].clone()];
    let selected = choose_newest_local(&reversed, &center).unwrap();
    assert_eq!(reversed[selected].path, expected);
}

#[cfg(unix)]
#[test]
fn same_marker_time_uses_lossless_unix_path_key() {
    use std::os::unix::ffi::OsStringExt;

    let center = SkillVersion {
        content_hash: ContentHash::from_bytes([0; 32]),
        marker_modified_at: Some(SystemTime::UNIX_EPOCH),
    };
    let first_path = PathBuf::from(OsString::from_vec(b"skills/\xff".to_vec()));
    let second_path = PathBuf::from(OsString::from_vec(b"skills/\xfe".to_vec()));
    let candidates = vec![
        LocalCandidate {
            path: first_path.clone(),
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([1; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
        LocalCandidate {
            path: second_path.clone(),
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([2; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
    ];

    let selected = choose_newest_local(&candidates, &center).unwrap();
    assert_eq!(candidates[selected].path, second_path);

    let reversed = vec![candidates[1].clone(), candidates[0].clone()];
    let selected = choose_newest_local(&reversed, &center).unwrap();
    assert_eq!(reversed[selected].path, second_path);
}

#[cfg(windows)]
#[test]
fn same_marker_time_uses_lossless_windows_raw_tie_break() {
    let center = SkillVersion {
        content_hash: ContentHash::from_bytes([0; 32]),
        marker_modified_at: Some(SystemTime::UNIX_EPOCH),
    };
    let first_path = PathBuf::from(r"C:\Managed\skills");
    let second_path = PathBuf::from(r"c:\managed\SKILLS");
    let candidates = vec![
        LocalCandidate {
            path: first_path.clone(),
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([1; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
        LocalCandidate {
            path: second_path,
            version: SkillVersion {
                content_hash: ContentHash::from_bytes([2; 32]),
                marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
            },
        },
    ];

    let selected = choose_newest_local(&candidates, &center).unwrap();
    assert_eq!(candidates[selected].path, first_path);

    let reversed = vec![candidates[1].clone(), candidates[0].clone()];
    let selected = choose_newest_local(&reversed, &center).unwrap();
    assert_eq!(reversed[selected].path, first_path);
}

#[test]
fn missing_target_is_reported_as_missing_when_binding_exists() {
    let fixture = fixture_with_matching_center_and_target();
    fs::remove_dir_all(&fixture.target_skills[0]).unwrap();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::with_center_and_binding(&fixture);
    let engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(report
        .observations
        .iter()
        .any(|observation| observation.status == DeploymentStatus::Missing));
}

#[test]
fn ambiguous_catalog_match_is_error_diagnostic_not_conflict() {
    let fixture = fixture_with_matching_center_and_target();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::with_ambiguous_hash(&fixture);
    let engine = WorkspaceEngine::new(local, catalog.clone());

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.status == DeploymentStatus::Error
            && matches!(diagnostic.error, WorkspaceError::AmbiguousMatch { .. })
    }));
    assert!(catalog.update_calls().is_empty());
}

#[test]
fn observe_never_calls_catalog_write_methods() {
    let fixture = fixture_with_matching_center_and_target();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::with_center_and_binding(&fixture);
    let engine = WorkspaceEngine::new(local.clone(), catalog.clone());

    engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(catalog.import_calls().is_empty());
    assert!(catalog.update_calls().is_empty());
    assert!(catalog.associate_calls().is_empty());
    assert!(local.deploy_calls().is_empty());
}

#[test]
fn observe_reports_in_sync_without_importing_or_deploying() {
    let fixture = fixture_with_matching_center_and_target();
    let catalog = FakeCatalog::with_center_and_binding(&fixture);
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let engine = WorkspaceEngine::new(local, catalog.clone());

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.observations[0].status, DeploymentStatus::InSync);
    assert!(catalog.import_calls().is_empty());
    assert!(catalog.update_calls().is_empty());
    assert!(catalog.associate_calls().is_empty());
    assert!(local_probe.deploy_calls().is_empty());
}

#[test]
fn observe_prefers_center_when_local_marker_time_is_unavailable() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let local = RecordingLocal::without_marker_time();
    let engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(report
        .observations
        .iter()
        .all(|observation| observation.status == DeploymentStatus::CenterNewer));
}

#[test]
fn reconcile_fails_closed_when_initial_catalog_list_fails() {
    let fixture = fixture_with_unmatched_local_skill();
    let catalog_state = FakeCatalog::empty(fixture.center_root.clone());
    let catalog = FailingCatalog::list(catalog_state.clone());
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(catalog_state.import_calls().is_empty());
    assert!(catalog_state.update_calls().is_empty());
    assert!(catalog_state.associate_calls().is_empty());
    assert!(local_probe.deploy_calls().is_empty());
    assert!(report.final_report.diagnostics.iter().any(|diagnostic| {
        matches!(
            &diagnostic.error,
            WorkspaceError::Catalog {
                operation: "list",
                ..
            }
        )
    }));
}

#[test]
fn reconcile_fails_closed_when_initial_catalog_bindings_fail() {
    let fixture = fixture_with_unmatched_local_skill();
    let catalog_state = FakeCatalog::empty(fixture.center_root.clone());
    let catalog = FailingCatalog::bindings(catalog_state.clone());
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(catalog_state.import_calls().is_empty());
    assert!(catalog_state.update_calls().is_empty());
    assert!(catalog_state.associate_calls().is_empty());
    assert!(local_probe.deploy_calls().is_empty());
    assert!(report.final_report.diagnostics.iter().any(|diagnostic| {
        matches!(
            &diagnostic.error,
            WorkspaceError::Catalog {
                operation: "bindings",
                ..
            }
        )
    }));
}

#[test]
fn reconcile_fails_closed_after_import_when_catalog_list_refresh_fails() {
    let fixture = fixture_with_unmatched_local_skill();
    let catalog_state = FakeCatalog::empty(fixture.center_root.clone());
    let catalog = FailingCatalog::list_after_import(catalog_state.clone());
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.imported.len(), 1);
    assert_eq!(catalog_state.import_calls().len(), 1);
    assert!(catalog_state.update_calls().is_empty());
    assert!(catalog_state.associate_calls().is_empty());
    assert!(local_probe.deploy_calls().is_empty());
    assert!(report.final_report.diagnostics.iter().any(|diagnostic| {
        matches!(
            &diagnostic.error,
            WorkspaceError::Catalog {
                operation: "list",
                ..
            }
        )
    }));
}

#[test]
fn reconcile_fails_closed_after_import_when_catalog_bindings_refresh_fails() {
    let fixture = fixture_with_unmatched_local_skill();
    let catalog_state = FakeCatalog::empty(fixture.center_root.clone());
    let catalog = FailingCatalog::bindings_after_import(catalog_state.clone());
    let local = RecordingLocal::default();
    let local_probe = local.clone();
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.imported.len(), 1);
    assert_eq!(catalog_state.import_calls().len(), 1);
    assert!(catalog_state.update_calls().is_empty());
    assert!(catalog_state.associate_calls().is_empty());
    assert!(local_probe.deploy_calls().is_empty());
    assert!(report.final_report.diagnostics.iter().any(|diagnostic| {
        matches!(
            &diagnostic.error,
            WorkspaceError::Catalog {
                operation: "bindings",
                ..
            }
        )
    }));
}

#[test]
fn observe_marks_catalog_list_failure_as_error_not_missing() {
    let fixture = fixture_with_matching_center_and_target();
    let catalog = FailingCatalog::list(FakeCatalog::with_center_and_binding(&fixture));
    let engine = WorkspaceEngine::new(RecordingLocal::default(), catalog);

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.observations[0].status, DeploymentStatus::Error);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.status == DeploymentStatus::Error
            && matches!(
                &diagnostic.error,
                WorkspaceError::Catalog {
                    operation: "list",
                    ..
                }
            )
    }));
}

#[test]
fn observe_marks_scan_failure_as_error_not_missing() {
    let fixture = fixture_with_matching_center_and_target();
    let catalog = FakeCatalog::with_center_and_binding(&fixture);
    let local = RecordingLocal::failing_scan(fixture.target_root.clone());
    let engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.observations[0].status, DeploymentStatus::Error);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == fixture.target_root
            && diagnostic.status == DeploymentStatus::Error
            && matches!(&diagnostic.error, WorkspaceError::Local(_))
    }));
}

#[test]
fn observe_marks_match_failure_as_error_not_missing() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let target_path = fixture.first_target();
    let unaffected_path = fixture.target_skills[1].clone();
    let catalog = FailingCatalog::match_at(
        FakeCatalog::with_center_and_bindings(&fixture),
        target_path.clone(),
    );
    let engine = WorkspaceEngine::new(RecordingLocal::default(), catalog);

    let report = engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.observations.len(), 2);
    assert_eq!(
        report
            .observations
            .iter()
            .find(|observation| observation.target_path == target_path)
            .unwrap()
            .status,
        DeploymentStatus::Error
    );
    assert_eq!(
        report
            .observations
            .iter()
            .find(|observation| observation.target_path == unaffected_path)
            .unwrap()
            .status,
        DeploymentStatus::CenterNewer
    );
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == target_path
            && diagnostic.status == DeploymentStatus::Error
            && matches!(
                &diagnostic.error,
                WorkspaceError::Catalog {
                    operation: "resolve_match",
                    ..
                }
            )
    }));
}

#[test]
fn reconcile_recovers_an_imported_skill_missing_its_binding() {
    let fixture = fixture_with_unmatched_local_skill();
    let catalog_state = FakeCatalog::empty(fixture.center_root.clone());
    let mut catalog = FailingCatalog::associate_once(catalog_state.clone());
    let local = RecordingLocal::default();
    let mut engine = WorkspaceEngine::new(&local, &mut catalog);

    let first = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(first.imported.len(), 1);
    assert!(catalog_state.associate_calls().is_empty());
    assert!(first.final_report.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == fixture.first_target()
            && matches!(
                &diagnostic.error,
                WorkspaceError::Catalog {
                    operation: "associate",
                    source: CatalogFailure::Conflict { .. },
                }
            )
    }));

    engine
        .observe(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();
    assert!(catalog_state.associate_calls().is_empty());

    let recovered = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(recovered.imported.is_empty());
    assert_eq!(catalog_state.import_calls().len(), 1);
    assert_eq!(catalog_state.associate_calls().len(), 1);
    assert_eq!(recovered.final_report.observations.len(), 1);
    assert_eq!(
        recovered.final_report.observations[0].status,
        DeploymentStatus::InSync
    );

    engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();
    assert_eq!(catalog_state.associate_calls().len(), 1);
}

#[test]
fn reconcile_imports_unmatched_local_skill_without_replacing_original() {
    let fixture = fixture_with_unmatched_local_skill();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::empty(fixture.center_root.clone());
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.final_report.diagnostics.len(), 0);
    assert!(fixture.original_skill_path.join("SKILL.md").is_file());
}

#[test]
fn reconcile_imports_identical_local_skills_once_and_associates_all_targets() {
    let fixture = fixture_with_duplicate_unmatched_local_skills();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::empty(fixture.center_root.clone());
    let catalog_state = catalog.clone();
    let mut engine = WorkspaceEngine::new(local, catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.imported.len(), 1);
    assert_eq!(catalog_state.import_calls().len(), 1);
    assert_eq!(catalog_state.associate_calls().len(), 2);
    assert_eq!(report.final_report.observations.len(), 2);
    assert!(report
        .final_report
        .observations
        .iter()
        .all(|observation| observation.key.skill_id == report.imported[0]));
}

#[test]
fn reconcile_propagates_center_version_to_all_corresponding_targets() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let mut engine = WorkspaceEngine::new(local.clone(), catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(report.propagated.len(), 2);
    assert_eq!(local.deploy_calls().len(), 2);
    assert!(report
        .final_report
        .observations
        .iter()
        .all(|observation| observation.status == DeploymentStatus::InSync));
    assert!(report.final_report.observations.iter().all(|observation| {
        observation
            .center
            .as_ref()
            .zip(observation.local.as_ref())
            .is_some_and(|(center, local)| center.version.content_hash == local.content_hash)
    }));
}

#[test]
fn reconcile_keeps_other_targets_when_one_propagation_fails() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let local = RecordingLocal::failing(fixture.first_target());
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let mut engine = WorkspaceEngine::new(local.clone(), catalog);

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(local.successful_deploy_calls().len(), 1);
    assert!(report
        .final_report
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.error, WorkspaceError::ReconcileFailed { .. })));
}

#[test]
fn reconcile_updates_center_once_for_multiple_newer_copies() {
    let fixture = fixture_with_two_local_newer_copies();
    let local = RecordingLocal::default();
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let mut engine = WorkspaceEngine::new(local, catalog.clone());

    let report = engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert_eq!(catalog.update_calls().len(), 1);
    assert_eq!(report.center_updated.len(), 1);
}

#[test]
fn reconcile_keeps_center_when_local_marker_time_is_unavailable() {
    let fixture = fixture_with_center_newer_skill_and_two_targets();
    let local = RecordingLocal::without_marker_time();
    let catalog = FakeCatalog::with_center_and_bindings(&fixture);
    let mut engine = WorkspaceEngine::new(local, catalog.clone());

    engine
        .reconcile(
            &fixture.workspace,
            &fixture.harnesses,
            &fixture.environment,
            DeploymentMode::Copy,
        )
        .unwrap();

    assert!(catalog.update_calls().is_empty());
}

#[test]
fn system_local_port_copies_and_reads_a_skill() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("target");
    let local = SystemLocalSkillPort;

    local
        .deploy(&source, &target, DeploymentMode::Copy)
        .unwrap();

    let report = local.scan(root.path(), ScanMode::Flat).unwrap();
    assert!(report.skills.iter().any(|skill| skill.path == target));
}

#[cfg(windows)]
#[test]
fn system_local_port_deploys_a_junction() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("target");
    let local = SystemLocalSkillPort;

    local
        .deploy(&source, &target, DeploymentMode::Junction)
        .unwrap();

    let report = local.scan(root.path(), ScanMode::Flat).unwrap();
    assert!(report.skills.iter().any(|skill| skill.path == target));
}

#[cfg(not(windows))]
#[test]
fn system_local_port_rejects_junction_on_non_windows() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("target");
    let local = SystemLocalSkillPort;

    let error = local
        .deploy(&source, &target, DeploymentMode::Junction)
        .unwrap_err();

    assert!(matches!(
        error,
        skill_local::LocalError::UnsupportedOperation {
            operation: "junction"
        }
    ));
}

fn custom_adapter(
    id: &str,
    global_skills_path: &Path,
    project_skills_path: Option<&str>,
) -> HarnessAdapter {
    HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: id.to_owned(),
        display_name: format!("{id} Harness"),
        global_skills_path: global_skills_path.to_string_lossy().into_owned(),
        project_skills_path: project_skills_path.map(str::to_owned),
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .expect("custom harness")
}

fn linked_resolution(
    root: PathBuf,
    disabled_root: Option<PathBuf>,
) -> Result<WorkspaceResolution, WorkspaceError> {
    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(99)),
        kind: WorkspaceKind::Linked {
            root,
            disabled_root,
        },
    };

    resolve_workspace(
        &workspace,
        &HarnessRegistry::with_builtins(),
        &HarnessEnvironment::new("C:/home", None),
        DeploymentMode::SymbolicLink,
    )
}

#[test]
fn target_resolution_linked_workspace_uses_a_stable_logical_harness_id() {
    let workspace_id = WorkspaceId::from_uuid(Uuid::from_u128(1));
    let workspace = Workspace {
        id: workspace_id,
        kind: WorkspaceKind::Linked {
            root: PathBuf::from("C:/managed/linked-skills"),
            disabled_root: None,
        },
    };
    let registry = HarnessRegistry::with_builtins();
    let environment = HarnessEnvironment::new("C:/home", None);

    let resolution = resolve_workspace(
        &workspace,
        &registry,
        &environment,
        DeploymentMode::SymbolicLink,
    )
    .unwrap();

    assert_eq!(resolution.targets.len(), 1);
    assert_eq!(
        resolution.targets[0].harness_id.as_str(),
        "linked-00000000-0000-0000-0000-000000000001"
    );
    assert_eq!(resolution.targets[0].role, TargetRole::Primary);
    assert_eq!(
        resolution.targets[0].path,
        PathBuf::from("C:/managed/linked-skills")
    );
}

#[test]
fn target_resolution_agents_uses_a_custom_global_path_as_a_primary_target() {
    let temp = tempdir().unwrap();
    let global_skills_path = temp.path().join("custom/global-skills");
    let mut registry = HarnessRegistry::with_builtins();
    registry
        .register(custom_adapter("custom_agent", &global_skills_path, None))
        .unwrap();
    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(2)),
        kind: WorkspaceKind::Agents,
    };
    let environment = HarnessEnvironment::new(temp.path().join("home"), None);

    let resolution =
        resolve_workspace(&workspace, &registry, &environment, DeploymentMode::Copy).unwrap();

    assert_eq!(resolution.targets.len(), 1);
    assert_eq!(resolution.discovery_roots.len(), 0);
    assert_eq!(resolution.unsupported.len(), 0);
    assert_eq!(resolution.targets[0].workspace_id, workspace.id);
    assert_eq!(resolution.targets[0].harness_id.as_str(), "custom_agent");
    assert_eq!(resolution.targets[0].path, global_skills_path);
    assert_eq!(resolution.targets[0].role, TargetRole::Primary);
    assert_eq!(resolution.targets[0].scan_mode, ScanMode::Flat);
    assert_eq!(resolution.targets[0].deployment_mode, DeploymentMode::Copy);
}

#[test]
fn target_resolution_agents_uses_capabilities_and_deduplicates_discovery_roots() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::create_dir_all(home.join(".copilot")).unwrap();
    fs::create_dir_all(home.join(".hermes")).unwrap();
    fs::create_dir_all(home.join(".agents/skills")).unwrap();
    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(3)),
        kind: WorkspaceKind::Agents,
    };
    let environment = HarnessEnvironment::new(home.clone(), None);

    let resolution = resolve_workspace(
        &workspace,
        &HarnessRegistry::with_builtins(),
        &environment,
        DeploymentMode::SymbolicLink,
    )
    .unwrap();

    let codex = resolution
        .targets
        .iter()
        .find(|target| target.harness_id.as_str() == "codex")
        .unwrap();
    let hermes = resolution
        .targets
        .iter()
        .find(|target| target.harness_id.as_str() == "hermes")
        .unwrap();
    let copilot = resolution
        .targets
        .iter()
        .find(|target| target.harness_id.as_str() == "github_copilot")
        .unwrap();

    assert_eq!(codex.path, home.join(".codex/skills"));
    assert_eq!(codex.scan_mode, ScanMode::Flat);
    assert_eq!(hermes.path, home.join(".hermes/skills"));
    assert_eq!(hermes.scan_mode, ScanMode::Recursive);
    assert_eq!(copilot.path, home.join(".copilot/skills"));
    assert!(resolution
        .targets
        .iter()
        .all(|target| target.path != home.join(".agents/skills")));
    assert_eq!(resolution.discovery_roots.len(), 1);
    assert_eq!(
        resolution.discovery_roots[0].path,
        home.join(".agents/skills")
    );
    assert_eq!(resolution.discovery_roots[0].scan_mode, ScanMode::Flat);
}

#[test]
fn target_resolution_project_uses_a_custom_project_relative_path() {
    let temp = tempdir().unwrap();
    let project_root = temp.path().join("project");
    let global_skills_path = temp.path().join("custom/global-skills");
    let mut registry = HarnessRegistry::with_builtins();
    registry
        .register(custom_adapter(
            "custom_project_agent",
            &global_skills_path,
            Some(".custom/skills"),
        ))
        .unwrap();
    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(4)),
        kind: WorkspaceKind::Project {
            root: project_root.clone(),
        },
    };
    let environment = HarnessEnvironment::new(temp.path().join("home"), None);

    let resolution = resolve_workspace(
        &workspace,
        &registry,
        &environment,
        DeploymentMode::Junction,
    )
    .unwrap();

    assert_eq!(resolution.targets.len(), 1);
    assert_eq!(resolution.discovery_roots.len(), 0);
    assert_eq!(resolution.unsupported.len(), 0);
    assert_eq!(
        resolution.targets[0].harness_id.as_str(),
        "custom_project_agent"
    );
    assert_eq!(
        resolution.targets[0].path,
        project_root.join(".custom/skills")
    );
    assert_eq!(resolution.targets[0].role, TargetRole::Primary);
    assert_eq!(resolution.targets[0].scan_mode, ScanMode::Recursive);
    assert_eq!(
        resolution.targets[0].deployment_mode,
        DeploymentMode::Junction
    );
}

#[test]
fn target_resolution_project_records_unsupported_harnesses_and_keeps_supported_targets() {
    let temp = tempdir().unwrap();
    let project_root = temp.path().join("project");
    let mut registry = HarnessRegistry::with_builtins();
    registry
        .register(custom_adapter(
            "project_capable",
            &temp.path().join("project-global"),
            Some(".project/skills"),
        ))
        .unwrap();
    registry
        .register(custom_adapter(
            "global_only",
            &temp.path().join("global-only"),
            None,
        ))
        .unwrap();
    let workspace = Workspace {
        id: WorkspaceId::from_uuid(Uuid::from_u128(5)),
        kind: WorkspaceKind::Project {
            root: project_root.clone(),
        },
    };
    let environment = HarnessEnvironment::new(temp.path().join("home"), None);

    let resolution =
        resolve_workspace(&workspace, &registry, &environment, DeploymentMode::Copy).unwrap();

    assert_eq!(resolution.targets.len(), 1);
    assert_eq!(resolution.targets[0].harness_id.as_str(), "project_capable");
    assert_eq!(
        resolution.targets[0].path,
        project_root.join(".project/skills")
    );
    assert_eq!(resolution.unsupported.len(), 1);
    assert_eq!(resolution.unsupported[0].harness_id.as_str(), "global_only");
    assert_eq!(resolution.unsupported[0].path, project_root);
    assert_eq!(
        resolution.unsupported[0].reason,
        "project scope is not supported"
    );
}

#[test]
fn target_resolution_linked_workspace_emits_primary_and_disabled_targets() {
    let root = PathBuf::from("C:/managed/linked-skills");
    let disabled_root = PathBuf::from("C:/managed/disabled-skills");
    let resolution = linked_resolution(root.clone(), Some(disabled_root.clone())).unwrap();

    assert_eq!(resolution.targets.len(), 2);
    assert_eq!(resolution.targets[0].path, root);
    assert_eq!(resolution.targets[0].role, TargetRole::Primary);
    assert_eq!(resolution.targets[1].path, disabled_root);
    assert_eq!(resolution.targets[1].role, TargetRole::Disabled);
    assert!(resolution
        .targets
        .iter()
        .all(|target| target.scan_mode == ScanMode::Recursive));
    assert_eq!(
        resolution.targets[0].harness_id,
        resolution.targets[1].harness_id
    );
    assert_eq!(resolution.discovery_roots.len(), 0);
    assert_eq!(resolution.unsupported.len(), 0);
}

#[test]
fn target_resolution_rejects_empty_linked_roots() {
    let cases = [
        (PathBuf::new(), None),
        (
            PathBuf::from("C:/managed/linked-skills"),
            Some(PathBuf::new()),
        ),
    ];

    for (root, disabled_root) in cases {
        let error = linked_resolution(root, disabled_root).unwrap_err();
        assert!(matches!(error, WorkspaceError::InvalidTarget { .. }));
    }
}

#[test]
fn target_resolution_rejects_parent_segments_in_linked_roots() {
    let error = linked_resolution(PathBuf::from("managed/../linked-skills"), None).unwrap_err();

    assert!(matches!(error, WorkspaceError::InvalidTarget { .. }));
}

#[cfg(windows)]
#[test]
fn target_resolution_rejects_case_insensitive_overlapping_windows_linked_roots() {
    let error = linked_resolution(
        PathBuf::from(r"C:\Managed\linked-skills"),
        Some(PathBuf::from(r"c:\managed\linked-skills\disabled")),
    )
    .unwrap_err();

    assert!(matches!(error, WorkspaceError::InvalidWorkspace { .. }));
}

#[test]
fn target_resolution_rejects_equal_or_lexically_nested_linked_roots() {
    let cases = [
        (
            PathBuf::from("C:/managed/linked-skills"),
            PathBuf::from("C:/managed/linked-skills"),
        ),
        (
            PathBuf::from("C:/managed/./linked-skills"),
            PathBuf::from("C:/managed/linked-skills/disabled"),
        ),
        (
            PathBuf::from("C:/managed/linked-skills/disabled"),
            PathBuf::from("C:/managed/linked-skills"),
        ),
    ];

    for (root, disabled_root) in cases {
        let error = linked_resolution(root, Some(disabled_root)).unwrap_err();
        assert!(matches!(error, WorkspaceError::InvalidWorkspace { .. }));
    }
}
