use tauri::State;

use crate::{
    commands::{parse_request, run_application},
    ipc::{
        CatalogSkillDetailDto, CatalogSkillSummaryDto, CatalogSkillsResponseDto, IpcError,
        SkillIdRequestDto,
    },
    state::AppState,
};

#[tauri::command]
pub async fn list_catalog_skills(
    state: State<'_, AppState>,
) -> Result<CatalogSkillsResponseDto, IpcError> {
    let skills = run_application(state.application.clone(), |application| {
        application.list_catalog_skills()
    })
    .await?;
    Ok(CatalogSkillsResponseDto {
        skills: skills
            .into_iter()
            .map(CatalogSkillSummaryDto::from)
            .collect(),
    })
}

#[tauri::command]
pub async fn get_catalog_skill(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<CatalogSkillDetailDto, IpcError> {
    let request: SkillIdRequestDto = parse_request(request)?;
    let skill_id = request.skill_id;
    let detail = run_application(state.application.clone(), move |application| {
        application.catalog_skill_detail(&skill_id)
    })
    .await?;
    Ok(detail.into())
}
