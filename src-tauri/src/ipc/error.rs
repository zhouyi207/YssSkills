use std::{collections::BTreeMap, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use skill_harness::HarnessError;
use skill_index::{IndexError, SkillLockError};
use skill_local::LocalError;
use skill_registry::{RegistryError, RetryAfter};
use skill_workspace::{CatalogFailure, WorkspaceError};

use crate::{
    agent_config::AgentConfigError, application::ApplicationError, persistence::PersistenceError,
    state::ApplicationWorkerError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<RetryAfterDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RetryAfterDto {
    Delay { seconds: u64 },
    At { epoch_millis: i64 },
}

impl IpcError {
    fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            retryable: false,
            context: BTreeMap::new(),
            retry_after: None,
        }
    }

    fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    fn with_context(mut self, key: &str, value: impl ToString) -> Self {
        self.context.insert(key.to_owned(), value.to_string());
        self
    }

    fn with_retry_after(mut self, retry_after: Option<RetryAfter>) -> Self {
        self.retry_after = retry_after.and_then(retry_after_dto);
        self
    }

    pub(crate) fn blocking_task_failed(reason: impl ToString) -> Self {
        Self::new(
            "application.blocking_task_failed",
            "A background operation stopped unexpectedly.",
        )
        .with_context("reason", reason)
        .retryable()
    }

    pub(crate) fn invalid_request_payload(reason: impl ToString) -> Self {
        Self::new("request.invalid", "One or more request fields are invalid.")
            .with_context("reason", reason)
    }
}

impl From<ApplicationWorkerError> for IpcError {
    fn from(error: ApplicationWorkerError) -> Self {
        match error {
            ApplicationWorkerError::Initialization(error)
            | ApplicationWorkerError::Operation(error) => error.into(),
            ApplicationWorkerError::Start(source) => Self::new(
                "application.worker_start_failed",
                "Unable to start the application worker.",
            )
            .with_context("reason", source),
            ApplicationWorkerError::Unavailable | ApplicationWorkerError::ResponseDropped => {
                Self::new(
                    "application.worker_unavailable",
                    "The application worker is unavailable.",
                )
                .retryable()
            }
        }
    }
}

impl From<ApplicationError> for IpcError {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::Persistence(error) => error.into(),
            ApplicationError::Workspace(error) => error.into(),
            ApplicationError::Harness(error) => error.into(),
            ApplicationError::Local(error) => error.into(),
            ApplicationError::Catalog(error) => error.into(),
            ApplicationError::AgentConfig(error) => error.into(),
            ApplicationError::SkillLock(error) => error.into(),
            ApplicationError::InvalidRequest { field, reason } => {
                Self::new("request.invalid", "One or more request fields are invalid.")
                    .with_context("field", field)
                    .with_context("reason", reason)
            }
            ApplicationError::InvalidSkillId(reason) => {
                Self::new("skill.invalid_id", "The skill identifier is invalid.")
                    .with_context("reason", reason)
            }
            ApplicationError::InvalidWorkspaceId(reason) => Self::new(
                "workspace.invalid_id",
                "The workspace identifier is invalid.",
            )
            .with_context("reason", reason),
            ApplicationError::InvalidSkillSetId(reason) => Self::new(
                "skill_set.invalid_id",
                "The Skill Set identifier is invalid.",
            )
            .with_context("reason", reason),
            ApplicationError::WorkspaceChangedDuringReconcile => Self::new(
                "workspace.changed_during_reconcile",
                "Workspace content changed while synchronization was being prepared.",
            )
            .retryable(),
        }
    }
}

impl From<SkillLockError> for IpcError {
    fn from(error: SkillLockError) -> Self {
        match error {
            SkillLockError::Io { path, source } => Self::new(
                "skill_metadata.lock_read_failed",
                "The Skill source metadata could not be read.",
            )
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            SkillLockError::Decode { path, source } => Self::new(
                "skill_metadata.lock_invalid",
                "The Skill source metadata file is invalid.",
            )
            .with_context("path", path.display())
            .with_context("reason", source),
        }
    }
}

