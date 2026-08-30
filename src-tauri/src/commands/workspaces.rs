use tauri::State;

use crate::{
    commands::{parse_request, run_application},
    ipc::{
        CreateWorkspaceRequestDto, IpcError, WorkspaceIdRequestDto, WorkspaceObservationDto,
        WorkspaceReconcileOutcomeDto, WorkspaceSummaryDto, WorkspacesOverviewDto,
    },
    state::AppState,
};

#[tauri::command]
pub async fn get_workspaces_overview(
    state: State<'_, AppState>,
) -> Result<WorkspacesOverviewDto, IpcError> {
    let overview = run_application(state.application.clone(), |application| {
        application.workspaces_overview()
    })
    .await?;
    Ok(overview.into())
}

#[tauri::command]
pub async fn create_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<WorkspaceSummaryDto, IpcError> {
    let request: CreateWorkspaceRequestDto = parse_request(request)?;
    let input = request.into();
    let workspace = run_application(state.application.clone(), move |application| {
        application.create_workspace(input)
    })
    .await?;
    Ok(workspace.into())
}

#[tauri::command]
pub async fn observe_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<WorkspaceObservationDto, IpcError> {
    let request: WorkspaceIdRequestDto = parse_request(request)?;
    let workspace_id = request.workspace_id;
    let observation = run_application(state.application.clone(), move |application| {
        application.observe_workspace(&workspace_id)
    })
    .await?;
    Ok(observation.into())
}

#[tauri::command]
pub async fn reconcile_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<WorkspaceReconcileOutcomeDto, IpcError> {
    let request: WorkspaceIdRequestDto = parse_request(request)?;
    let workspace_id = request.workspace_id;
    let outcome = run_application(state.application.clone(), move |application| {
        application.reconcile_workspace(&workspace_id)
    })
    .await?;
    Ok(outcome.into())
}
