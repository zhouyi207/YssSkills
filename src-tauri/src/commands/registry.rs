use tauri::State;
use yss_api::{
    parse_request, IpcError, RegistryLeaderboardRequestDto, RegistryResultDto,
    RegistrySearchRequestDto, YssApi,
};

use crate::commands::run_api;

#[tauri::command]
pub async fn search_registry(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<RegistryResultDto, IpcError> {
    let request: RegistrySearchRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.search_registry(request)
    })
    .await
}

#[tauri::command]
pub async fn get_registry_leaderboard(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<RegistryResultDto, IpcError> {
    let request: RegistryLeaderboardRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.get_registry_leaderboard(request)
    })
    .await
}
