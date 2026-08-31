use std::path::PathBuf;

use tauri::State;
use yss_api::fetch_catalog_skill_updates;

use crate::{
    commands::{parse_request, run_application},
    ipc::{
        CatalogSkillDetailDto, CatalogSkillsResponseDto, CreateSkillSetRequestDto,
        DeleteCatalogSkillsRequestDto, DeleteCatalogSkillsResponseDto, DeleteSkillSetsRequestDto,
        DeleteSkillSetsResponseDto, ExportCatalogSkillsRequestDto, ExportCatalogSkillsResponseDto,
        ImportLocalSkillsRequestDto, ImportLocalSkillsResponseDto, IpcError,
        RebuildCatalogIndexResponseDto, ScanImportFolderRequestDto, ScanImportFolderResponseDto,
        SkillIdRequestDto, SkillSetDto, UpdateCatalogSkillsRequestDto,
        UpdateCatalogSkillsResponseDto, UpdateSkillSetRequestDto,
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
pub async fn create_skill_set(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<SkillSetDto, IpcError> {
    let request: CreateSkillSetRequestDto = parse_request(request)?;
    let set = run_application(state.application.clone(), move |application| {
        application.create_skill_set(request.name, request.skill_ids)
    })
    .await?;
    Ok(set.into())
}

#[tauri::command]
pub async fn update_skill_set(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<SkillSetDto, IpcError> {
    let request: UpdateSkillSetRequestDto = parse_request(request)?;
    let set_id = request.set_id;
    let set = run_application(state.application.clone(), move |application| {
        application.update_skill_set(&set_id, request.name, request.skill_ids)
    })
    .await?;
    Ok(set.into())
}

#[tauri::command]
pub async fn delete_skill_sets(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<DeleteSkillSetsResponseDto, IpcError> {
    let request: DeleteSkillSetsRequestDto = parse_request(request)?;
    let deleted = run_application(state.application.clone(), move |application| {
        application.delete_skill_sets(request.set_ids)
    })
    .await?;
    Ok(DeleteSkillSetsResponseDto {
        deleted_set_ids: deleted
            .into_iter()
            .map(|set_id| set_id.to_string())
            .collect(),
    })
}

#[tauri::command]
pub async fn update_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<UpdateCatalogSkillsResponseDto, IpcError> {
    let request: UpdateCatalogSkillsRequestDto = parse_request(request)?;
    let application = state.application.clone();
    let plan = run_application(application.clone(), move |application| {
        application.plan_catalog_skill_updates(request.skill_ids, request.set_ids)
    })
    .await?;
    let fetched = tauri::async_runtime::spawn_blocking(move || fetch_catalog_skill_updates(plan))
        .await
        .map_err(IpcError::blocking_task_failed)?;
    let outcome = run_application(application, move |application| {
        application.apply_catalog_skill_updates(fetched)
    })
    .await?;
    Ok(outcome.into())
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
