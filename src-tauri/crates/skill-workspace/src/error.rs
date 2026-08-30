use std::path::PathBuf;

use skill_core::SkillId;
use skill_harness::{HarnessError, HarnessId};
use skill_local::LocalError;
use thiserror::Error;

use crate::model::{DeploymentKey, WorkspaceId};

pub type CatalogFailure = Box<dyn std::error::Error + Send + Sync + 'static>;

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
