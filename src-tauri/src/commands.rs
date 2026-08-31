pub(crate) mod dashboard;
pub(crate) mod registry;
pub(crate) mod settings;
pub(crate) mod skills;
pub(crate) mod workspaces;

use yss_api::{IpcError, YssApi};

async fn run_api<T, F>(api: YssApi, operation: F) -> Result<T, IpcError>
where
    T: Send + 'static,
    F: FnOnce(&YssApi) -> Result<T, IpcError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&api))
        .await
        .map_err(IpcError::blocking_task_failed)?
}
