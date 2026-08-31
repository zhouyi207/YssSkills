use tauri::State;
use yss_api::{parse_request, AppSettingsDto, IpcError, UpdateCatalogRootRequestDto, YssApi};

use crate::commands::run_api;

#[tauri::command]
pub async fn get_app_settings(state: State<'_, YssApi>) -> Result<AppSettingsDto, IpcError> {
    run_api(state.inner().clone(), YssApi::get_app_settings).await
}

#[tauri::command]
pub async fn update_catalog_root(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<AppSettingsDto, IpcError> {
    let request: UpdateCatalogRootRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.update_catalog_root(request)
    })
    .await
}
