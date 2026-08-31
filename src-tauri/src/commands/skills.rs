use std::path::PathBuf;

use tauri::State;

use crate::{
    commands::{parse_request, run_application},
    ipc::{
        CatalogSkillDetailDto, CatalogSkillsResponseDto, DeleteCatalogSkillsRequestDto,
        DeleteCatalogSkillsResponseDto, ExportCatalogSkillsRequestDto,
        ExportCatalogSkillsResponseDto, ImportLocalSkillsRequestDto, ImportLocalSkillsResponseDto,
        IpcError, RebuildCatalogIndexResponseDto, ScanImportFolderRequestDto,
        ScanImportFolderResponseDto, SkillIdRequestDto,
    },
    state::AppState,
};

#[tauri::command]
pub async fn list_catalog_skills(
    state: State<'_, AppState>,
) -> Result<CatalogSkillsResponseDto, IpcError> {
    let skills = run_application(state.application.clone(), |application| {
        application.list_catalog_skills_view()
    })
    .await?;
    Ok(skills.into())
}

#[tauri::command]
pub async fn rebuild_catalog_index(
    state: State<'_, AppState>,
) -> Result<RebuildCatalogIndexResponseDto, IpcError> {
    let outcome = run_application(state.application.clone(), |application| {
        application.rebuild_catalog_index()
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn scan_import_folder(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<ScanImportFolderResponseDto, IpcError> {
    let request: ScanImportFolderRequestDto = parse_request(request)?;
    let root = PathBuf::from(request.root);
    let preview = run_application(state.application.clone(), move |application| {
        application.scan_import_folder(root)
    })
    .await?;
    Ok(preview.into())
}

#[tauri::command]
pub async fn import_local_skills(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<ImportLocalSkillsResponseDto, IpcError> {
    let request: ImportLocalSkillsRequestDto = parse_request(request)?;
    let root = PathBuf::from(request.root);
    let paths = request.paths.into_iter().map(PathBuf::from).collect();
    let outcome = run_application(state.application.clone(), move |application| {
        application.import_local_skills(root, paths)
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn export_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<ExportCatalogSkillsResponseDto, IpcError> {
    let request: ExportCatalogSkillsRequestDto = parse_request(request)?;
    let destination_root = PathBuf::from(request.destination_root);
    let outcome = run_application(state.application.clone(), move |application| {
        application.export_catalog_skills(destination_root, request.skill_ids)
    })
    .await?;
    Ok(outcome.into())
}

#[tauri::command]
pub async fn delete_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<DeleteCatalogSkillsResponseDto, IpcError> {
    let request: DeleteCatalogSkillsRequestDto = parse_request(request)?;
    let deleted = run_application(state.application.clone(), move |application| {
        application.delete_catalog_skills(request.skill_ids)
    })
    .await?;
    Ok(DeleteCatalogSkillsResponseDto {
        deleted_skill_ids: deleted
            .into_iter()
            .map(|skill_id| skill_id.to_string())
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
