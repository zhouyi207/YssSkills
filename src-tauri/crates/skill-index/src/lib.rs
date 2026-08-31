use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use skill_core::{ContentHash, SkillId, SkillMarker, SkillMetadata};
use skill_local::{
    inspect_flat_skill_directory, inspect_skill_filesystem, read_skill, FilesystemFingerprint,
    LocalError, ScannedSkill, SkillFilesystemStamp,
};
use thiserror::Error;

const APPLICATION_ID: i64 = 0x5953_5349;
const SCHEMA_VERSION: i64 = 1;
const PARSE_VERSION: i64 = 1;
const MAX_RECONCILE_RETRIES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    Uninitialized,
    Ready,
    Reconciling,
    Stale,
}

impl IndexState {
    fn encode(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Ready => "ready",
            Self::Reconciling => "reconciling",
            Self::Stale => "stale",
        }
    }

    fn decode(value: &str) -> Result<Self, IndexError> {
        match value {
            "uninitialized" => Ok(Self::Uninitialized),
            "ready" => Ok(Self::Ready),
            "reconciling" => Ok(Self::Reconciling),
            "stale" => Ok(Self::Stale),
            _ => Err(IndexError::InvalidData {
                entity: "skill_index_meta",
                field: "state",
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedSkill {
    pub id: SkillId,
    pub path: PathBuf,
    pub metadata: SkillMetadata,
    pub content_hash: ContentHash,
    pub marker_modified_at: Option<SystemTime>,
    pub indexed_at_epoch_millis: i64,
}

#[derive(Debug, Clone)]
pub struct IndexDiagnostic {
    pub skill_id: SkillId,
    pub path: PathBuf,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct IndexSnapshot {
    pub skills: Vec<IndexedSkill>,
    pub diagnostics: Vec<IndexDiagnostic>,
    pub state: IndexState,
    pub revision: i64,
    pub last_reconciled_at_epoch_millis: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub inserted: Vec<SkillId>,
    pub updated: Vec<SkillId>,
    pub removed: Vec<SkillId>,
    pub unchanged: Vec<SkillId>,
    pub invalid: Vec<SkillId>,
}

#[derive(Debug, Clone)]
pub struct IndexOpenStatus {
    pub needs_rebuild: bool,
    pub recovered_from: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("skill index database operation {operation} failed")]
    Database {
        operation: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("skill index filesystem operation {operation} failed for {path:?}")]
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
    #[error("persisted {entity} field {field} is invalid")]
    InvalidData {
        entity: &'static str,
        field: &'static str,
    },
    #[error("skill index schema is incompatible")]
    IncompatibleSchema,
    #[error("skill index integrity check failed")]
    InvalidSchema,
    #[error("skill index changed while filesystem reconciliation was in progress")]
    ConcurrentModification,
    #[error("skill index reconciliation was cancelled")]
    Cancelled,
}

impl IndexError {
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

    fn is_recoverable_database_damage(&self) -> bool {
        match self {
            Self::IncompatibleSchema | Self::InvalidSchema | Self::InvalidData { .. } => true,
            Self::Database { operation, source } => {
                if matches!(
                    source.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase)
                ) {
                    return true;
                }
                matches!(
                    *operation,
                    "validate_meta_table"
                        | "validate_entries_table"
                        | "load_meta"
                        | "prepare_cached_snapshot"
                        | "query_cached_snapshot"
                        | "decode_cached_row"
                ) && !matches!(
                    source.sqlite_error_code(),
                    Some(
                        rusqlite::ErrorCode::PermissionDenied
                            | rusqlite::ErrorCode::OperationAborted
                            | rusqlite::ErrorCode::DatabaseBusy
                            | rusqlite::ErrorCode::DatabaseLocked
                            | rusqlite::ErrorCode::OutOfMemory
                            | rusqlite::ErrorCode::ReadOnly
                            | rusqlite::ErrorCode::OperationInterrupted
                            | rusqlite::ErrorCode::SystemIoFailure
                            | rusqlite::ErrorCode::DiskFull
                            | rusqlite::ErrorCode::CannotOpen
                            | rusqlite::ErrorCode::FileLockingProtocolFailed
                    )
                )
            }
            _ => false,
        }
    }
}

pub struct SkillIndex {
    connection: Connection,
    database_path: PathBuf,
}

impl SkillIndex {
    pub fn open(database_path: impl AsRef<Path>) -> Result<(Self, IndexOpenStatus), IndexError> {
        let database_path = database_path.as_ref().to_path_buf();
        ensure_database_parent(&database_path)?;
        let existed = database_path.exists();

        match Self::open_checked(database_path.clone(), existed) {
            Ok((index, needs_rebuild)) => Ok((
                index,
                IndexOpenStatus {
                    needs_rebuild,
                    recovered_from: None,
                },
            )),
            Err(error) if existed && error.is_recoverable_database_damage() => {
                let backup = move_unusable_database(&database_path)?;
                let (index, _) = Self::open_checked(database_path, false)?;
                Ok((
                    index,
                    IndexOpenStatus {
                        needs_rebuild: true,
                        recovered_from: Some(backup),
                    },
                ))
            }
            Err(error) => Err(error),
        }
    }

    fn open_checked(database_path: PathBuf, existed: bool) -> Result<(Self, bool), IndexError> {
        let connection = Connection::open(&database_path)
            .map_err(|source| IndexError::database("open", source))?;
        configure_connection(&connection)?;
        verify_integrity(&connection)?;

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(|source| IndexError::database("inspect_schema_tables", source))?;
        if !existed || table_count == 0 {
            create_schema(&connection)?;
        } else {
            validate_schema(&connection)?;
        }

        let index = Self {
            connection,
            database_path,
        };
        let meta = index.load_meta()?;
        // Decode every persisted record while opening. This is still an SQLite-only read,
        // and it turns logical cache corruption into a disposable-index rebuild instead of
        // allowing one malformed row to break every later list query.
        let _validated_snapshot = index.load_cached_snapshot()?;
        Ok((index, meta.state == IndexState::Uninitialized))
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn matches_root(&self, skills_root: &Path) -> Result<bool, IndexError> {
        let Some(indexed_root) = self.load_meta()?.skills_root else {
            return Ok(false);
        };
        let normalized_root = fs::canonicalize(skills_root)
            .map_err(|source| IndexError::io("normalize_skills_root", skills_root, source))?;
        Ok(indexed_root == normalized_root)
    }

    pub fn snapshot(&self) -> Result<IndexSnapshot, IndexError> {
        let meta = self.load_meta()?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT skill_id, path, name, description, version, content_hash,
                        marker_modified_at, indexed_at, state, error_kind, error_message
                 FROM skill_index_entries
                 ORDER BY CASE state WHEN 'valid' THEN 0 ELSE 1 END,
                          name COLLATE NOCASE, skill_id",
            )
            .map_err(|source| IndexError::database("prepare_snapshot", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(SnapshotRow {
                    skill_id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    version: row.get(4)?,
                    content_hash: row.get(5)?,
                    marker_modified_at: row.get(6)?,
                    indexed_at: row.get(7)?,
                    state: row.get(8)?,
                    error_kind: row.get(9)?,
                    error_message: row.get(10)?,
                })
            })
            .map_err(|source| IndexError::database("query_snapshot", source))?;

        let mut skills = Vec::new();
        let mut diagnostics = Vec::new();
        for row in rows {
            let row = row.map_err(|source| IndexError::database("decode_snapshot_row", source))?;
            match row.decode()? {
                SnapshotEntry::Valid(skill) => skills.push(skill),
                SnapshotEntry::Invalid(diagnostic) => diagnostics.push(diagnostic),
            }
        }

        Ok(IndexSnapshot {
            skills,
            diagnostics,
            state: meta.state,
            revision: meta.revision,
            last_reconciled_at_epoch_millis: meta.last_reconciled_at_epoch_millis,
        })
    }

    pub fn skill(&self, skill_id: SkillId) -> Result<Option<IndexedSkill>, IndexError> {
        let row = self
            .connection
            .query_row(
                "SELECT skill_id, path, name, description, version, content_hash,
                        marker_modified_at, indexed_at, state, error_kind, error_message
                 FROM skill_index_entries WHERE skill_id = ?1",
                [skill_id.to_string()],
                |row| {
                    Ok(SnapshotRow {
                        skill_id: row.get(0)?,
                        path: row.get(1)?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                        version: row.get(4)?,
                        content_hash: row.get(5)?,
                        marker_modified_at: row.get(6)?,
                        indexed_at: row.get(7)?,
                        state: row.get(8)?,
                        error_kind: row.get(9)?,
                        error_message: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(|source| IndexError::database("query_skill", source))?;
        match row.map(SnapshotRow::decode).transpose()? {
            Some(SnapshotEntry::Valid(skill)) => Ok(Some(skill)),
            Some(SnapshotEntry::Invalid(_)) | None => Ok(None),
        }
    }

    pub fn reconcile(
        &mut self,
        skills_root: &Path,
        cancellation: &AtomicBool,
    ) -> Result<ReconcileReport, IndexError> {
        self.run_reconcile(skills_root, false, cancellation)
    }

    pub fn rebuild(
        &mut self,
        skills_root: &Path,
        cancellation: &AtomicBool,
    ) -> Result<ReconcileReport, IndexError> {
        self.run_reconcile(skills_root, true, cancellation)
    }

    fn run_reconcile(
        &mut self,
        skills_root: &Path,
        rebuild: bool,
        cancellation: &AtomicBool,
    ) -> Result<ReconcileReport, IndexError> {
        let mut last_conflict = None;
        for _ in 0..MAX_RECONCILE_RETRIES {
            ensure_not_cancelled(cancellation)?;
            let cached = if rebuild {
                CachedSnapshot {
                    revision: self.load_meta()?.revision,
                    entries: HashMap::new(),
                }
            } else {
                self.load_cached_snapshot()?
            };
            match self.mark_reconciling(cached.revision) {
                Ok(()) => {}
                Err(IndexError::ConcurrentModification) => {
                    last_conflict = Some(IndexError::ConcurrentModification);
                    continue;
                }
                Err(error) => return Err(error),
            }
            let prepared = match prepare_reconcile(skills_root, &cached, cancellation) {
                Ok(prepared) => prepared,
                Err(IndexError::ConcurrentModification) => {
                    self.mark_stale_if_revision(cached.revision)?;
                    last_conflict = Some(IndexError::ConcurrentModification);
                    continue;
                }
                Err(error) => {
                    self.mark_stale_if_revision(cached.revision)?;
                    return Err(error);
                }
            };
            match self.commit_reconcile(cached.revision, prepared, rebuild) {
                Ok(report) => return Ok(report),
                Err(IndexError::ConcurrentModification) => {
                    last_conflict = Some(IndexError::ConcurrentModification);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_conflict.unwrap_or(IndexError::ConcurrentModification))
    }

    pub fn refresh_skill(
        &mut self,
        skill_id: SkillId,
        path: &Path,
    ) -> Result<IndexedSkill, IndexError> {
        let stamp = inspect_skill_filesystem(path)
            .map_err(|source| IndexError::local("inspect_skill", source))?;
        let scanned = read_skill(path).map_err(|source| IndexError::local("read_skill", source))?;
        let verified_stamp = inspect_skill_filesystem(path)
            .map_err(|source| IndexError::local("verify_skill_stamp", source))?;
        if stamp != verified_stamp {
            return Err(IndexError::ConcurrentModification);
        }
        let record = valid_record(skill_id, verified_stamp, scanned)?;
        let indexed = record.as_indexed_skill()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| IndexError::database("begin_refresh_skill", source))?;
        upsert_record(&transaction, &record)?;
        advance_revision(&transaction, None, IndexState::Stale, None)?;
        transaction
            .commit()
            .map_err(|source| IndexError::database("commit_refresh_skill", source))?;
        Ok(indexed)
    }

    pub fn remove_skill(&mut self, skill_id: SkillId) -> Result<(), IndexError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| IndexError::database("begin_remove_skill", source))?;
        transaction
            .execute(
                "DELETE FROM skill_index_entries WHERE skill_id = ?1",
                [skill_id.to_string()],
            )
            .map_err(|source| IndexError::database("remove_skill", source))?;
        advance_revision(&transaction, None, IndexState::Stale, None)?;
        transaction
            .commit()
            .map_err(|source| IndexError::database("commit_remove_skill", source))?;
        Ok(())
    }

    pub fn mark_stale(&self) -> Result<(), IndexError> {
        self.connection
            .execute(
                "UPDATE skill_index_meta SET state = 'stale' WHERE singleton = 1",
                [],
            )
            .map_err(|source| IndexError::database("mark_stale", source))?;
        Ok(())
    }

    fn load_meta(&self) -> Result<IndexMeta, IndexError> {
        self.connection
            .query_row(
                "SELECT revision, state, last_reconciled_at, parse_version
                        , skills_root
                 FROM skill_index_meta WHERE singleton = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<Vec<u8>>>(4)?,
                    ))
                },
            )
            .map_err(|source| IndexError::database("load_meta", source))
            .and_then(
                |(revision, state, last_reconciled_at_epoch_millis, parse_version, skills_root)| {
                    if parse_version != PARSE_VERSION {
                        return Err(IndexError::IncompatibleSchema);
                    }
                    Ok(IndexMeta {
                        revision,
                        state: IndexState::decode(&state)?,
                        last_reconciled_at_epoch_millis,
                        skills_root: skills_root.as_deref().map(decode_path).transpose()?,
                    })
                },
            )
    }

    fn load_cached_snapshot(&self) -> Result<CachedSnapshot, IndexError> {
        let meta = self.load_meta()?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT skill_id, path, normalized_path, marker, marker_path,
                        marker_modified_at, marker_file_size, filesystem_fingerprint,
                        name, description, version, content_hash, state, error_kind,
                        error_message, indexed_at, parse_version
                 FROM skill_index_entries",
            )
            .map_err(|source| IndexError::database("prepare_cached_snapshot", source))?;
        let rows = statement
            .query_map([], |row| {
                Ok(RecordRow {
                    skill_id: row.get(0)?,
                    path: row.get(1)?,
                    normalized_path: row.get(2)?,
                    marker: row.get(3)?,
                    marker_path: row.get(4)?,
                    marker_modified_at: row.get(5)?,
                    marker_file_size: row.get(6)?,
                    filesystem_fingerprint: row.get(7)?,
                    name: row.get(8)?,
                    description: row.get(9)?,
                    version: row.get(10)?,
                    content_hash: row.get(11)?,
                    state: row.get(12)?,
                    error_kind: row.get(13)?,
                    error_message: row.get(14)?,
                    indexed_at: row.get(15)?,
                    parse_version: row.get(16)?,
                })
            })
            .map_err(|source| IndexError::database("query_cached_snapshot", source))?;
        let mut entries = HashMap::new();
        for row in rows {
            let row = row.map_err(|source| IndexError::database("decode_cached_row", source))?;
            let record = row.decode()?;
            entries.insert(record.skill_id, record);
        }
        Ok(CachedSnapshot {
            revision: meta.revision,
            entries,
        })
    }

    fn mark_reconciling(&self, revision: i64) -> Result<(), IndexError> {
        let changed = self
            .connection
            .execute(
                "UPDATE skill_index_meta SET state = 'reconciling'
                 WHERE singleton = 1 AND revision = ?1",
                [revision],
            )
            .map_err(|source| IndexError::database("mark_reconciling", source))?;
        if changed == 1 {
            Ok(())
        } else {
            Err(IndexError::ConcurrentModification)
        }
    }

    fn mark_stale_if_revision(&self, revision: i64) -> Result<(), IndexError> {
        self.connection
            .execute(
                "UPDATE skill_index_meta SET state = 'stale'
                 WHERE singleton = 1 AND revision = ?1",
                [revision],
            )
            .map_err(|source| IndexError::database("mark_reconcile_stale", source))?;
        Ok(())
    }

    fn commit_reconcile(
        &mut self,
        base_revision: i64,
        prepared: PreparedReconcile,
        rebuild: bool,
    ) -> Result<ReconcileReport, IndexError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| IndexError::database("begin_reconcile_commit", source))?;
        let current_revision: i64 = transaction
            .query_row(
                "SELECT revision FROM skill_index_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|source| IndexError::database("verify_reconcile_revision", source))?;
        if current_revision != base_revision {
            return Err(IndexError::ConcurrentModification);
        }

        if rebuild {
            transaction
                .execute("DELETE FROM skill_index_entries", [])
                .map_err(|source| IndexError::database("clear_rebuilt_index", source))?;
        } else {
            for skill_id in &prepared.report.removed {
                transaction
                    .execute(
                        "DELETE FROM skill_index_entries WHERE skill_id = ?1",
                        [skill_id.to_string()],
                    )
                    .map_err(|source| IndexError::database("remove_missing_skill", source))?;
            }
        }
        for record in &prepared.upserts {
            upsert_record(&transaction, record)?;
        }
        let reconciled_at = current_epoch_millis()?;
        advance_revision(
            &transaction,
            Some(base_revision),
            IndexState::Ready,
            Some(reconciled_at),
        )?;
        transaction
            .execute(
                "UPDATE skill_index_meta SET skills_root = ?1 WHERE singleton = 1",
                [encode_path(&prepared.normalized_skills_root)],
            )
            .map_err(|source| IndexError::database("record_skills_root", source))?;
        transaction
            .commit()
            .map_err(|source| IndexError::database("commit_reconcile", source))?;
        Ok(prepared.report)
    }
}

#[derive(Debug)]
struct IndexMeta {
    revision: i64,
    state: IndexState,
    last_reconciled_at_epoch_millis: Option<i64>,
    skills_root: Option<PathBuf>,
}

#[derive(Debug)]
struct CachedSnapshot {
    revision: i64,
    entries: HashMap<SkillId, IndexRecord>,
}

#[derive(Debug)]
struct PreparedReconcile {
    upserts: Vec<IndexRecord>,
    report: ReconcileReport,
    normalized_skills_root: PathBuf,
}

#[derive(Debug, Clone)]
struct IndexRecord {
    skill_id: SkillId,
    stamp: SkillFilesystemStamp,
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    content_hash: Option<ContentHash>,
    state: &'static str,
    error_kind: Option<String>,
    error_message: Option<String>,
    indexed_at_epoch_millis: i64,
    parse_version: i64,
}

impl IndexRecord {
    fn stamp_matches(&self, stamp: &SkillFilesystemStamp) -> bool {
        self.parse_version == PARSE_VERSION && self.stamp == *stamp
    }

    fn as_indexed_skill(&self) -> Result<IndexedSkill, IndexError> {
        if self.state != "valid" {
            return Err(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "state",
            });
        }
        let name = self.name.clone().ok_or(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "name",
        })?;
        let description = self.description.clone().ok_or(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "description",
        })?;
        let metadata = SkillMetadata::new_with_version(name, description, self.version.clone())
            .map_err(|_| IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "metadata",
            })?;
        let content_hash = self.content_hash.ok_or(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "content_hash",
        })?;
        Ok(IndexedSkill {
            id: self.skill_id,
            path: self.stamp.path.clone(),
            metadata,
            content_hash,
            marker_modified_at: self.stamp.marker_modified_at,
            indexed_at_epoch_millis: self.indexed_at_epoch_millis,
        })
    }
}