impl From<AgentConfigError> for IpcError {
    fn from(error: AgentConfigError) -> Self {
        match error {
            AgentConfigError::Io {
                operation,
                path,
                source,
            } => Self::new(
                "agent.configuration_filesystem_failed",
                "The Agent configuration could not be saved.",
            )
            .with_context("operation", operation)
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            AgentConfigError::Decode(source) => Self::new(
                "agent.configuration_invalid",
                "The Agent configuration file is invalid.",
            )
            .with_context("reason", source),
            AgentConfigError::InvalidData { field } => Self::new(
                "agent.configuration_invalid",
                "The Agent configuration file is invalid.",
            )
            .with_context("field", field),
            AgentConfigError::Encode(source) => Self::new(
                "agent.configuration_encode_failed",
                "The Agent configuration could not be encoded.",
            )
            .with_context("reason", source),
        }
    }
}

impl From<PersistenceError> for IpcError {
    fn from(error: PersistenceError) -> Self {
        match error {
            PersistenceError::Database { operation, .. } => Self::new(
                "persistence.database_failed",
                "The local database operation failed.",
            )
            .with_context("operation", operation)
            .retryable(),
            PersistenceError::Io {
                operation,
                path,
                source,
            } => Self::new(
                "persistence.filesystem_failed",
                "The persistent storage path could not be accessed.",
            )
            .with_context("operation", operation)
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            PersistenceError::Local { operation, source } => {
                IpcError::from(*source).with_context("operation", operation)
            }
            PersistenceError::Index { operation, source } => {
                IpcError::from(*source).with_context("catalogOperation", operation)
            }
            PersistenceError::IndexAfterFilesystemCommit {
                operation,
                path,
                source,
            } => Self::new(
                "skill_index.update_after_filesystem_commit_failed",
                "The Skill files were updated, but the derived index could not be synchronized.",
            )
            .with_context("operation", operation)
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            PersistenceError::InvalidData { entity, field } => Self::new(
                "persistence.invalid_data",
                "The local database contains invalid data.",
            )
            .with_context("entity", entity)
            .with_context("field", field),
            PersistenceError::NotFound { entity, id } => {
                Self::new("persistence.not_found", "The requested item was not found.")
                    .with_context("entity", entity)
                    .with_context("id", id)
            }
            PersistenceError::Conflict { entity, id } => Self::new(
                "persistence.conflict",
                "The requested change conflicts with existing data.",
            )
            .with_context("entity", entity)
            .with_context("id", id),
            PersistenceError::CatalogNotEmpty => Self::new(
                "settings.catalog_not_empty",
                "The central catalog path can only change before skills are imported.",
            ),
            PersistenceError::Clock => {
                Self::new("system.clock_invalid", "The system clock is invalid.")
            }
            PersistenceError::Cleanup {
                operation,
                source,
                cleanup,
            } => Self::new(
                "persistence.cleanup_failed",
                "A storage operation failed and could not be fully cleaned up.",
            )
            .with_context("operation", operation)
            .with_context("operationError", source)
            .with_context("cleanupError", cleanup),
        }
    }
}

impl From<IndexError> for IpcError {
    fn from(error: IndexError) -> Self {
        match error {
            IndexError::Database { operation, .. } => Self::new(
                "skill_index.database_failed",
                "The derived Skill index database operation failed.",
            )
            .with_context("operation", operation)
            .retryable(),
            IndexError::Io {
                operation,
                path,
                source,
            } => Self::new(
                "skill_index.filesystem_failed",
                "The filesystem could not be inspected for the derived Skill index.",
            )
            .with_context("operation", operation)
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            IndexError::Local { operation, source } => {
                IpcError::from(*source).with_context("indexOperation", operation)
            }
            IndexError::InvalidData { entity, field } => Self::new(
                "skill_index.invalid_data",
                "The derived Skill index contains invalid data and must be rebuilt.",
            )
            .with_context("entity", entity)
            .with_context("field", field),
            IndexError::IncompatibleSchema => Self::new(
                "skill_index.schema_incompatible",
                "The derived Skill index schema is incompatible and must be rebuilt.",
            ),
            IndexError::InvalidSchema => Self::new(
                "skill_index.schema_invalid",
                "The derived Skill index is damaged and must be rebuilt.",
            ),
            IndexError::ConcurrentModification => Self::new(
                "skill_index.filesystem_changed",
                "Skill files changed while the derived index was being updated.",
            )
            .retryable(),
            IndexError::Cancelled => Self::new(
                "skill_index.cancelled",
                "The derived Skill index operation was cancelled.",
            )
            .retryable(),
        }
    }
}

