use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use skill_core::{InstalledSkill, SkillId, SkillSetId, SkillSource};
use skill_harness::HarnessId;
use skill_index::{
    IndexDiagnostic, IndexError, IndexState, IndexedSkill, ReconcileReport as IndexReconcileReport,
    SkillIndex,
};
use skill_local::{
    copy_skill, read_skill, scan_directory, ExistingDestination, LocalError, ScanMode, ScannedSkill,
};
use skill_workspace::{
    CatalogFailure, CentralCatalogPort, CentralMatch, CentralSkillSnapshot, DeploymentBinding,
    DeploymentKey, DeploymentMode, SkillVersion, Workspace, WorkspaceId, WorkspaceKind,
};
use thiserror::Error;

const CATALOG_ROOT_KEY: &str = "catalog_root";
const CACHE_DIRECTORY_NAME: &str = "cache";
const SKILLS_DIRECTORY_NAME: &str = "skills";
const SKILL_INDEX_DATABASE_NAME: &str = "skill-index.sqlite3";

#[derive(Debug, Clone)]
pub struct StoredWorkspace {
    pub name: String,
    pub workspace: Workspace,
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSkillSet {
    pub id: SkillSetId,
    pub name: String,
    pub skill_ids: Vec<SkillId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogActivity {
    pub occurred_at_epoch_seconds: i64,
    pub kind: CatalogActivityKind,
}

#[derive(Debug, Clone)]
pub struct CatalogIndexView {
    pub skills: Vec<CentralSkillSnapshot>,
    pub diagnostics: Vec<IndexDiagnostic>,
    pub state: IndexState,
    pub revision: i64,
    pub last_reconciled_at_epoch_millis: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIndexWorkerConfig {
    pub database_path: PathBuf,
    pub skills_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogActivityKind {
    Imported,
    Updated,
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database operation {operation} failed")]
    Database {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("filesystem operation {operation} failed for {path:?}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("local skill operation {operation} failed")]
    Local {
        operation: &'static str,
        #[source]
        source: Box<LocalError>,
    },
    #[error("derived skill index operation {operation} failed")]
    Index {
        operation: &'static str,
        #[source]
        source: Box<IndexError>,
    },
    #[error(
        "derived skill index operation {operation} failed after filesystem commit at {path:?}"
    )]
    IndexAfterFilesystemCommit {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: Box<IndexError>,
    },
    #[error("persisted {entity} field {field} is invalid")]
    InvalidData {
        entity: &'static str,
        field: &'static str,
    },
    #[error("{entity} was not found: {id}")]
    NotFound { entity: &'static str, id: String },
    #[error("{entity} conflicts with an existing record: {id}")]
    Conflict { entity: &'static str, id: String },
    #[error("the central catalog path cannot change while skills are installed")]
    CatalogNotEmpty,
    #[error("system time is before the Unix epoch")]
    Clock,
    #[error("{operation} failed and cleanup also failed")]
    Cleanup {
        operation: &'static str,
        #[source]
        source: Box<PersistenceError>,
        cleanup: Box<PersistenceError>,
    },
}

impl PersistenceError {
    fn database(operation: &'static str, source: rusqlite::Error) -> Self {
        Self::Database { operation, source }
    }

    fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    fn local(operation: &'static str, source: LocalError) -> Self {
        Self::Local {
            operation,
            source: Box::new(source),
        }
    }

    fn index(operation: &'static str, source: IndexError) -> Self {
        Self::Index {
            operation,
            source: Box::new(source),
        }
    }

    fn into_catalog_failure(self) -> CatalogFailure {
        match self {
            Self::InvalidData { entity, field } => {
                CatalogFailure::invalid_data(format!("invalid {field} in {entity}"))
            }
            Self::NotFound { entity, id } => CatalogFailure::not_found(format!("{entity}:{id}")),
            Self::Conflict { entity, id } => CatalogFailure::conflict(format!("{entity}:{id}")),
            Self::Local { source, .. } => CatalogFailure::local_operation(*source),
            error => CatalogFailure::storage(error),
        }
    }
}

pub struct PersistentCatalog {
    connection: Connection,
    catalog_root: PathBuf,
    skill_index: SkillIndex,
}

impl PersistentCatalog {
    pub fn open(
        database_path: impl AsRef<Path>,
        default_catalog_root: impl AsRef<Path>,
    ) -> Result<Self, PersistenceError> {
        let database_path = database_path.as_ref();
        let skill_index_database_path = database_path.with_file_name(SKILL_INDEX_DATABASE_NAME);
        if let Some(parent) = database_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|source| PersistenceError::io("create_database_parent", parent, source))?;
        }

        let mut connection = Connection::open(database_path)
            .map_err(|source| PersistenceError::database("open", source))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|source| PersistenceError::database("configure_busy_timeout", source))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|source| PersistenceError::database("enable_foreign_keys", source))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|source| PersistenceError::database("enable_wal", source))?;

        initialize_schema(&mut connection)?;

        let persisted_root = connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [CATALOG_ROOT_KEY],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(|source| PersistenceError::database("load_catalog_root", source))?;
        let catalog_root = match persisted_root {
            Some(encoded) => decode_path(&encoded, "app_settings", "catalog_root")?,
            None => {
                let root = default_catalog_root.as_ref().to_path_buf();
                validate_absolute_path(&root, "app_settings", "catalog_root")?;
                fs::create_dir_all(&root).map_err(|source| {
                    PersistenceError::io("create_default_catalog_root", &root, source)
                })?;
                connection
                    .execute(
                        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
                        params![CATALOG_ROOT_KEY, encode_path(&root)],
                    )
                    .map_err(|source| {
                        PersistenceError::database("persist_default_catalog_root", source)
                    })?;
                root
            }
        };

        validate_absolute_path(&catalog_root, "app_settings", "catalog_root")?;
        fs::create_dir_all(&catalog_root)
            .map_err(|source| PersistenceError::io("create_catalog_root", &catalog_root, source))?;

        ensure_catalog_directories(&catalog_root)?;
        let (mut skill_index, index_status) = SkillIndex::open(&skill_index_database_path)
            .map_err(|source| PersistenceError::index("open", source))?;
        if let Some(backup) = index_status.recovered_from {
            eprintln!(
                "recovered the derived skill index after moving the unusable database to {}",
                backup.display()
            );
        }
        let skills_root = catalog_root.join(SKILLS_DIRECTORY_NAME);
        let needs_rebuild = index_status.needs_rebuild
            || !skill_index
                .matches_root(&skills_root)
                .map_err(|source| PersistenceError::index("validate_catalog_root", source))?;
        if needs_rebuild {
            let cancellation = AtomicBool::new(false);
            skill_index
                .rebuild(&skills_root, &cancellation)
                .map_err(|source| PersistenceError::index("initial_rebuild", source))?;
        } else {
            skill_index
                .mark_stale()
                .map_err(|source| PersistenceError::index("mark_startup_stale", source))?;
        }
        let mut catalog = Self {
            connection,
            catalog_root,
            skill_index,
        };
        catalog.ensure_agents_workspace()?;
        Ok(catalog)
    }

    pub fn catalog_root(&self) -> &Path {
        &self.catalog_root
    }

    pub fn catalog_index_worker_config(&self) -> CatalogIndexWorkerConfig {
        CatalogIndexWorkerConfig {
            database_path: self.skill_index.database_path().to_path_buf(),
            skills_root: self.catalog_root.join(SKILLS_DIRECTORY_NAME),
        }
    }

    pub fn rebuild_catalog_index(
        &mut self,
        cancellation: &AtomicBool,
    ) -> Result<IndexReconcileReport, PersistenceError> {
        self.skill_index
            .rebuild(&self.catalog_root.join(SKILLS_DIRECTORY_NAME), cancellation)
            .map_err(|source| PersistenceError::index("rebuild", source))
    }

    pub fn set_catalog_root(&mut self, root: PathBuf) -> Result<(), PersistenceError> {
        validate_absolute_path(&root, "app_settings", "catalog_root")?;
        let metadata = fs::metadata(&root)
            .map_err(|source| PersistenceError::io("inspect_catalog_root", &root, source))?;
        if !metadata.is_dir() {
            return Err(PersistenceError::InvalidData {
                entity: "app_settings",
                field: "catalog_root",
            });
        }

        if !directory_is_empty(&self.catalog_root.join(SKILLS_DIRECTORY_NAME))? {
            return Err(PersistenceError::CatalogNotEmpty);
        }

        ensure_catalog_directories(&root)?;
        self.connection
            .execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![CATALOG_ROOT_KEY, encode_path(&root)],
            )
            .map_err(|source| PersistenceError::database("update_catalog_root", source))?;
        self.catalog_root = root;
        let cancellation = AtomicBool::new(false);
        self.skill_index
            .rebuild(
                &self.catalog_root.join(SKILLS_DIRECTORY_NAME),
                &cancellation,
            )
            .map_err(|source| PersistenceError::IndexAfterFilesystemCommit {
                operation: "rebuild_after_catalog_root_update",
                path: self.catalog_root.clone(),
                source: Box::new(source),
            })?;
        Ok(())
    }