fn prepare_reconcile(
    skills_root: &Path,
    cached: &CachedSnapshot,
    cancellation: &AtomicBool,
) -> Result<PreparedReconcile, IndexError> {
    let inspection = inspect_flat_skill_directory(skills_root)
        .map_err(|source| IndexError::local("inspect_skills_root", source))?;
    let normalized_skills_root = fs::canonicalize(skills_root)
        .map_err(|source| IndexError::io("normalize_skills_root", skills_root, source))?;
    let mut seen = HashSet::new();
    let mut upserts = Vec::new();
    let mut report = ReconcileReport::default();

    for diagnostic in inspection.diagnostics {
        ensure_not_cancelled(cancellation)?;
        let Some(directory_name) = diagnostic.path.file_name() else {
            continue;
        };
        let skill_id = SkillId::from_directory_name(directory_name);
        seen.insert(skill_id);
        let fallback_stamp = fallback_stamp(&diagnostic.path)?;
        upserts.push(invalid_record(skill_id, fallback_stamp, diagnostic.error)?);
        if cached.entries.contains_key(&skill_id) {
            report.updated.push(skill_id);
        } else {
            report.inserted.push(skill_id);
        }
        report.invalid.push(skill_id);
    }

    for stamp in inspection.skills {
        ensure_not_cancelled(cancellation)?;
        let directory_name = stamp.path.file_name().ok_or(IndexError::InvalidData {
            entity: "filesystem_skill",
            field: "directory_name",
        })?;
        let skill_id = SkillId::from_directory_name(directory_name);
        seen.insert(skill_id);
        if cached
            .entries
            .get(&skill_id)
            .is_some_and(|entry| entry.stamp_matches(&stamp))
        {
            report.unchanged.push(skill_id);
            continue;
        }

        let record = read_stable_record(skill_id, stamp, cancellation)?;
        if record.state == "invalid" {
            report.invalid.push(skill_id);
        }
        if cached.entries.contains_key(&skill_id) {
            report.updated.push(skill_id);
        } else {
            report.inserted.push(skill_id);
        }
        upserts.push(record);
    }

    report.removed = cached
        .entries
        .keys()
        .filter(|skill_id| !seen.contains(skill_id))
        .copied()
        .collect();
    sort_report(&mut report);
    Ok(PreparedReconcile {
        upserts,
        report,
        normalized_skills_root,
    })
}

