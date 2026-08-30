use tauri::State;

use crate::{
    commands::run_application,
    ipc::{DashboardOverviewDto, IpcError},
    state::AppState,
};

#[tauri::command]
pub async fn get_dashboard_overview(
    state: State<'_, AppState>,
) -> Result<DashboardOverviewDto, IpcError> {
    let overview = run_application(state.application.clone(), |application| {
        application.dashboard_overview()
    })
    .await?;
    Ok(overview.into())
}