    pub fn ensure_agents_workspace(&mut self) -> Result<StoredWorkspace, PersistenceError> {
        if let Some(mut workspace) = self
            .list_workspaces()?
            .into_iter()
            .find(|stored| matches!(stored.workspace.kind, WorkspaceKind::Agents))
        {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|source| {
                    PersistenceError::database("begin_enforce_agents_link_mode", source)
                })?;
            transaction
                .execute(
                    "UPDATE workspaces SET deployment_mode = 'link' WHERE workspace_id = ?1",
                    [workspace.workspace.id.to_string()],
                )
                .map_err(|source| PersistenceError::database("update_agents_link_mode", source))?;
            transaction
                .execute(
                    "UPDATE deployment_bindings
                     SET deployment_mode = 'link'
                     WHERE workspace_id = ?1",
                    [workspace.workspace.id.to_string()],
                )
                .map_err(|source| {
                    PersistenceError::database("update_agents_binding_link_modes", source)
                })?;
            transaction.commit().map_err(|source| {
                PersistenceError::database("commit_enforce_agents_link_mode", source)
            })?;
            workspace.deployment_mode = DeploymentMode::Link;
            return Ok(workspace);
        }

        let stored = StoredWorkspace {
            name: "Agents".to_owned(),
            workspace: Workspace {
                id: WorkspaceId::new(),
                kind: WorkspaceKind::Agents,
            },
            deployment_mode: DeploymentMode::Link,
        };
        self.insert_workspace(&stored)?;
        Ok(stored)
    }

    pub fn insert_workspace(&mut self, stored: &StoredWorkspace) -> Result<(), PersistenceError> {
        let (kind, root_path, disabled_root_path) = encode_workspace_kind(&stored.workspace.kind);
        self.connection
            .execute(
                "INSERT INTO workspaces (
                     workspace_id, display_name, kind, root_path, disabled_root_path, deployment_mode
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    stored.workspace.id.to_string(),
                    stored.name,
                    kind,
                    root_path,
                    disabled_root_path,
                    encode_deployment_mode(stored.deployment_mode),
                ],
            )
            .map_err(|source| match &source {
                rusqlite::Error::SqliteFailure(error, _)
                    if error.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    PersistenceError::Conflict {
                        entity: "workspace",
                        id: stored.workspace.id.to_string(),
                    }
                }
                _ => PersistenceError::database("insert_workspace", source),
            })?;
        Ok(())
    }

    pub fn list_workspaces(&self) -> Result<Vec<StoredWorkspace>, PersistenceError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT workspace_id, display_name, kind, root_path, disabled_root_path,
                        deployment_mode
                 FROM workspaces
                 ORDER BY CASE kind WHEN 'agents' THEN 0 WHEN 'project' THEN 1 ELSE 2 END,
                          lower(display_name), workspace_id",
            )
            .map_err(|source| PersistenceError::database("prepare_list_workspaces", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(WorkspaceRow {
                    workspace_id: row.get(0)?,
                    display_name: row.get(1)?,
                    kind: row.get(2)?,
                    root_path: row.get(3)?,
                    disabled_root_path: row.get(4)?,
                    deployment_mode: row.get(5)?,
                })
            })
            .map_err(|source| PersistenceError::database("query_list_workspaces", source))?;

        let mut workspaces = Vec::new();
        for row in rows {
            let row =
                row.map_err(|source| PersistenceError::database("decode_workspace_row", source))?;
            workspaces.push(row.into_stored()?);
        }
        Ok(workspaces)
    }

    pub fn workspace(&self, id: WorkspaceId) -> Result<StoredWorkspace, PersistenceError> {
        self.list_workspaces()?
            .into_iter()
            .find(|stored| stored.workspace.id == id)
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "workspace",
                id: id.to_string(),
            })
    }

    pub fn list_skill_sets(&self) -> Result<Vec<StoredSkillSet>, PersistenceError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT skill_sets.set_id, skill_sets.display_name, skill_set_members.skill_id
                 FROM skill_sets
                 LEFT JOIN skill_set_members ON skill_set_members.set_id = skill_sets.set_id
                 ORDER BY lower(skill_sets.display_name), skill_sets.set_id,
                          skill_set_members.position",
            )
            .map_err(|source| PersistenceError::database("prepare_list_skill_sets", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|source| PersistenceError::database("query_list_skill_sets", source))?;

        let mut sets = Vec::<StoredSkillSet>::new();
        for row in rows {
            let (raw_set_id, name, raw_skill_id) =
                row.map_err(|source| PersistenceError::database("decode_skill_set_row", source))?;
            let set_id =
                SkillSetId::parse(&raw_set_id).map_err(|_| PersistenceError::InvalidData {
                    entity: "skill_sets",
                    field: "set_id",
                })?;
            let needs_new_set = sets.last().is_none_or(|stored| stored.id != set_id);
            if needs_new_set {
                sets.push(StoredSkillSet {
                    id: set_id,
                    name,
                    skill_ids: Vec::new(),
                });
            }
            if let Some(raw_skill_id) = raw_skill_id {
                let skill_id =
                    SkillId::parse(&raw_skill_id).map_err(|_| PersistenceError::InvalidData {
                        entity: "skill_set_members",
                        field: "skill_id",
                    })?;
                if let Some(stored) = sets.last_mut() {
                    stored.skill_ids.push(skill_id);
                }
            }
        }
        Ok(sets)
    }

    pub fn insert_skill_set(&mut self, stored: &StoredSkillSet) -> Result<(), PersistenceError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| PersistenceError::database("begin_insert_skill_set", source))?;
        insert_skill_set_definition(&transaction, stored)?;
        replace_skill_set_members(&transaction, stored.id, &stored.skill_ids)?;
        transaction
            .commit()
            .map_err(|source| PersistenceError::database("commit_insert_skill_set", source))?;
        Ok(())
    }

    pub fn update_skill_set(&mut self, stored: &StoredSkillSet) -> Result<(), PersistenceError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| PersistenceError::database("begin_update_skill_set", source))?;
        let changed = transaction
            .execute(
                "UPDATE skill_sets SET display_name = ?1 WHERE set_id = ?2",
                params![stored.name, stored.id.to_string()],
            )
            .map_err(|source| map_skill_set_constraint(source, &stored.name))?;
        if changed == 0 {
            return Err(PersistenceError::NotFound {
                entity: "skill_set",
                id: stored.id.to_string(),
            });
        }
        transaction
            .execute(
                "DELETE FROM skill_set_members WHERE set_id = ?1",
                [stored.id.to_string()],
            )
            .map_err(|source| PersistenceError::database("clear_skill_set_members", source))?;
        replace_skill_set_members(&transaction, stored.id, &stored.skill_ids)?;
        transaction
            .commit()
            .map_err(|source| PersistenceError::database("commit_update_skill_set", source))?;
        Ok(())
    }

    pub fn delete_skill_sets(&mut self, set_ids: &[SkillSetId]) -> Result<(), PersistenceError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| PersistenceError::database("begin_delete_skill_sets", source))?;
        for set_id in set_ids {
            let changed = transaction
                .execute(
                    "DELETE FROM skill_sets WHERE set_id = ?1",
                    [set_id.to_string()],
                )
                .map_err(|source| PersistenceError::database("delete_skill_set", source))?;
            if changed == 0 {
                return Err(PersistenceError::NotFound {
                    entity: "skill_set",
                    id: set_id.to_string(),
                });
            }
        }
        transaction
            .commit()
            .map_err(|source| PersistenceError::database("commit_delete_skill_sets", source))?;
        Ok(())
    }

    pub fn remove_skill_from_sets(&mut self, skill_id: SkillId) -> Result<(), PersistenceError> {
        self.connection
            .execute(
                "DELETE FROM skill_set_members WHERE skill_id = ?1",
                [skill_id.to_string()],
            )
            .map_err(|source| PersistenceError::database("remove_skill_from_sets", source))?;
        Ok(())
    }

    pub fn catalog_index_view(&self) -> Result<CatalogIndexView, PersistenceError> {
        let snapshot = self
            .skill_index
            .snapshot()
            .map_err(|source| PersistenceError::index("query_snapshot", source))?;
        Ok(CatalogIndexView {
            skills: snapshot
                .skills
                .into_iter()
                .map(snapshot_from_indexed)
                .collect(),
            diagnostics: snapshot.diagnostics,
            state: snapshot.state,
            revision: snapshot.revision,
            last_reconciled_at_epoch_millis: snapshot.last_reconciled_at_epoch_millis,
        })
    }

    pub fn list_catalog_skills(&self) -> Result<Vec<CentralSkillSnapshot>, PersistenceError> {
        Ok(self.catalog_index_view()?.skills)
    }

    pub fn catalog_skill(
        &self,
        skill_id: SkillId,
    ) -> Result<(CentralSkillSnapshot, ScannedSkill), PersistenceError> {
        let indexed_path = self
            .skill_index
            .skill(skill_id)
            .map_err(|source| PersistenceError::index("query_skill", source))?
            .map(|indexed| indexed.path);
        let path = match indexed_path {
            Some(path) if path.is_dir() => path,
            _ => self.find_catalog_skill_path(skill_id)?,
        };
        let scanned = read_skill(&path)
            .map_err(|source| PersistenceError::local("read_catalog_skill", source))?;
        let snapshot = snapshot_from_scanned_ref(
            skill_id,
            SkillSource::Local {
                path: scanned.path.clone(),
            },
            &scanned,
        );
        Ok((snapshot, scanned))
    }

    fn find_catalog_skill_path(&self, skill_id: SkillId) -> Result<PathBuf, PersistenceError> {
        let skills_root = self.catalog_root.join(SKILLS_DIRECTORY_NAME);
        let report = scan_directory(&skills_root, ScanMode::Flat)
            .map_err(|source| PersistenceError::local("scan_catalog_skills", source))?;
        report
            .skills
            .into_iter()
            .find_map(|scanned| {
                let directory_name = scanned.path.file_name()?;
                (SkillId::from_directory_name(directory_name) == skill_id).then_some(scanned.path)
            })
            .ok_or_else(|| PersistenceError::NotFound {
                entity: "skill",
                id: skill_id.to_string(),
            })
    }

    pub fn all_bindings(&self) -> Result<Vec<DeploymentBinding>, PersistenceError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT skill_id, harness_id, workspace_id, target_path, deployment_mode
                 FROM deployment_bindings
                 ORDER BY workspace_id, harness_id, skill_id",
            )
            .map_err(|source| PersistenceError::database("prepare_list_bindings", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(BindingRow {
                    skill_id: row.get(0)?,
                    harness_id: row.get(1)?,
                    workspace_id: row.get(2)?,
                    target_path: row.get(3)?,
                    deployment_mode: row.get(4)?,
                })
            })
            .map_err(|source| PersistenceError::database("query_list_bindings", source))?;

        let mut bindings = Vec::new();
        for row in rows {
            let row =
                row.map_err(|source| PersistenceError::database("decode_binding_row", source))?;
            bindings.push(row.into_binding()?);
        }
        Ok(bindings)
    }

    pub fn delete_bindings_for_skill(&mut self, skill_id: SkillId) -> Result<(), PersistenceError> {
        self.connection
            .execute(
                "DELETE FROM deployment_bindings WHERE skill_id = ?1",
                [skill_id.to_string()],
            )
            .map_err(|source| PersistenceError::database("delete_skill_bindings", source))?;
        Ok(())
    }

    pub fn remove_catalog_skill_from_index(
        &mut self,
        skill_id: SkillId,
        deleted_path: &Path,
    ) -> Result<(), PersistenceError> {
        self.skill_index.remove_skill(skill_id).map_err(|source| {
            PersistenceError::IndexAfterFilesystemCommit {
                operation: "remove_deleted_skill",
                path: deleted_path.to_path_buf(),
                source: Box::new(source),
            }
        })
    }

    pub fn delete_bindings_for_harness_workspace(
        &mut self,
        harness_id: &HarnessId,
        workspace_id: WorkspaceId,
    ) -> Result<(), PersistenceError> {
        self.connection
            .execute(
                "DELETE FROM deployment_bindings
                 WHERE harness_id = ?1 AND workspace_id = ?2",
                params![harness_id.as_str(), workspace_id.to_string()],
            )
            .map_err(|source| {
                PersistenceError::database("delete_harness_workspace_bindings", source)
            })?;
        Ok(())
    }

    pub fn activity_since(
        &self,
        minimum_epoch_seconds: i64,
    ) -> Result<Vec<CatalogActivity>, PersistenceError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT occurred_at, kind
                 FROM catalog_activity
                 WHERE occurred_at >= ?1
                 ORDER BY occurred_at, activity_id",
            )
            .map_err(|source| PersistenceError::database("prepare_catalog_activity", source))?;
        let rows = statement
            .query_map([minimum_epoch_seconds], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| PersistenceError::database("query_catalog_activity", source))?;

        let mut activity = Vec::new();
        for row in rows {
            let (occurred_at_epoch_seconds, kind) =
                row.map_err(|source| PersistenceError::database("decode_activity_row", source))?;
            let kind = match kind.as_str() {
                "imported" => CatalogActivityKind::Imported,
                "updated" => CatalogActivityKind::Updated,
                _ => {
                    return Err(PersistenceError::InvalidData {
                        entity: "catalog_activity",
                        field: "kind",
                    })
                }
            };
            activity.push(CatalogActivity {
                occurred_at_epoch_seconds,
                kind,
            });
        }
        Ok(activity)
    }

    fn cleanup_failed_import(
        &self,
        pending_path: &Path,
        pending_owned: bool,
        final_path: &Path,
        final_owned: bool,
        operation: PersistenceError,
    ) -> PersistenceError {
        let cleanup = (|| {
            if pending_owned {
                remove_path_if_exists(pending_path, "cleanup_pending_import")?;
            }
            if final_owned {
                remove_path_if_exists(final_path, "cleanup_final_import")?;
            }
            Ok::<(), PersistenceError>(())
        })();

        match cleanup {
            Ok(()) => operation,
            Err(cleanup) => PersistenceError::Cleanup {
                operation: "import_local",
                source: Box::new(operation),
                cleanup: Box::new(cleanup),
            },
        }
    }
}

