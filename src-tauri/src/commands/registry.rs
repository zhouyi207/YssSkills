use tauri::State;

use crate::{
    commands::parse_request,
    ipc::{IpcError, RegistryLeaderboardRequestDto, RegistryResultDto, RegistrySearchRequestDto},
    state::AppState,
};

#[tauri::command]
pub async fn search_registry(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<RegistryResultDto, IpcError> {
    let request: RegistrySearchRequestDto = parse_request(request)?;
    let client = state.registry.clone();
    let query = request.query.trim().to_owned();
    let response_query = query.clone();
    let limit = request.limit;
    let result = tauri::async_runtime::spawn_blocking(move || client.search(&query, limit))
        .await
        .map_err(|_| IpcError::blocking_task_failed())?
        .map_err(IpcError::from)?;
    Ok(RegistryResultDto::from_search(response_query, result))
}

#[tauri::command]
pub async fn get_registry_leaderboard(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<RegistryResultDto, IpcError> {
    let request: RegistryLeaderboardRequestDto = parse_request(request)?;
    let client = state.registry.clone();
    let leaderboard = request.leaderboard.into();
    let result = tauri::async_runtime::spawn_blocking(move || client.leaderboard(leaderboard))
        .await
        .map_err(|_| IpcError::blocking_task_failed())?
        .map_err(IpcError::from)?;
    Ok(RegistryResultDto::from_leaderboard(result))
}