fn read_stable_record(
    skill_id: SkillId,
    mut stamp: SkillFilesystemStamp,
    cancellation: &AtomicBool,
) -> Result<IndexRecord, IndexError> {
    for _ in 0..MAX_RECONCILE_RETRIES {
        ensure_not_cancelled(cancellation)?;
        let read_result = read_skill(&stamp.path);
        let verified_stamp = match inspect_skill_filesystem(&stamp.path) {
            Ok(verified) => verified,
            Err(LocalError::PathNotFound { .. } | LocalError::MarkerNotFound { .. }) => {
                return Err(IndexError::ConcurrentModification)
            }
            Err(source) => return Err(IndexError::local("verify_skill_stamp", source)),
        };
        if stamp != verified_stamp {
            stamp = verified_stamp;
            continue;
        }
        return match read_result {
            Ok(scanned) => valid_record(skill_id, stamp, scanned),
            Err(source) => invalid_record(skill_id, stamp, source),
        };
    }
    Err(IndexError::ConcurrentModification)
}

fn valid_record(
    skill_id: SkillId,
    stamp: SkillFilesystemStamp,
    scanned: ScannedSkill,
) -> Result<IndexRecord, IndexError> {
    let indexed_at_epoch_millis = current_epoch_millis()?;
    Ok(IndexRecord {
        skill_id,
        stamp,
        name: Some(scanned.document.metadata().name().to_owned()),
        description: Some(scanned.document.metadata().description().to_owned()),
        version: scanned.document.metadata().version().map(ToOwned::to_owned),
        content_hash: Some(scanned.content_hash),
        state: "valid",
        error_kind: None,
        error_message: None,
        indexed_at_epoch_millis,
        parse_version: PARSE_VERSION,
    })
}