impl CentralCatalogPort for PersistentCatalog {
    fn list(&self) -> Result<Vec<CentralSkillSnapshot>, CatalogFailure> {
        self.list_catalog_skills()
            .map_err(PersistenceError::into_catalog_failure)
    }

    fn bindings(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<DeploymentBinding>, CatalogFailure> {
        self.all_bindings()
            .map(|bindings| {
                bindings
                    .into_iter()
                    .filter(|binding| binding.key.workspace_id == workspace_id)
                    .collect()
            })
            .map_err(PersistenceError::into_catalog_failure)
    }

    fn resolve_match(
        &self,
        scanned: &ScannedSkill,
        target_path: &Path,
    ) -> Result<CentralMatch, CatalogFailure> {
        let bindings = self
            .all_bindings()
            .map_err(PersistenceError::into_catalog_failure)?;
        let mut path_matches: Vec<SkillId> = bindings
            .into_iter()
            .filter(|binding| binding.target_path == target_path)
            .map(|binding| binding.key.skill_id)
            .collect();
        path_matches.sort_by_key(ToString::to_string);
        path_matches.dedup();
        match path_matches.as_slice() {
            [skill_id] => return Ok(CentralMatch::Unique(*skill_id)),
            [_, _, ..] => return Ok(CentralMatch::Ambiguous(path_matches)),
            [] => {}
        }

        let mut hash_matches: Vec<SkillId> = self
            .list_catalog_skills()
            .map_err(PersistenceError::into_catalog_failure)?
            .into_iter()
            .filter(|snapshot| snapshot.version.content_hash == scanned.content_hash)
            .map(|snapshot| snapshot.installed.id)
            .collect();
        hash_matches.sort_by_key(ToString::to_string);
        hash_matches.dedup();
        Ok(match hash_matches.as_slice() {
            [] => CentralMatch::None,
            [skill_id] => CentralMatch::Unique(*skill_id),
            _ => CentralMatch::Ambiguous(hash_matches),
        })
    }

    fn import_local(
        &mut self,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        let directory_name = scanned.path.file_name().ok_or_else(|| {
            CatalogFailure::invalid_data("local skill path has no directory name")
        })?;
        let skill_id = SkillId::from_directory_name(directory_name);
        let pending_path = self
            .catalog_root
            .join(CACHE_DIRECTORY_NAME)
            .join(directory_name);
        let final_path = self
            .catalog_root
            .join(SKILLS_DIRECTORY_NAME)
            .join(directory_name);
        if pending_path.exists() || final_path.exists() {
            return Err(CatalogFailure::conflict(format!(
                "catalog path:{}",
                final_path.display()
            )));
        }

        if let Err(source) = copy_skill(&scanned.path, &pending_path, ExistingDestination::Reject) {
            let error = self.cleanup_failed_import(
                &pending_path,
                false,
                &final_path,
                false,
                PersistenceError::local("copy_imported_skill", source),
            );
            return Err(error.into_catalog_failure());
        }
        if let Err(source) = fs::rename(&pending_path, &final_path) {
            let error = self.cleanup_failed_import(
                &pending_path,
                true,
                &final_path,
                false,
                PersistenceError::io("commit_imported_skill", &final_path, source),
            );
            return Err(error.into_catalog_failure());
        }

        let imported = match read_skill(&final_path) {
            Ok(imported) => imported,
            Err(source) => {
                let error = self.cleanup_failed_import(
                    &pending_path,
                    false,
                    &final_path,
                    true,
                    PersistenceError::local("verify_imported_skill", source),
                );
                return Err(error.into_catalog_failure());
            }
        };
        self.skill_index
            .refresh_skill(skill_id, &final_path)
            .map_err(|source| PersistenceError::IndexAfterFilesystemCommit {
                operation: "index_imported_skill",
                path: final_path.clone(),
                source: Box::new(source),
            })
            .map_err(PersistenceError::into_catalog_failure)?;
        if let Ok(occurred_at) = unix_timestamp() {
            if self
                .connection
                .execute(
                    "INSERT INTO catalog_activity (skill_id, kind, occurred_at)
                     VALUES (?1, 'imported', ?2)",
                    params![skill_id.to_string(), occurred_at],
                )
                .is_err()
            {
                eprintln!(
                    "failed to record catalog import activity after the import was committed"
                );
            }
        }

        Ok(snapshot_from_scanned(
            skill_id,
            SkillSource::Local {
                path: imported.path.clone(),
            },
            imported,
        ))
    }

    fn update_from_local(
        &mut self,
        skill_id: &SkillId,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure> {
        let (current, _) = self
            .catalog_skill(*skill_id)
            .map_err(PersistenceError::into_catalog_failure)?;
        let occurred_at = unix_timestamp().map_err(PersistenceError::into_catalog_failure)?;
        copy_skill(
            &scanned.path,
            &current.installed.location,
            ExistingDestination::Replace,
        )
        .map_err(CatalogFailure::local_operation)?;
        let updated =
            read_skill(&current.installed.location).map_err(CatalogFailure::local_operation)?;
        self.skill_index
            .refresh_skill(*skill_id, &current.installed.location)
            .map_err(|source| PersistenceError::IndexAfterFilesystemCommit {
                operation: "index_updated_skill",
                path: current.installed.location.clone(),
                source: Box::new(source),
            })
            .map_err(PersistenceError::into_catalog_failure)?;
        if self
            .connection
            .execute(
                "INSERT INTO catalog_activity (skill_id, kind, occurred_at)
                 VALUES (?1, 'updated', ?2)",
                params![skill_id.to_string(), occurred_at],
            )
            .is_err()
        {
            eprintln!("failed to record catalog update activity after the update was committed");
        }

        Ok(snapshot_from_scanned(
            *skill_id,
            current.installed.source,
            updated,
        ))
    }

    fn associate(&mut self, binding: DeploymentBinding) -> Result<(), CatalogFailure> {
        self.catalog_skill(binding.key.skill_id)
            .map_err(PersistenceError::into_catalog_failure)?;
        self.workspace(binding.key.workspace_id)
            .map_err(PersistenceError::into_catalog_failure)?;

        let existing = self
            .connection
            .query_row(
                "SELECT target_path, deployment_mode
                 FROM deployment_bindings
                 WHERE skill_id = ?1 AND harness_id = ?2 AND workspace_id = ?3",
                params![
                    binding.key.skill_id.to_string(),
                    binding.key.harness_id.as_str(),
                    binding.key.workspace_id.to_string(),
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|source| {
                PersistenceError::database("load_existing_binding", source).into_catalog_failure()
            })?;
        if let Some((target_path, deployment_mode)) = existing {
            let target_path = decode_path(&target_path, "deployment_bindings", "target_path")
                .map_err(PersistenceError::into_catalog_failure)?;
            let deployment_mode = decode_deployment_mode(&deployment_mode)
                .map_err(PersistenceError::into_catalog_failure)?;
            if target_path == binding.target_path && deployment_mode == binding.deployment_mode {
                return Ok(());
            }
            return Err(CatalogFailure::conflict(format!("binding:{}", binding.key)));
        }

        let encoded_target = encode_path(&binding.target_path);
        let target_owner = self
            .connection
            .query_row(
                "SELECT skill_id, harness_id, workspace_id
                 FROM deployment_bindings WHERE target_path = ?1 LIMIT 1",
                [encoded_target.clone()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|source| {
                PersistenceError::database("find_binding_target_owner", source)
                    .into_catalog_failure()
            })?;
        if target_owner.is_some() {
            return Err(CatalogFailure::conflict(format!(
                "target:{}",
                binding.target_path.display()
            )));
        }

        self.connection
            .execute(
                "INSERT INTO deployment_bindings (
                     skill_id, harness_id, workspace_id, target_path, deployment_mode
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    binding.key.skill_id.to_string(),
                    binding.key.harness_id.as_str(),
                    binding.key.workspace_id.to_string(),
                    encoded_target,
                    encode_deployment_mode(binding.deployment_mode),
                ],
            )
            .map_err(|source| {
                PersistenceError::database("insert_binding", source).into_catalog_failure()
            })?;
        Ok(())
    }
}

#[derive(Debug)]
struct WorkspaceRow {
    workspace_id: String,
    display_name: String,
    kind: String,
    root_path: Option<Vec<u8>>,
    disabled_root_path: Option<Vec<u8>>,
    deployment_mode: String,
}

impl WorkspaceRow {
    fn into_stored(self) -> Result<StoredWorkspace, PersistenceError> {
        let id =
            WorkspaceId::parse(&self.workspace_id).map_err(|_| PersistenceError::InvalidData {
                entity: "workspaces",
                field: "workspace_id",
            })?;
        if self.display_name.trim().is_empty() {
            return Err(PersistenceError::InvalidData {
                entity: "workspaces",
                field: "display_name",
            });
        }
        let kind = match self.kind.as_str() {
            "agents" if self.root_path.is_none() && self.disabled_root_path.is_none() => {
                WorkspaceKind::Agents
            }
            "project" if self.disabled_root_path.is_none() => WorkspaceKind::Project {
                root: decode_required_path(self.root_path.as_deref(), "workspaces", "root_path")?,
            },
            "linked" => WorkspaceKind::Linked {
                root: decode_required_path(self.root_path.as_deref(), "workspaces", "root_path")?,
                disabled_root: self
                    .disabled_root_path
                    .as_deref()
                    .map(|value| decode_path(value, "workspaces", "disabled_root_path"))
                    .transpose()?,
            },
            _ => {
                return Err(PersistenceError::InvalidData {
                    entity: "workspaces",
                    field: "kind",
                })
            }
        };
        Ok(StoredWorkspace {
            name: self.display_name,
            workspace: Workspace { id, kind },
            deployment_mode: decode_deployment_mode(&self.deployment_mode)?,
        })
    }
}

#[derive(Debug)]
struct BindingRow {
    skill_id: String,
    harness_id: String,
    workspace_id: String,
    target_path: Vec<u8>,
    deployment_mode: String,
}

impl BindingRow {
    fn into_binding(self) -> Result<DeploymentBinding, PersistenceError> {
        let skill_id =
            SkillId::parse(&self.skill_id).map_err(|_| PersistenceError::InvalidData {
                entity: "deployment_bindings",
                field: "skill_id",
            })?;
        let harness_id =
            HarnessId::new(&self.harness_id).map_err(|_| PersistenceError::InvalidData {
                entity: "deployment_bindings",
                field: "harness_id",
            })?;
        let workspace_id =
            WorkspaceId::parse(&self.workspace_id).map_err(|_| PersistenceError::InvalidData {
                entity: "deployment_bindings",
                field: "workspace_id",
            })?;
        Ok(DeploymentBinding {
            key: DeploymentKey {
                skill_id,
                harness_id,
                workspace_id,
            },
            target_path: decode_path(&self.target_path, "deployment_bindings", "target_path")?,
            deployment_mode: decode_deployment_mode(&self.deployment_mode)?,
        })
    }
}

fn initialize_schema(connection: &mut Connection) -> Result<(), PersistenceError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| PersistenceError::database("begin_schema_initialization", source))?;
    if !table_exists(&transaction, "app_settings")? {
        create_schema(&transaction)?;
    }
    create_skill_set_tables(&transaction)?;
    transaction
        .commit()
        .map_err(|source| PersistenceError::database("commit_schema_initialization", source))?;
    Ok(())
}

fn create_skill_set_tables(transaction: &Transaction<'_>) -> Result<(), PersistenceError> {
    transaction
        .execute_batch(
            r#"
             CREATE TABLE IF NOT EXISTS skill_sets (
                 set_id TEXT PRIMARY KEY NOT NULL,
                 display_name TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS skill_set_members (
                 set_id TEXT NOT NULL,
                 skill_id TEXT NOT NULL,
                 position INTEGER NOT NULL,
                 PRIMARY KEY (set_id, skill_id),
                 UNIQUE (set_id, position),
                 FOREIGN KEY (set_id) REFERENCES skill_sets(set_id) ON DELETE CASCADE
             );
             CREATE UNIQUE INDEX IF NOT EXISTS skill_sets_display_name
                 ON skill_sets(lower(display_name));
             "#,
        )
        .map_err(|source| PersistenceError::database("create_skill_set_tables", source))?;
    Ok(())
}

fn insert_skill_set_definition(
    transaction: &Transaction<'_>,
    stored: &StoredSkillSet,
) -> Result<(), PersistenceError> {
    transaction
        .execute(
            "INSERT INTO skill_sets (set_id, display_name) VALUES (?1, ?2)",
            params![stored.id.to_string(), stored.name],
        )
        .map_err(|source| map_skill_set_constraint(source, &stored.name))?;
    Ok(())
}

fn replace_skill_set_members(
    transaction: &Transaction<'_>,
    set_id: SkillSetId,
    skill_ids: &[SkillId],
) -> Result<(), PersistenceError> {
    for (position, skill_id) in skill_ids.iter().enumerate() {
        let position = i64::try_from(position).map_err(|_| PersistenceError::InvalidData {
            entity: "skill_set_members",
            field: "position",
        })?;
        transaction
            .execute(
                "INSERT INTO skill_set_members (set_id, skill_id, position)
                 VALUES (?1, ?2, ?3)",
                params![set_id.to_string(), skill_id.to_string(), position],
            )
            .map_err(|source| PersistenceError::database("insert_skill_set_member", source))?;
    }
    Ok(())
}

fn map_skill_set_constraint(source: rusqlite::Error, name: &str) -> PersistenceError {
    match &source {
        rusqlite::Error::SqliteFailure(error, _)
            if error.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            PersistenceError::Conflict {
                entity: "skill_set",
                id: name.to_owned(),
            }
        }
        _ => PersistenceError::database("write_skill_set", source),
    }
}

fn table_exists(transaction: &Transaction<'_>, table: &str) -> Result<bool, PersistenceError> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|source| PersistenceError::database("inspect_schema", source))
}

fn create_schema(transaction: &Transaction<'_>) -> Result<(), PersistenceError> {
    transaction
        .execute_batch(
            r#"
             CREATE TABLE app_settings (
                 key TEXT PRIMARY KEY NOT NULL,
                 value BLOB NOT NULL
             );
             CREATE TABLE catalog_activity (
                 activity_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 skill_id TEXT NOT NULL,
                 kind TEXT NOT NULL CHECK (kind IN ('imported', 'updated')),
                 occurred_at INTEGER NOT NULL
             );
             CREATE INDEX catalog_activity_time ON catalog_activity(occurred_at);
             "#,
        )
        .map_err(|source| PersistenceError::database("create_schema", source))?;
    create_deployment_tables(transaction)?;
    create_deployment_indexes(transaction)?;
    Ok(())
}

fn create_deployment_tables(transaction: &Transaction<'_>) -> Result<(), PersistenceError> {
    transaction
        .execute_batch(
            r#"
             CREATE TABLE workspaces (
                 workspace_id TEXT PRIMARY KEY NOT NULL,
                 display_name TEXT NOT NULL,
                 kind TEXT NOT NULL CHECK (kind IN ('agents', 'project', 'linked')),
                 root_path BLOB,
                 disabled_root_path BLOB,
                 deployment_mode TEXT NOT NULL CHECK (deployment_mode IN ('copy', 'link'))
             );
             CREATE TABLE deployment_bindings (
                 skill_id TEXT NOT NULL,
                 harness_id TEXT NOT NULL,
                 workspace_id TEXT NOT NULL,
                 target_path BLOB NOT NULL,
                 deployment_mode TEXT NOT NULL CHECK (deployment_mode IN ('copy', 'link')),
                 PRIMARY KEY (skill_id, harness_id, workspace_id),
                 FOREIGN KEY (workspace_id) REFERENCES workspaces(workspace_id) ON DELETE CASCADE
             );
             "#,
        )
        .map_err(|source| PersistenceError::database("create_deployment_tables", source))?;
    Ok(())
}

fn create_deployment_indexes(transaction: &Transaction<'_>) -> Result<(), PersistenceError> {
    transaction
        .execute_batch(
            r#"
             CREATE UNIQUE INDEX one_agents_workspace
                 ON workspaces(kind) WHERE kind = 'agents';
             CREATE INDEX deployment_bindings_workspace
                 ON deployment_bindings(workspace_id);
             CREATE INDEX deployment_bindings_target
                 ON deployment_bindings(target_path);
             "#,
        )
        .map_err(|source| PersistenceError::database("create_deployment_indexes", source))?;
    Ok(())
}

fn encode_workspace_kind(kind: &WorkspaceKind) -> (&'static str, Option<Vec<u8>>, Option<Vec<u8>>) {
    match kind {
        WorkspaceKind::Agents => ("agents", None, None),
        WorkspaceKind::Project { root } => ("project", Some(encode_path(root)), None),
        WorkspaceKind::Linked {
            root,
            disabled_root,
        } => (
            "linked",
            Some(encode_path(root)),
            disabled_root.as_deref().map(encode_path),
        ),
    }
}

fn encode_deployment_mode(mode: DeploymentMode) -> &'static str {
    match mode {
        DeploymentMode::Copy => "copy",
        DeploymentMode::Link => "link",
    }
}

