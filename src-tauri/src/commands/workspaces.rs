use tauri::State;

use crate::{
    commands::{parse_request, run_application},
    ipc::{
        AddDetectedAgentsRequestDto, AddDetectedAgentsResponseDto,
        CopyProjectAgentSkillsRequestDto, CopyProjectAgentSkillsResponseDto,
        CreateWorkspaceRequestDto, DeleteAgentsRequestDto, DeleteAgentsResponseDto,
        DeleteProjectAgentsRequestDto, DeleteProjectAgentsResponseDto, DetectAgentsResponseDto,
        IpcError, SaveAgentRequestDto, SaveAgentResponseDto, WorkspaceIdRequestDto,
        WorkspaceObservationDto, WorkspaceReconcileOutcomeDto, WorkspaceSummaryDto,
        WorkspacesOverviewDto,
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
pub async fn detect_agents(
    state: State<'_, AppState>,
) -> Result<DetectAgentsResponseDto, IpcError> {
    let outcome = run_application(state.application.clone(), |application| {
        application.detect_agents()
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn add_detected_agents(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<AddDetectedAgentsResponseDto, IpcError> {
    let request: AddDetectedAgentsRequestDto = parse_request(request)?;
    let outcome = run_application(state.application.clone(), move |application| {
        application.add_detected_agents(request.detector_ids)
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn delete_agents(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<DeleteAgentsResponseDto, IpcError> {
    let request: DeleteAgentsRequestDto = parse_request(request)?;
    let outcome = run_application(state.application.clone(), move |application| {
        application.delete_agents(request.agent_ids)
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn copy_project_agent_skills(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<CopyProjectAgentSkillsResponseDto, IpcError> {
    let request: CopyProjectAgentSkillsRequestDto = parse_request(request)?;
    let input = request.into();
    let outcome = run_application(state.application.clone(), move |application| {
        application.copy_project_agent_skills(input)
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn delete_project_agents(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<DeleteProjectAgentsResponseDto, IpcError> {
    let request: DeleteProjectAgentsRequestDto = parse_request(request)?;
    let outcome = run_application(state.application.clone(), move |application| {
        application.delete_project_agents(&request.workspace_id, request.agent_ids)
    })
    .await?;
    Ok(outcome.into())
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
pub async fn save_agent(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<SaveAgentResponseDto, IpcError> {
    let request: SaveAgentRequestDto = parse_request(request)?;
    let input = request.into();
    let outcome = run_application(state.application.clone(), move |application| {
        application.save_agent(input)
    })
    .await?;
    Ok(outcome.into())
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