fn invalid_record(
    skill_id: SkillId,
    stamp: SkillFilesystemStamp,
    error: LocalError,
) -> Result<IndexRecord, IndexError> {
    let indexed_at_epoch_millis = current_epoch_millis()?;
    Ok(IndexRecord {
        skill_id,
        stamp,
        name: None,
        description: None,
        version: None,
        content_hash: None,
        state: "invalid",
        error_kind: Some(local_error_kind(&error).to_owned()),
        error_message: Some(error.to_string()),
        indexed_at_epoch_millis,
        parse_version: PARSE_VERSION,
    })
}

fn fallback_stamp(path: &Path) -> Result<SkillFilesystemStamp, IndexError> {
    let normalized_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(SkillFilesystemStamp {
        path: path.to_path_buf(),
        normalized_path,
        marker: SkillMarker::Canonical,
        marker_path: path.join(SkillMarker::Canonical.file_name()),
        marker_modified_at: None,
        marker_file_size: 0,
        fingerprint: FilesystemFingerprint::from_bytes([0; 32]),
    })
}

fn local_error_kind(error: &LocalError) -> &'static str {
    match error {
        LocalError::Parse { .. } => "parse",
        LocalError::MarkerNotFound { .. } => "marker_not_found",
        LocalError::PathNotFound { .. } => "path_not_found",
        LocalError::NotDirectory { .. } => "not_directory",
        LocalError::InvalidPathEncoding { .. } | LocalError::InvalidPath { .. } => "invalid_path",
        LocalError::Io { .. } | LocalError::Walk { .. } => "filesystem",
        _ => "local_operation",
    }
}

