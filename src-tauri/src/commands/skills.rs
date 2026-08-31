use tauri::State;
use yss_api::{
    parse_request, CatalogSkillDetailDto, CatalogSkillsResponseDto, CreateSkillSetRequestDto,
    DeleteCatalogSkillsRequestDto, DeleteCatalogSkillsResponseDto, DeleteSkillSetsRequestDto,
    DeleteSkillSetsResponseDto, ExportCatalogSkillsRequestDto, ExportCatalogSkillsResponseDto,
    ImportLocalSkillsRequestDto, ImportLocalSkillsResponseDto, IpcError,
    RebuildCatalogIndexResponseDto, ScanImportFolderRequestDto, ScanImportFolderResponseDto,
    SkillIdRequestDto, SkillSetDto, UpdateCatalogSkillsRequestDto, UpdateCatalogSkillsResponseDto,
    UpdateSkillSetRequestDto, YssApi,
};

use crate::commands::run_api;

#[tauri::command]
pub async fn list_catalog_skills(
    state: State<'_, YssApi>,
) -> Result<CatalogSkillsResponseDto, IpcError> {
    run_api(state.inner().clone(), YssApi::list_catalog_skills).await
}

#[tauri::command]
pub async fn create_skill_set(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<SkillSetDto, IpcError> {
    let request: CreateSkillSetRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.create_skill_set(request)
    })
    .await
}

#[tauri::command]
pub async fn update_skill_set(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<SkillSetDto, IpcError> {
    let request: UpdateSkillSetRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.update_skill_set(request)
    })
    .await
}

#[tauri::command]
pub async fn delete_skill_sets(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<DeleteSkillSetsResponseDto, IpcError> {
    let request: DeleteSkillSetsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.delete_skill_sets(request)
    })
    .await
}

#[tauri::command]
pub async fn update_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<UpdateCatalogSkillsResponseDto, IpcError> {
    let request: UpdateCatalogSkillsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.update_catalog_skills(request)
    })
    .await
}

#[tauri::command]
pub async fn rebuild_catalog_index(
    state: State<'_, YssApi>,
) -> Result<RebuildCatalogIndexResponseDto, IpcError> {
    run_api(state.inner().clone(), YssApi::rebuild_catalog_index).await
}

#[tauri::command]
pub async fn scan_import_folder(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<ScanImportFolderResponseDto, IpcError> {
    let request: ScanImportFolderRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.scan_import_folder(request)
    })
    .await
}

#[tauri::command]
pub async fn import_local_skills(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<ImportLocalSkillsResponseDto, IpcError> {
    let request: ImportLocalSkillsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.import_local_skills(request)
    })
    .await
}

#[tauri::command]
pub async fn export_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<ExportCatalogSkillsResponseDto, IpcError> {
    let request: ExportCatalogSkillsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.export_catalog_skills(request)
    })
    .await
}

#[tauri::command]
pub async fn delete_catalog_skills(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<DeleteCatalogSkillsResponseDto, IpcError> {
    let request: DeleteCatalogSkillsRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.delete_catalog_skills(request)
    })
    .await
}

#[tauri::command]
pub async fn get_catalog_skill(
    request: Option<serde_json::Value>,
    state: State<'_, YssApi>,
) -> Result<CatalogSkillDetailDto, IpcError> {
    let request: SkillIdRequestDto = parse_request(request)?;
    run_api(state.inner().clone(), move |api| {
        api.get_catalog_skill(request)
    })
    .await
}