fn decode_deployment_mode(value: &str) -> Result<DeploymentMode, PersistenceError> {
    match value {
        "copy" => Ok(DeploymentMode::Copy),
        "link" => Ok(DeploymentMode::Link),
        _ => Err(PersistenceError::InvalidData {
            entity: "persisted deployment",
            field: "deployment_mode",
        }),
    }
}

fn snapshot_from_scanned(
    skill_id: SkillId,
    source: SkillSource,
    scanned: ScannedSkill,
) -> CentralSkillSnapshot {
    CentralSkillSnapshot {
        installed: InstalledSkill {
            id: skill_id,
            metadata: scanned.document.metadata().clone(),
            location: scanned.path,
            source,
            content_hash: scanned.content_hash,
        },
        version: SkillVersion {
            content_hash: scanned.content_hash,
            marker_modified_at: scanned.marker_modified_at,
        },
    }
}

fn snapshot_from_scanned_ref(
    skill_id: SkillId,
    source: SkillSource,
    scanned: &ScannedSkill,
) -> CentralSkillSnapshot {
    CentralSkillSnapshot {
        installed: InstalledSkill {
            id: skill_id,
            metadata: scanned.document.metadata().clone(),
            location: scanned.path.clone(),
            source,
            content_hash: scanned.content_hash,
        },
        version: SkillVersion {
            content_hash: scanned.content_hash,
            marker_modified_at: scanned.marker_modified_at,
        },
    }
}