fn sort_report(report: &mut ReconcileReport) {
    for skill_ids in [
        &mut report.inserted,
        &mut report.updated,
        &mut report.removed,
        &mut report.unchanged,
        &mut report.invalid,
    ] {
        skill_ids.sort_by_key(ToString::to_string);
        skill_ids.dedup();
    }
}

fn ensure_not_cancelled(cancellation: &AtomicBool) -> Result<(), IndexError> {
    if cancellation.load(Ordering::Acquire) {
        Err(IndexError::Cancelled)
    } else {
        Ok(())
    }
}

fn upsert_record(transaction: &Transaction<'_>, record: &IndexRecord) -> Result<(), IndexError> {
    let marker_file_size =
        i64::try_from(record.stamp.marker_file_size).map_err(|_| IndexError::InvalidData {
            entity: "filesystem_skill",
            field: "marker_file_size",
        })?;
    transaction
        .execute(
            "INSERT INTO skill_index_entries (
                 skill_id, path, normalized_path, marker, marker_path, marker_modified_at,
                 marker_file_size, filesystem_fingerprint, name, description, version,
                 content_hash, state, error_kind, error_message, indexed_at, parse_version
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
             )
             ON CONFLICT(skill_id) DO UPDATE SET
                 path = excluded.path,
                 normalized_path = excluded.normalized_path,
                 marker = excluded.marker,
                 marker_path = excluded.marker_path,
                 marker_modified_at = excluded.marker_modified_at,
                 marker_file_size = excluded.marker_file_size,
                 filesystem_fingerprint = excluded.filesystem_fingerprint,
                 name = excluded.name,
                 description = excluded.description,
                 version = excluded.version,
                 content_hash = excluded.content_hash,
                 state = excluded.state,
                 error_kind = excluded.error_kind,
                 error_message = excluded.error_message,
                 indexed_at = excluded.indexed_at,
                 parse_version = excluded.parse_version",
            params![
                record.skill_id.to_string(),
                encode_path(&record.stamp.path),
                encode_path(&record.stamp.normalized_path),
                encode_marker(record.stamp.marker),
                encode_path(&record.stamp.marker_path),
                record.stamp.marker_modified_at.map(encode_system_time),
                marker_file_size,
                record.stamp.fingerprint.as_bytes().as_slice(),
                record.name,
                record.description,
                record.version,
                record.content_hash.map(ContentHash::to_hex),
                record.state,
                record.error_kind,
                record.error_message,
                record.indexed_at_epoch_millis,
                record.parse_version,
            ],
        )
        .map_err(|source| IndexError::database("upsert_skill", source))?;
    Ok(())
}

