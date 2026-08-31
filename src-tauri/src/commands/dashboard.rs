use tauri::State;
use yss_api::{DashboardOverviewDto, IpcError, YssApi};

use crate::commands::run_api;

#[tauri::command]
pub async fn get_dashboard_overview(
    state: State<'_, YssApi>,
) -> Result<DashboardOverviewDto, IpcError> {
    run_api(state.inner().clone(), YssApi::get_dashboard_overview).await
}