impl From<WorkspaceError> for IpcError {
    fn from(error: WorkspaceError) -> Self {
        match error {
            WorkspaceError::InvalidWorkspace { reason } => Self::new(
                "workspace.invalid",
                "The workspace configuration is invalid.",
            )
            .with_context("reason", reason),
            WorkspaceError::InvalidTarget { path, reason } => {
                Self::new("workspace.invalid_target", "A workspace target is invalid.")
                    .with_context("path", path.display())
                    .with_context("reason", reason)
            }
            WorkspaceError::Harness(error) => error.into(),
            WorkspaceError::Local(error) => error.into(),
            WorkspaceError::Catalog { operation, source } => {
                IpcError::from(source).with_context("operation", operation)
            }
            WorkspaceError::AmbiguousMatch { path, candidates } => Self::new(
                "catalog.ambiguous_match",
                "A local skill matches more than one catalog skill.",
            )
            .with_context("path", path.display())
            .with_context(
                "candidates",
                candidates
                    .into_iter()
                    .map(|candidate| candidate.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            WorkspaceError::Unsupported {
                workspace_id,
                harness_id,
                reason,
            } => Self::new(
                "workspace.unsupported",
                "The harness does not support this workspace.",
            )
            .with_context("workspaceId", workspace_id)
            .with_context("harnessId", harness_id)
            .with_context("reason", reason),
            WorkspaceError::Missing { path } => {
                Self::new("workspace.path_missing", "A workspace path is missing.")
                    .with_context("path", path.display())
            }
            WorkspaceError::ReconcileFailed { key, source } => {
                let cause = IpcError::from(*source);
                Self::new(
                    "workspace.reconcile_failed",
                    "A workspace deployment could not be reconciled.",
                )
                .with_context("deploymentKey", key)
                .with_context("cause", cause.code)
                .with_context("reason", cause.message)
            }
        }
    }
}

impl From<CatalogFailure> for IpcError {
    fn from(error: CatalogFailure) -> Self {
        match error {
            CatalogFailure::Storage { source } => Self::new(
                "catalog.storage_failed",
                "The central catalog storage operation failed.",
            )
            .with_context("reason", source)
            .retryable(),
            CatalogFailure::InvalidData { reason } => Self::new(
                "catalog.invalid_data",
                "The central catalog contains invalid data.",
            )
            .with_context("reason", reason),
            CatalogFailure::NotFound { item } => {
                Self::new("catalog.not_found", "The catalog item was not found.")
                    .with_context("item", item)
            }
            CatalogFailure::Conflict { reason } => Self::new(
                "catalog.conflict",
                "The catalog operation conflicts with existing data.",
            )
            .with_context("reason", reason),
            CatalogFailure::LocalOperation { source } => (*source).into(),
        }
    }
}

impl From<LocalError> for IpcError {
    fn from(error: LocalError) -> Self {
        match error {
            LocalError::PathNotFound { path } => {
                Self::new("local.path_not_found", "The requested path does not exist.")
                    .with_context("path", path.display())
            }
            LocalError::NotDirectory { path } => Self::new(
                "local.not_directory",
                "The requested path is not a directory.",
            )
            .with_context("path", path.display()),
            LocalError::Io { path, source } => Self::new(
                "local.filesystem_failed",
                "The local filesystem operation failed.",
            )
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            LocalError::Walk { path, source } => Self::new(
                "local.filesystem_failed",
                "The local filesystem operation failed.",
            )
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            LocalError::MarkerNotFound { path } => Self::new(
                "skill.marker_not_found",
                "The directory does not contain a SKILL.md file.",
            )
            .with_context("path", path.display()),
            LocalError::Parse { path, source } => Self::new(
                "skill.invalid_document",
                "The SKILL.md document is invalid.",
            )
            .with_context("path", path.display())
            .with_context("reason", source),
            LocalError::PathOutsideRoot { root, path } => Self::new(
                "local.path_outside_root",
                "A file was found outside the allowed root.",
            )
            .with_context("root", root.display())
            .with_context("path", path.display()),
            LocalError::InvalidPathEncoding { path } => Self::new(
                "local.invalid_path_encoding",
                "A path cannot be represented safely on this platform.",
            )
            .with_context("path", path.display()),
            LocalError::InvalidPath { path } => {
                Self::new("local.invalid_path", "The requested path is invalid.")
                    .with_context("path", path.display())
            }
            LocalError::DestinationExists { path } => Self::new(
                "local.destination_exists",
                "The destination already exists.",
            )
            .with_context("path", path.display()),
            LocalError::NotLink { path } => Self::new(
                "local.not_link",
                "The requested path is not a symbolic link or junction.",
            )
            .with_context("path", path.display()),
            LocalError::PathConflict { source, target } => Self::new(
                "local.path_conflict",
                "The source and destination paths conflict.",
            )
            .with_context("sourcePath", source.display())
            .with_context("targetPath", target.display()),
            LocalError::NestedLink { path } => Self::new(
                "local.nested_link",
                "The skill contains a nested link that cannot be copied safely.",
            )
            .with_context("path", path.display()),
            LocalError::UnsupportedOperation { operation } => Self::new(
                "local.unsupported_operation",
                "The operation is not supported on this platform.",
            )
            .with_context("operation", operation),
            LocalError::VerificationRead { path, source } => Self::new(
                "local.verification_failed",
                "The filesystem operation could not be verified.",
            )
            .with_context("path", path.display())
            .with_context("reason", source),
            LocalError::VerificationMismatch {
                path,
                expected,
                actual,
            } => Self::new(
                "local.verification_failed",
                "The filesystem operation could not be verified.",
            )
            .with_context("path", path.display())
            .with_context("expected", expected)
            .with_context("actual", actual),
            LocalError::VerificationCleanup {
                path,
                operation,
                cleanup,
            } => Self::new(
                "local.verification_failed",
                "The filesystem operation could not be verified.",
            )
            .with_context("path", path.display())
            .with_context("operationError", operation)
            .with_context("cleanupError", cleanup),
            LocalError::InvalidDebounce => Self::new(
                "watcher.invalid_debounce",
                "The watcher debounce duration is invalid.",
            ),
            LocalError::DuplicateWatchTarget { id } => Self::new(
                "watcher.duplicate_target",
                "The watcher target is already registered.",
            )
            .with_context("id", id),
            LocalError::InvalidWatchTarget { id, path, reason } => {
                Self::new("watcher.invalid_target", "The watcher target is invalid.")
                    .with_context("id", id)
                    .with_context("path", path.display())
                    .with_context("reason", reason)
            }
            LocalError::Watch {
                operation,
                path,
                source,
            } => Self::new(
                "watcher.operation_failed",
                "The filesystem watcher operation failed.",
            )
            .with_context("operation", operation)
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            LocalError::WatcherCallback { errors } => {
                let reason = errors
                    .into_iter()
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; ");
                let error = Self::new(
                    "watcher.callback_failed",
                    "The filesystem watcher reported an error.",
                )
                .retryable();
                if reason.is_empty() {
                    error
                } else {
                    error.with_context("reason", reason)
                }
            }
            LocalError::WatcherStatePoisoned | LocalError::WatcherClosed => Self::new(
                "watcher.unavailable",
                "The filesystem watcher is unavailable.",
            )
            .retryable(),
        }
    }
}

impl From<HarnessError> for IpcError {
    fn from(error: HarnessError) -> Self {
        match error {
            HarnessError::HomeDirectoryUnavailable => Self::new(
                "harness.home_unavailable",
                "The user home directory is unavailable.",
            ),
            HarnessError::InvalidId(reason) => {
                Self::new("harness.invalid_id", "The harness identifier is invalid.")
                    .with_context("reason", reason)
            }
            HarnessError::EmptyDisplayName => Self::new(
                "harness.empty_display_name",
                "The harness display name must not be empty.",
            ),
            HarnessError::GlobalSkillsPathMustBeAbsolute { path } => Self::new(
                "harness.invalid_path",
                "A configured harness path is invalid.",
            )
            .with_context("field", "globalSkillsPath")
            .with_context("path", path)
            .with_context("reason", "must be absolute or start with '~/'"),
            HarnessError::ConfigurationPathMustBeAbsolute { path } => Self::new(
                "harness.invalid_path",
                "A configured harness path is invalid.",
            )
            .with_context("field", "configurationPath")
            .with_context("path", path)
            .with_context("reason", "must be absolute or start with '~/'"),
            HarnessError::ProjectSkillsPathMustBeRelative { path } => Self::new(
                "harness.invalid_path",
                "A configured harness path is invalid.",
            )
            .with_context("field", "projectSkillsPath")
            .with_context("path", path)
            .with_context("reason", "must be relative to the project root"),
            HarnessError::ProjectSkillsPathContainsParent { path } => Self::new(
                "harness.invalid_path",
                "A configured harness path is invalid.",
            )
            .with_context("field", "projectSkillsPath")
            .with_context("path", path)
            .with_context("reason", "must not contain parent directory segments"),
            HarnessError::PathProbe { path, source } => Self::new(
                "harness.path_unavailable",
                "A harness path could not be inspected.",
            )
            .with_context("path", path.display())
            .with_context("reason", source)
            .retryable(),
            HarnessError::DuplicateId { id } => Self::new(
                "harness.duplicate_id",
                "The harness identifier is already registered.",
            )
            .with_context("id", id),
        }
    }
}

impl From<RegistryError> for IpcError {
    fn from(error: RegistryError) -> Self {
        match error {
            RegistryError::InvalidQuery { reason } => Self::new(
                "registry.invalid_query",
                "The registry search query is invalid.",
            )
            .with_context("reason", reason),
            RegistryError::InvalidLimit { limit, min, max } => Self::new(
                "registry.invalid_limit",
                "The registry result limit is invalid.",
            )
            .with_context("limit", limit)
            .with_context("minimum", min)
            .with_context("maximum", max),
            RegistryError::InvalidTimeout => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "timeout must be greater than zero"),
            RegistryError::InvalidResponseLimit => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "response body limit must be greater than zero"),
            RegistryError::ResponseLimitTooLarge { requested, maximum } => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("requested", requested)
            .with_context("maximum", maximum)
            .with_context("reason", "response body limit exceeds the global maximum"),
            RegistryError::InvalidBaseUrl => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "base URL is invalid"),
            RegistryError::UnsupportedBaseUrlScheme => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "base URL must use HTTP or HTTPS"),
            RegistryError::BaseUrlQueryOrFragment => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "base URL must not contain a query or fragment"),
            RegistryError::InvalidProxy => Self::new(
                "registry.invalid_configuration",
                "The registry client configuration is invalid.",
            )
            .with_context("reason", "proxy configuration is invalid"),
            RegistryError::Timeout { operation, kind } => {
                Self::new("registry.timeout", "The registry request timed out.")
                    .with_context("operation", operation)
                    .with_context("kind", kind)
                    .retryable()
            }
            RegistryError::Transport { operation, kind } => {
                Self::new("registry.transport", "The registry could not be reached.")
                    .with_context("operation", operation)
                    .with_context("kind", kind)
                    .retryable()
            }
            RegistryError::HttpStatus {
                status,
                retry_after,
            } => Self::new(
                "registry.http_status",
                "The registry returned an unsuccessful response.",
            )
            .with_context("status", status)
            .with_retry_after(retry_after),
            RegistryError::AuthenticationRequired {
                status,
                retry_after,
            } => Self::new(
                "registry.authentication_required",
                "Registry authentication is required.",
            )
            .with_context("status", status)
            .with_retry_after(retry_after),
            RegistryError::RateLimited {
                status,
                retry_after,
            } => Self::new(
                "registry.rate_limited",
                "The registry rate limit was exceeded.",
            )
            .with_context("status", status)
            .with_retry_after(retry_after)
            .retryable(),
            RegistryError::ResponseTooLarge { limit, observed } => {
                let mut error = Self::new(
                    "registry.response_too_large",
                    "The registry response exceeded the configured size limit.",
                )
                .with_context("limit", limit);
                if let Some(observed) = observed {
                    error = error.with_context("observed", observed);
                }
                error
            }
            RegistryError::InvalidResponse { kind, message } => Self::new(
                "registry.invalid_response",
                "The registry returned an invalid response.",
            )
            .with_context("kind", kind)
            .with_context("reason", message),
            RegistryError::MissingResponseField { kind, field } => Self::new(
                "registry.invalid_response",
                "The registry returned an invalid response.",
            )
            .with_context("kind", kind)
            .with_context("field", field)
            .with_context("reason", "required response field is missing"),
            RegistryError::InvalidRegistrySkillId(reason) => Self::new(
                "registry.invalid_skill_id",
                "The registry returned an invalid skill identifier.",
            )
            .with_context("reason", reason),
            RegistryError::Source(reason) => Self::new(
                "registry.invalid_source",
                "The registry source reference is invalid.",
            )
            .with_context("reason", reason),
        }
    }
}