fn advance_revision(
    transaction: &Transaction<'_>,
    expected_revision: Option<i64>,
    state: IndexState,
    reconciled_at: Option<i64>,
) -> Result<(), IndexError> {
    let changed = match expected_revision {
        Some(expected_revision) => transaction.execute(
            "UPDATE skill_index_meta
             SET revision = revision + 1, state = ?1, last_reconciled_at = ?2,
                 parse_version = ?3
             WHERE singleton = 1 AND revision = ?4",
            params![
                state.encode(),
                reconciled_at,
                PARSE_VERSION,
                expected_revision
            ],
        ),
        None => transaction.execute(
            "UPDATE skill_index_meta
             SET revision = revision + 1, state = ?1, parse_version = ?2
             WHERE singleton = 1",
            params![state.encode(), PARSE_VERSION],
        ),
    }
    .map_err(|source| IndexError::database("advance_revision", source))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(IndexError::ConcurrentModification)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), IndexError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|source| IndexError::database("configure_busy_timeout", source))?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| IndexError::database("enable_wal", source))?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|source| IndexError::database("configure_synchronous", source))?;
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), IndexError> {
    let result: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|source| IndexError::database("check_integrity", source))?;
    if result == "ok" {
        Ok(())
    } else {
        Err(IndexError::InvalidSchema)
    }
}

fn validate_schema(connection: &Connection) -> Result<(), IndexError> {
    let application_id: i64 = connection
        .query_row("PRAGMA application_id", [], |row| row.get(0))
        .map_err(|source| IndexError::database("read_application_id", source))?;
    let schema_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| IndexError::database("read_schema_version", source))?;
    if application_id != APPLICATION_ID || schema_version != SCHEMA_VERSION {
        return Err(IndexError::IncompatibleSchema);
    }
    connection
        .query_row(
            "SELECT revision, state, parse_version, skills_root
             FROM skill_index_meta WHERE singleton = 1",
            [],
            |_| Ok(()),
        )
        .map_err(|source| IndexError::database("validate_meta_table", source))?;
    connection
        .query_row("SELECT COUNT(*) FROM skill_index_entries", [], |_| Ok(()))
        .map_err(|source| IndexError::database("validate_entries_table", source))?;
    Ok(())
}

fn create_schema(connection: &Connection) -> Result<(), IndexError> {
    let transaction = connection
        .unchecked_transaction()
        .map_err(|source| IndexError::database("begin_schema", source))?;
    transaction
        .execute_batch(
            r#"
            PRAGMA application_id = 1498633033;
            PRAGMA user_version = 1;

            CREATE TABLE skill_index_meta (
                singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
                revision INTEGER NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('uninitialized', 'ready', 'reconciling', 'stale')),
                last_reconciled_at INTEGER,
                skills_root BLOB,
                parse_version INTEGER NOT NULL
            );

            INSERT INTO skill_index_meta (
                singleton, revision, state, last_reconciled_at, skills_root, parse_version
            ) VALUES (1, 0, 'uninitialized', NULL, NULL, 1);

            CREATE TABLE skill_index_entries (
                skill_id TEXT PRIMARY KEY NOT NULL,
                path BLOB NOT NULL,
                normalized_path BLOB NOT NULL UNIQUE,
                marker TEXT NOT NULL CHECK (marker IN ('canonical', 'legacy')),
                marker_path BLOB NOT NULL,
                marker_modified_at BLOB,
                marker_file_size INTEGER NOT NULL CHECK (marker_file_size >= 0),
                filesystem_fingerprint BLOB NOT NULL,
                name TEXT,
                description TEXT,
                version TEXT,
                content_hash TEXT,
                state TEXT NOT NULL CHECK (state IN ('valid', 'invalid')),
                error_kind TEXT,
                error_message TEXT,
                indexed_at INTEGER NOT NULL,
                parse_version INTEGER NOT NULL,
                CHECK (
                    (state = 'valid' AND name IS NOT NULL AND description IS NOT NULL
                     AND content_hash IS NOT NULL AND error_kind IS NULL AND error_message IS NULL)
                    OR
                    (state = 'invalid' AND name IS NULL AND description IS NULL
                     AND content_hash IS NULL AND error_kind IS NOT NULL AND error_message IS NOT NULL)
                )
            );

            CREATE INDEX skill_index_valid_name
                ON skill_index_entries(state, name COLLATE NOCASE, skill_id);
            "#,
        )
        .map_err(|source| IndexError::database("create_schema", source))?;
    transaction
        .commit()
        .map_err(|source| IndexError::database("commit_schema", source))?;
    Ok(())
}

