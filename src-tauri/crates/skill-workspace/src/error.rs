use std::{error::Error as StdError, path::PathBuf};

use skill_core::SkillId;
use skill_harness::{HarnessError, HarnessId};
use skill_local::LocalError;
use thiserror::Error;

use crate::model::{DeploymentKey, WorkspaceId};

#[derive(Debug, Error)]
pub enum CatalogFailure {
    #[error("central catalog storage operation failed: {source}")]
    Storage {
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
    #[error("central catalog data is invalid: {reason}")]
    InvalidData { reason: String },
    #[error("central catalog item was not found: {item}")]
    NotFound { item: String },
    #[error("central catalog conflict: {reason}")]
    Conflict { reason: String },
    #[error("central catalog local operation failed: {source}")]
    LocalOperation {
        #[source]
        source: Box<LocalError>,
    },
}

impl CatalogFailure {
    pub fn storage(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Storage {
            source: Box::new(source),
        }
    }

    pub fn invalid_data(reason: impl Into<String>) -> Self {
        Self::InvalidData {
            reason: reason.into(),
        }
    }

    pub fn not_found(item: impl Into<String>) -> Self {
        Self::NotFound { item: item.into() }
    }

    pub fn conflict(reason: impl Into<String>) -> Self {
        Self::Conflict {
            reason: reason.into(),
        }
    }

    pub fn local_operation(source: LocalError) -> Self {
        Self::LocalOperation {
            source: Box::new(source),
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("invalid workspace: {reason}")]
    InvalidWorkspace { reason: &'static str },
    #[error("invalid workspace target {path:?}: {reason}")]
    InvalidTarget { path: PathBuf, reason: &'static str },
    #[error(transparent)]
    Harness(#[from] HarnessError),
    #[error(transparent)]
    Local(#[from] LocalError),
    #[error("central catalog operation {operation} failed: {source}")]
    Catalog {
        operation: &'static str,
        #[source]
        source: CatalogFailure,
    },
    #[error("central catalog match is ambiguous for {path:?}")]
    AmbiguousMatch {
        path: PathBuf,
        candidates: Vec<SkillId>,
    },
    #[error("workspace {workspace_id} does not support harness {harness_id}: {reason}")]
    Unsupported {
        workspace_id: WorkspaceId,
        harness_id: HarnessId,
        reason: &'static str,
    },
    #[error("path is missing: {path:?}")]
    Missing { path: PathBuf },
    #[error("reconcile failed for {key}: {source}")]
    ReconcileFailed {
        key: DeploymentKey,
        #[source]
        source: Box<WorkspaceError>,
    },
}