fn retry_after_dto(value: RetryAfter) -> Option<RetryAfterDto> {
    match value {
        RetryAfter::Delay(delay) => Some(RetryAfterDto::Delay {
            seconds: delay.as_secs(),
        }),
        RetryAfter::At(time) => {
            let millis = time.duration_since(UNIX_EPOCH).ok()?.as_millis();
            let epoch_millis = i64::try_from(millis).ok()?;
            Some(RetryAfterDto::At { epoch_millis })
        }
    }
}

#[cfg(test)]
mod tests {
    use skill_index::IndexError;
    use skill_registry::{RegistryError, RetryAfter};

    use super::*;

    #[test]
    fn rate_limit_error_keeps_machine_readable_retry_after() {
        let error = IpcError::from(RegistryError::RateLimited {
            status: 429,
            retry_after: Some(RetryAfter::Delay(std::time::Duration::from_secs(15))),
        });

        assert_eq!(error.code, "registry.rate_limited");
        assert!(error.retryable);
        assert_eq!(
            error.retry_after,
            Some(RetryAfterDto::Delay { seconds: 15 })
        );
        assert_eq!(error.context.get("status").map(String::as_str), Some("429"));
    }

    #[test]
    fn database_errors_do_not_expose_underlying_details() {
        let error = IpcError::from(PersistenceError::Database {
            operation: "query",
            source: rusqlite::Error::InvalidQuery,
        });
        let json = serde_json::to_string(&error).unwrap();

        assert_eq!(error.code, "persistence.database_failed");
        assert!(!json.contains("InvalidQuery"));
    }