fn snapshot_from_indexed(indexed: IndexedSkill) -> CentralSkillSnapshot {
    CentralSkillSnapshot {
        installed: InstalledSkill {
            id: indexed.id,
            metadata: indexed.metadata,
            location: indexed.path.clone(),
            source: SkillSource::Local { path: indexed.path },
            content_hash: indexed.content_hash,
        },
        version: SkillVersion {
            content_hash: indexed.content_hash,
            marker_modified_at: indexed.marker_modified_at,
        },
    }
}

fn decode_required_path(
    value: Option<&[u8]>,
    entity: &'static str,
    field: &'static str,
) -> Result<PathBuf, PersistenceError> {
    value
        .ok_or(PersistenceError::InvalidData { entity, field })
        .and_then(|value| decode_path(value, entity, field))
}

fn validate_absolute_path(
    path: &Path,
    entity: &'static str,
    field: &'static str,
) -> Result<(), PersistenceError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(PersistenceError::InvalidData { entity, field });
    }
    Ok(())
}

fn ensure_catalog_directories(catalog_root: &Path) -> Result<(), PersistenceError> {
    let cache = catalog_root.join(CACHE_DIRECTORY_NAME);
    fs::create_dir_all(&cache)
        .map_err(|source| PersistenceError::io("create_cache_directory", &cache, source))?;
    let skills = catalog_root.join(SKILLS_DIRECTORY_NAME);
    fs::create_dir_all(&skills)
        .map_err(|source| PersistenceError::io("create_skills_directory", skills, source))
}

