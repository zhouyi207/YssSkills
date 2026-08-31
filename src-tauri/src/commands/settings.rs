use std::path::PathBuf;

use tauri::State;
use yss_api::AppState;

use crate::{
    commands::{parse_request, run_application},
    ipc::{AppSettingsDto, IpcError, UpdateCatalogRootRequestDto},
};

#[tauri::command]
pub async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettingsDto, IpcError> {
    let settings = run_application(state.application.clone(), |application| {
        Ok(application.app_settings())
    })
    .await?;
    Ok(settings.into())
}

#[tauri::command]
pub async fn update_catalog_root(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<AppSettingsDto, IpcError> {
    let request: UpdateCatalogRootRequestDto = parse_request(request)?;
    let catalog_root = PathBuf::from(request.catalog_root);
    let settings = run_application(state.application.clone(), move |application| {
        application.update_catalog_root(catalog_root)
    })
    .await?;
    Ok(settings.into())
}