    #[test]
    fn validation_errors_keep_the_rejected_field_and_reason() {
        let error = IpcError::from(ApplicationError::InvalidRequest {
            field: "agentRoot",
            reason: "must not overlap the central catalog or a workspace root",
        });

        assert_eq!(
            error.context.get("field").map(String::as_str),
            Some("agentRoot")
        );
        assert_eq!(
            error.context.get("reason").map(String::as_str),
            Some("must not overlap the central catalog or a workspace root")
        );
    }

    #[test]
    fn filesystem_errors_keep_the_path_and_safe_system_reason() {
        let error = IpcError::from(LocalError::Io {
            path: "C:/Users/example/.agent/skills".into(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied"),
        });

        assert_eq!(
            error.context.get("path").map(String::as_str),
            Some("C:/Users/example/.agent/skills")
        );
        assert_eq!(
            error.context.get("reason").map(String::as_str),
            Some("access denied")
        );
    }

    #[test]
    fn post_commit_index_errors_explain_that_filesystem_truth_was_preserved() {
        let error = IpcError::from(PersistenceError::IndexAfterFilesystemCommit {
            operation: "index_imported_skill",
            path: "C:/Users/example/.yss-skills/skills/example".into(),
            source: Box::new(IndexError::ConcurrentModification),
        });

        assert_eq!(
            error.code,
            "skill_index.update_after_filesystem_commit_failed"
        );
        assert_eq!(
            error.context.get("operation").map(String::as_str),
            Some("index_imported_skill")
        );
        assert_eq!(
            error.context.get("path").map(String::as_str),
            Some("C:/Users/example/.yss-skills/skills/example")
        );
        assert!(error
            .context
            .get("reason")
            .is_some_and(|reason| reason.contains("changed")));
    }
}