fn directory_is_empty(path: &Path) -> Result<bool, PersistenceError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| PersistenceError::io("read_catalog_directory", path, source))?;
    match entries.next() {
        Some(Ok(_)) => Ok(false),
        Some(Err(source)) => Err(PersistenceError::io("read_catalog_entry", path, source)),
        None => Ok(true),
    }
}

fn remove_path_if_exists(path: &Path, operation: &'static str) -> Result<(), PersistenceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(PersistenceError::io(operation, path, source)),
    };

    let result = if metadata.file_type().is_symlink() {
        fs::remove_dir(path)
            .or_else(|directory_error| fs::remove_file(path).map_err(|_| directory_error))
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|source| PersistenceError::io(operation, path, source))
}

fn unix_timestamp() -> Result<i64, PersistenceError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PersistenceError::Clock)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| PersistenceError::Clock)
}

#[cfg(unix)]
fn encode_path(path: &Path) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(path.as_os_str().as_bytes().len() + 1);
    encoded.push(1);
    encoded.extend_from_slice(path.as_os_str().as_bytes());
    encoded
}

#[cfg(windows)]
fn encode_path(path: &Path) -> Vec<u8> {
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let mut encoded = Vec::with_capacity(units.len() * 2 + 1);
    encoded.push(2);
    for unit in units {
        encoded.extend_from_slice(&unit.to_le_bytes());
    }
    encoded
}

