use tauri::State;
use yss_api::{
    parse_request, AddDetectedAgentsRequestDto, AddDetectedAgentsResponseDto,
    CopyProjectAgentSkillsRequestDto, CopyProjectAgentSkillsResponseDto, CreateWorkspaceRequestDto,
    DeleteAgentsRequestDto, DeleteAgentsResponseDto, DeleteProjectAgentsRequestDto,
    DeleteProjectAgentsResponseDto, DetectAgentsResponseDto, IpcError, SaveAgentRequestDto,
    SaveAgentResponseDto, WorkspaceIdRequestDto, WorkspaceObservationDto,
    WorkspaceReconcileOutcomeDto, WorkspaceSummaryDto, WorkspacesOverviewDto, YssApi,
};

use crate::commands::run_api;

#[tauri::command]
pub async fn get_workspaces_overview(
    state: State<'_, YssApi>,
) -> Result<WorkspacesOverviewDto, IpcError> {
    run_api(state.inner().clone(), YssApi::get_workspaces_overview).await
}

#[tauri::command]
pub async fn detect_agents(state: State<'_, YssApi>) -> Result<DetectAgentsResponseDto, IpcError> {
    run_api(state.inner().clone(), YssApi::detect_agents).await
}

#[tauri::command]
pub async fn add_detected_agents(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<AddDetectedAgentsResponseDto, IpcError> {
    let request: AddDetectedAgentsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.add_detected_agents(request)
    })
    .await
}

#[tauri::command]
pub async fn delete_agents(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<DeleteAgentsResponseDto, IpcError> {
    let request: DeleteAgentsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| api.delete_agents(request)).await
}

#[tauri::command]
pub async fn copy_project_agent_skills(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<CopyProjectAgentSkillsResponseDto, IpcError> {
    let request: CopyProjectAgentSkillsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.copy_project_agent_skills(request)
    })
    .await
}

#[tauri::command]
pub async fn delete_project_agents(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<DeleteProjectAgentsResponseDto, IpcError> {
    let request: DeleteProjectAgentsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.delete_project_agents(request)
    })
    .await
}

#[tauri::command]
pub async fn create_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<WorkspaceSummaryDto, IpcError> {
    let request: CreateWorkspaceRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.create_workspace(request)
    })
    .await
}

#[tauri::command]
pub async fn save_agent(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<SaveAgentResponseDto, IpcError> {
    let request: SaveAgentRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| api.save_agent(request)).await
}

#[tauri::command]
pub async fn observe_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<WorkspaceObservationDto, IpcError> {
    let request: WorkspaceIdRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.observe_workspace(request)
    })
    .await
}

#[tauri::command]
pub async fn reconcile_workspace(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<WorkspaceReconcileOutcomeDto, IpcError> {
    let request: WorkspaceIdRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.reconcile_workspace(request)
    })
    .await
}