fn ensure_database_parent(database_path: &Path) -> Result<(), IndexError> {
    if let Some(parent) = database_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|source| IndexError::io("create_database_parent", parent, source))?;
    }
    Ok(())
}

fn move_unusable_database(database_path: &Path) -> Result<PathBuf, IndexError> {
    let backup = unused_backup_path(database_path)?;
    fs::rename(database_path, &backup)
        .map_err(|source| IndexError::io("backup_unusable_index", database_path, source))?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = append_to_path(database_path, suffix);
        if !sidecar.exists() {
            continue;
        }
        let backup_sidecar = append_to_path(&backup, suffix);
        fs::rename(&sidecar, &backup_sidecar)
            .map_err(|source| IndexError::io("backup_unusable_index_sidecar", sidecar, source))?;
    }
    Ok(backup)
}

fn unused_backup_path(database_path: &Path) -> Result<PathBuf, IndexError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    for suffix in 0_u32..=u32::MAX {
        let extension = if suffix == 0 {
            format!(".invalid-{timestamp}")
        } else {
            format!(".invalid-{timestamp}-{suffix}")
        };
        let candidate = append_to_path(database_path, &extension);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(IndexError::InvalidData {
        entity: "skill_index_backup",
        field: "path",
    })
}

fn append_to_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn current_epoch_millis() -> Result<i64, IndexError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IndexError::InvalidData {
            entity: "system_clock",
            field: "current_time",
        })?
        .as_millis();
    i64::try_from(millis).map_err(|_| IndexError::InvalidData {
        entity: "system_clock",
        field: "current_time",
    })
}

fn encode_marker(marker: SkillMarker) -> &'static str {
    match marker {
        SkillMarker::Canonical => "canonical",
        SkillMarker::Legacy => "legacy",
    }
}

fn decode_marker(value: &str) -> Result<SkillMarker, IndexError> {
    match value {
        "canonical" => Ok(SkillMarker::Canonical),
        "legacy" => Ok(SkillMarker::Legacy),
        _ => Err(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "marker",
        }),
    }
}

fn encode_system_time(value: SystemTime) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(13);
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            encoded.push(1);
            encoded.extend_from_slice(&duration.as_secs().to_le_bytes());
            encoded.extend_from_slice(&duration.subsec_nanos().to_le_bytes());
        }
        Err(error) => {
            let duration = error.duration();
            encoded.push(0);
            encoded.extend_from_slice(&duration.as_secs().to_le_bytes());
            encoded.extend_from_slice(&duration.subsec_nanos().to_le_bytes());
        }
    }
    encoded
}

fn decode_system_time(value: &[u8]) -> Result<SystemTime, IndexError> {
    if value.len() != 13 {
        return Err(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "marker_modified_at",
        });
    }
    let direction = value[0];
    let seconds =
        u64::from_le_bytes(
            value[1..9]
                .try_into()
                .map_err(|_| IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "marker_modified_at",
                })?,
        );
    let nanos =
        u32::from_le_bytes(
            value[9..13]
                .try_into()
                .map_err(|_| IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "marker_modified_at",
                })?,
        );
    if nanos >= 1_000_000_000 {
        return Err(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "marker_modified_at",
        });
    }
    let duration = Duration::new(seconds, nanos);
    match direction {
        1 => UNIX_EPOCH
            .checked_add(duration)
            .ok_or(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "marker_modified_at",
            }),
        0 => UNIX_EPOCH
            .checked_sub(duration)
            .ok_or(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "marker_modified_at",
            }),
        _ => Err(IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "marker_modified_at",
        }),
    }
}

fn encode_path(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        path.as_os_str().as_bytes().to_vec()
    }

    #[cfg(windows)]
    {
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[cfg(not(any(unix, windows)))]
    {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

fn decode_path(value: &[u8]) -> Result<PathBuf, IndexError> {
    #[cfg(unix)]
    {
        Ok(PathBuf::from(OsString::from_vec(value.to_vec())))
    }

    #[cfg(windows)]
    {
        if !value.len().is_multiple_of(2) {
            return Err(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "path",
            });
        }
        let wide = value
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        Ok(PathBuf::from(OsString::from_wide(&wide)))
    }

    #[cfg(not(any(unix, windows)))]
    {
        String::from_utf8(value.to_vec())
            .map(PathBuf::from)
            .map_err(|_| IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "path",
            })
    }
}