#[cfg(not(any(unix, windows)))]
fn encode_path(path: &Path) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(path.as_os_str().to_string_lossy().len() + 1);
    encoded.push(3);
    encoded.extend_from_slice(path.as_os_str().to_string_lossy().as_bytes());
    encoded
}

fn decode_path(
    encoded: &[u8],
    entity: &'static str,
    field: &'static str,
) -> Result<PathBuf, PersistenceError> {
    let Some((tag, payload)) = encoded.split_first() else {
        return Err(PersistenceError::InvalidData { entity, field });
    };
    decode_path_payload(*tag, payload).ok_or(PersistenceError::InvalidData { entity, field })
}

#[cfg(unix)]
fn decode_path_payload(tag: u8, payload: &[u8]) -> Option<PathBuf> {
    (tag == 1).then(|| PathBuf::from(OsString::from_vec(payload.to_vec())))
}

#[cfg(windows)]
fn decode_path_payload(tag: u8, payload: &[u8]) -> Option<PathBuf> {
    if tag != 2 || !payload.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = payload
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Some(PathBuf::from(OsString::from_wide(&units)))
}

#[cfg(not(any(unix, windows)))]
fn decode_path_payload(tag: u8, payload: &[u8]) -> Option<PathBuf> {
    if tag != 3 {
        return None;
    }
    String::from_utf8(payload.to_vec()).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skill_workspace::CentralCatalogPort;
    use tempfile::tempdir;

    use super::*;

    fn write_skill(path: &Path, name: &str, body: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n{body}\n"),
        )
        .unwrap();
    }

    #[test]
    fn schema_and_workspaces_survive_reopen_without_version_metadata() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let project_root = root.path().join("project");
        fs::create_dir_all(&project_root).unwrap();

        let project = StoredWorkspace {
            name: "Test project".to_owned(),
            workspace: Workspace {
                id: WorkspaceId::new(),
                kind: WorkspaceKind::Project {
                    root: project_root.clone(),
                },
            },
            deployment_mode: DeploymentMode::Copy,
        };
        {
            let mut catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();
            catalog.insert_workspace(&project).unwrap();
            assert_eq!(catalog.list_workspaces().unwrap().len(), 2);
        }

        let catalog = PersistentCatalog::open(&database, root.path().join("ignored")).unwrap();
        let restored = catalog.workspace(project.workspace.id).unwrap();
        assert_eq!(restored.name, project.name);
        assert_eq!(restored.workspace.kind, project.workspace.kind);
        assert_eq!(catalog.catalog_root(), catalog_root);
    }

    #[test]
    fn existing_state_schema_adds_skill_set_tables_in_place() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let mut connection = Connection::open(&database).unwrap();
        let transaction = connection.transaction().unwrap();
        create_schema(&transaction).unwrap();
        transaction.commit().unwrap();
        drop(connection);

        let catalog = PersistentCatalog::open(&database, root.path().join("catalog")).unwrap();
        assert!(catalog.list_skill_sets().unwrap().is_empty());
        drop(catalog);

        let connection = Connection::open(&database).unwrap();
        for table in ["skill_sets", "skill_set_members"] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "missing migrated table {table}");
        }
    }

    #[test]
    fn catalog_skills_are_indexed_from_the_filesystem_without_a_state_database_table() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let skill_path = catalog_root.join("skills/filesystem-skill");
        let mut catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();
        write_skill(&skill_path, "filesystem-skill", "filesystem body");
        catalog
            .rebuild_catalog_index(&AtomicBool::new(false))
            .unwrap();

        let skills = catalog.list_catalog_skills().unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].installed.id,
            SkillId::from_directory_name(std::ffi::OsStr::new("filesystem-skill"))
        );
        assert_eq!(skills[0].installed.location, skill_path);
        let has_catalog_table: bool = catalog
            .connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'table' AND name = 'catalog_skills'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!has_catalog_table);
    }

    #[test]
    fn imported_skill_and_binding_survive_reopen() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let source = root.path().join("source-skill");
        let target = root.path().join("target/skill");
        write_skill(&source, "persistent-skill", "first body");
        let scanned = read_skill(&source).unwrap();

        let (skill_id, agents_id) = {
            let mut catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();
            let agents = catalog.ensure_agents_workspace().unwrap();
            let imported = catalog.import_local(&scanned).unwrap();
            let skill_id = imported.installed.id;
            catalog
                .associate(DeploymentBinding {
                    key: DeploymentKey {
                        skill_id,
                        harness_id: HarnessId::new("test-harness").unwrap(),
                        workspace_id: agents.workspace.id,
                    },
                    target_path: target.clone(),
                    deployment_mode: DeploymentMode::Copy,
                })
                .unwrap();
            (skill_id, agents.workspace.id)
        };

        let catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();
        let skills = catalog.list_catalog_skills().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].installed.id, skill_id);
        assert_eq!(
            skills[0].installed.location,
            catalog_root.join("skills/source-skill")
        );
        assert_eq!(
            skills[0].installed.source,
            SkillSource::Local {
                path: catalog_root.join("skills/source-skill")
            }
        );
        assert!(source.exists());
        let bindings = catalog.bindings(agents_id).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].target_path, target);
        assert_eq!(catalog.activity_since(0).unwrap().len(), 1);
    }

    #[test]
    fn import_rejects_a_different_skill_with_the_same_directory_name() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let first = root.path().join("first/shared-name");
        let second = root.path().join("second/shared-name");
        write_skill(&first, "first", "first body");
        write_skill(&second, "second", "second body");
        let mut catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();

        catalog.import_local(&read_skill(&first).unwrap()).unwrap();
        let error = catalog
            .import_local(&read_skill(&second).unwrap())
            .unwrap_err();

        assert!(matches!(error, CatalogFailure::Conflict { .. }));
        let stored = read_skill(&catalog_root.join("skills/shared-name")).unwrap();
        assert_eq!(stored.document.metadata().name(), "first");
    }

    #[test]
    fn update_remains_committed_when_activity_recording_fails() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let original = root.path().join("original");
        let updated_source = root.path().join("updated");
        write_skill(&original, "activity-test", "original body");
        write_skill(&updated_source, "activity-test", "updated body");
        let mut catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();
        let imported = catalog
            .import_local(&read_skill(&original).unwrap())
            .unwrap();
        catalog
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_update_activity
                 BEFORE INSERT ON catalog_activity
                 WHEN NEW.kind = 'updated'
                 BEGIN
                     SELECT RAISE(FAIL, 'recording disabled');
                 END;",
            )
            .unwrap();

        let result = catalog
            .update_from_local(
                &imported.installed.id,
                &read_skill(&updated_source).unwrap(),
            )
            .unwrap();

        assert_eq!(result.installed.metadata.name(), "activity-test");
        let (_, persisted) = catalog.catalog_skill(imported.installed.id).unwrap();
        assert!(persisted.document.body().contains("updated body"));
        assert_eq!(catalog.activity_since(0).unwrap().len(), 1);
    }

    #[test]
    fn cache_preserves_unmanaged_entries() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let catalog_root = root.path().join("catalog");
        let unmanaged = catalog_root
            .join(CACHE_DIRECTORY_NAME)
            .join("not-owned-by-yssskills");
        fs::create_dir_all(&unmanaged).unwrap();
        fs::write(unmanaged.join("data.txt"), "keep me").unwrap();

        let _catalog = PersistentCatalog::open(&database, &catalog_root).unwrap();

        assert_eq!(
            fs::read_to_string(unmanaged.join("data.txt")).unwrap(),
            "keep me"
        );
    }

    #[test]
    fn catalog_root_can_only_change_while_empty() {
        let root = tempdir().unwrap();
        let database = root.path().join("state.sqlite3");
        let first_root = root.path().join("first");
        let second_root = root.path().join("second");
        fs::create_dir_all(&second_root).unwrap();
        let mut catalog = PersistentCatalog::open(&database, &first_root).unwrap();

        catalog.set_catalog_root(second_root.clone()).unwrap();
        assert_eq!(catalog.catalog_root(), second_root);

        let source = root.path().join("source");
        write_skill(&source, "root-test", "body");
        catalog.import_local(&read_skill(&source).unwrap()).unwrap();
        assert!(matches!(
            catalog.set_catalog_root(first_root),
            Err(PersistenceError::CatalogNotEmpty)
        ));
    }
}