#[derive(Debug)]
struct SnapshotRow {
    skill_id: String,
    path: Vec<u8>,
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    content_hash: Option<String>,
    marker_modified_at: Option<Vec<u8>>,
    indexed_at: i64,
    state: String,
    error_kind: Option<String>,
    error_message: Option<String>,
}

enum SnapshotEntry {
    Valid(IndexedSkill),
    Invalid(IndexDiagnostic),
}

impl SnapshotRow {
    fn decode(self) -> Result<SnapshotEntry, IndexError> {
        let skill_id = SkillId::parse(&self.skill_id).map_err(|_| IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "skill_id",
        })?;
        let path = decode_path(&self.path)?;
        if self.state == "invalid" {
            return Ok(SnapshotEntry::Invalid(IndexDiagnostic {
                skill_id,
                path,
                kind: self.error_kind.ok_or(IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "error_kind",
                })?,
                message: self.error_message.ok_or(IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "error_message",
                })?,
            }));
        }
        if self.state != "valid" {
            return Err(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "state",
            });
        }
        let metadata = SkillMetadata::new_with_version(
            self.name.ok_or(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "name",
            })?,
            self.description.ok_or(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "description",
            })?,
            self.version,
        )
        .map_err(|_| IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "metadata",
        })?;
        let content_hash =
            ContentHash::from_hex(&self.content_hash.ok_or(IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "content_hash",
            })?)
            .map_err(|_| IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "content_hash",
            })?;
        let marker_modified_at = self
            .marker_modified_at
            .as_deref()
            .map(decode_system_time)
            .transpose()?;
        Ok(SnapshotEntry::Valid(IndexedSkill {
            id: skill_id,
            path,
            metadata,
            content_hash,
            marker_modified_at,
            indexed_at_epoch_millis: self.indexed_at,
        }))
    }
}

#[derive(Debug)]
struct RecordRow {
    skill_id: String,
    path: Vec<u8>,
    normalized_path: Vec<u8>,
    marker: String,
    marker_path: Vec<u8>,
    marker_modified_at: Option<Vec<u8>>,
    marker_file_size: i64,
    filesystem_fingerprint: Vec<u8>,
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    content_hash: Option<String>,
    state: String,
    error_kind: Option<String>,
    error_message: Option<String>,
    indexed_at: i64,
    parse_version: i64,
}

impl RecordRow {
    fn decode(self) -> Result<IndexRecord, IndexError> {
        let skill_id = SkillId::parse(&self.skill_id).map_err(|_| IndexError::InvalidData {
            entity: "skill_index_entries",
            field: "skill_id",
        })?;
        let marker_file_size =
            u64::try_from(self.marker_file_size).map_err(|_| IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "marker_file_size",
            })?;
        let fingerprint: [u8; 32] =
            self.filesystem_fingerprint
                .try_into()
                .map_err(|_| IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "filesystem_fingerprint",
                })?;
        let content_hash = self
            .content_hash
            .as_deref()
            .map(ContentHash::from_hex)
            .transpose()
            .map_err(|_| IndexError::InvalidData {
                entity: "skill_index_entries",
                field: "content_hash",
            })?;
        let record = IndexRecord {
            skill_id,
            stamp: SkillFilesystemStamp {
                path: decode_path(&self.path)?,
                normalized_path: decode_path(&self.normalized_path)?,
                marker: decode_marker(&self.marker)?,
                marker_path: decode_path(&self.marker_path)?,
                marker_modified_at: self
                    .marker_modified_at
                    .as_deref()
                    .map(decode_system_time)
                    .transpose()?,
                marker_file_size,
                fingerprint: FilesystemFingerprint::from_bytes(fingerprint),
            },
            name: self.name,
            description: self.description,
            version: self.version,
            content_hash,
            state: match self.state.as_str() {
                "valid" => "valid",
                "invalid" => "invalid",
                _ => {
                    return Err(IndexError::InvalidData {
                        entity: "skill_index_entries",
                        field: "state",
                    })
                }
            },
            error_kind: self.error_kind,
            error_message: self.error_message,
            indexed_at_epoch_millis: self.indexed_at,
            parse_version: self.parse_version,
        };
        match record.state {
            "valid" => {
                record.as_indexed_skill()?;
                if record.error_kind.is_some() || record.error_message.is_some() {
                    return Err(IndexError::InvalidData {
                        entity: "skill_index_entries",
                        field: "valid_error_fields",
                    });
                }
            }
            "invalid" => {
                if record.name.is_some()
                    || record.description.is_some()
                    || record.version.is_some()
                    || record.content_hash.is_some()
                    || record.error_kind.is_none()
                    || record.error_message.is_none()
                {
                    return Err(IndexError::InvalidData {
                        entity: "skill_index_entries",
                        field: "invalid_record_fields",
                    });
                }
            }
            _ => {
                return Err(IndexError::InvalidData {
                    entity: "skill_index_entries",
                    field: "state",
                })
            }
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_time_round_trips_on_both_sides_of_epoch() {
        for value in [
            UNIX_EPOCH + Duration::new(17, 123),
            UNIX_EPOCH - Duration::new(17, 123),
        ] {
            assert_eq!(
                decode_system_time(&encode_system_time(value)).unwrap(),
                value
            );
        }
    }

    #[test]
    fn application_id_constant_matches_schema_literal() {
        assert_eq!(APPLICATION_ID, 1_498_633_033);
    }
}
