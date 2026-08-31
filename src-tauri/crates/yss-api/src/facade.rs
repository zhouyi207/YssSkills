use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::Value;
use skill_registry::{RegistryError, SkillsShClient};
use thiserror::Error;

use crate::*;

#[derive(Clone)]
pub struct YssApi {
    application: ApplicationHandle,
    registry: SkillsShClient,
}

#[derive(Debug, Error)]
pub enum YssApiStartError {
    #[error(transparent)]
    Application(#[from] ApplicationWorkerError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

impl YssApi {
    pub fn start(
        database_path: PathBuf,
        default_catalog_root: PathBuf,
    ) -> Result<Self, YssApiStartError> {
        Ok(Self {
            application: ApplicationHandle::start(database_path, default_catalog_root)?,
            registry: SkillsShClient::new()?,
        })
    }

    fn execute<T, F>(&self, operation: F) -> Result<T, IpcError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Application) -> Result<T, ApplicationError> + Send + 'static,
    {
        self.application.execute(operation).map_err(Into::into)
    }

    pub fn get_dashboard_overview(&self) -> Result<DashboardOverviewDto, IpcError> {
        self.execute(|application| application.dashboard_overview())
            .map(Into::into)
    }

    pub fn list_catalog_skills(&self) -> Result<CatalogSkillsResponseDto, IpcError> {
        self.execute(|application| application.list_catalog_skills_view())
            .map(Into::into)
    }

    pub fn create_skill_set(
        &self,
        request: CreateSkillSetRequestDto,
    ) -> Result<SkillSetDto, IpcError> {
        self.execute(move |application| {
            application.create_skill_set(request.name, request.skill_ids)
        })
        .map(Into::into)
    }

    pub fn update_skill_set(
        &self,
        request: UpdateSkillSetRequestDto,
    ) -> Result<SkillSetDto, IpcError> {
        self.execute(move |application| {
            application.update_skill_set(&request.set_id, request.name, request.skill_ids)
        })
        .map(Into::into)
    }

    pub fn delete_skill_sets(
        &self,
        request: DeleteSkillSetsRequestDto,
    ) -> Result<DeleteSkillSetsResponseDto, IpcError> {
        let deleted =
            self.execute(move |application| application.delete_skill_sets(request.set_ids))?;
        Ok(DeleteSkillSetsResponseDto {
            deleted_set_ids: deleted.into_iter().map(|id| id.to_string()).collect(),
        })
    }

    pub fn update_catalog_skills(
        &self,
        request: UpdateCatalogSkillsRequestDto,
    ) -> Result<UpdateCatalogSkillsResponseDto, IpcError> {
        let plan = self.execute(move |application| {
            application.plan_catalog_skill_updates(request.skill_ids, request.set_ids)
        })?;
        let fetched = fetch_catalog_skill_updates(plan);
        self.execute(move |application| application.apply_catalog_skill_updates(fetched))
            .map(Into::into)
    }

    pub fn rebuild_catalog_index(&self) -> Result<RebuildCatalogIndexResponseDto, IpcError> {
        self.execute(Application::rebuild_catalog_index)
            .map(Into::into)
    }

    pub fn scan_import_folder(
        &self,
        request: ScanImportFolderRequestDto,
    ) -> Result<ScanImportFolderResponseDto, IpcError> {
        self.execute(move |application| application.scan_import_folder(PathBuf::from(request.root)))
            .map(Into::into)
    }

    pub fn import_local_skills(
        &self,
        request: ImportLocalSkillsRequestDto,
    ) -> Result<ImportLocalSkillsResponseDto, IpcError> {
        let root = PathBuf::from(request.root);
        let paths = request.paths.into_iter().map(PathBuf::from).collect();
        self.execute(move |application| application.import_local_skills(root, paths))
            .map(Into::into)
    }

    pub fn export_catalog_skills(
        &self,
        request: ExportCatalogSkillsRequestDto,
    ) -> Result<ExportCatalogSkillsResponseDto, IpcError> {
        self.execute(move |application| {
            application
                .export_catalog_skills(PathBuf::from(request.destination_root), request.skill_ids)
        })
        .map(Into::into)
    }

    pub fn delete_catalog_skills(
        &self,
        request: DeleteCatalogSkillsRequestDto,
    ) -> Result<DeleteCatalogSkillsResponseDto, IpcError> {
        let deleted =
            self.execute(move |application| application.delete_catalog_skills(request.skill_ids))?;
        Ok(DeleteCatalogSkillsResponseDto {
            deleted_skill_ids: deleted.into_iter().map(|id| id.to_string()).collect(),
        })
    }

    pub fn get_catalog_skill(
        &self,
        request: SkillIdRequestDto,
    ) -> Result<CatalogSkillDetailDto, IpcError> {
        self.execute(move |application| application.catalog_skill_detail(&request.skill_id))
            .map(Into::into)
    }

    pub fn get_workspaces_overview(&self) -> Result<WorkspacesOverviewDto, IpcError> {
        self.execute(|application| application.workspaces_overview())
            .map(Into::into)
    }

    pub fn detect_agents(&self) -> Result<DetectAgentsResponseDto, IpcError> {
        self.execute(|application| application.detect_agents())
            .map(Into::into)
    }

    pub fn add_detected_agents(
        &self,
        request: AddDetectedAgentsRequestDto,
    ) -> Result<AddDetectedAgentsResponseDto, IpcError> {
        self.execute(move |application| application.add_detected_agents(request.detector_ids))
            .map(Into::into)
    }

    pub fn delete_agents(
        &self,
        request: DeleteAgentsRequestDto,
    ) -> Result<DeleteAgentsResponseDto, IpcError> {
        self.execute(move |application| application.delete_agents(request.agent_ids))
            .map(Into::into)
    }

    pub fn copy_project_agent_skills(
        &self,
        request: CopyProjectAgentSkillsRequestDto,
    ) -> Result<CopyProjectAgentSkillsResponseDto, IpcError> {
        let input = request.into();
        self.execute(move |application| application.copy_project_agent_skills(input))
            .map(Into::into)
    }

    pub fn delete_project_agents(
        &self,
        request: DeleteProjectAgentsRequestDto,
    ) -> Result<DeleteProjectAgentsResponseDto, IpcError> {
        self.execute(move |application| {
            application.delete_project_agents(&request.workspace_id, request.agent_ids)
        })
        .map(Into::into)
    }

    pub fn create_workspace(
        &self,
        request: CreateWorkspaceRequestDto,
    ) -> Result<WorkspaceSummaryDto, IpcError> {
        let input = request.into();
        self.execute(move |application| application.create_workspace(input))
            .map(Into::into)
    }

    pub fn save_agent(
        &self,
        request: SaveAgentRequestDto,
    ) -> Result<SaveAgentResponseDto, IpcError> {
        let input = request.into();
        self.execute(move |application| application.save_agent(input))
            .map(Into::into)
    }

    pub fn observe_workspace(
        &self,
        request: WorkspaceIdRequestDto,
    ) -> Result<WorkspaceObservationDto, IpcError> {
        self.execute(move |application| application.observe_workspace(&request.workspace_id))
            .map(Into::into)
    }

    pub fn reconcile_workspace(
        &self,
        request: WorkspaceIdRequestDto,
    ) -> Result<WorkspaceReconcileOutcomeDto, IpcError> {
        self.execute(move |application| application.reconcile_workspace(&request.workspace_id))
            .map(Into::into)
    }

    pub fn get_app_settings(&self) -> Result<AppSettingsDto, IpcError> {
        self.execute(|application| Ok(application.app_settings()))
            .map(Into::into)
    }

    pub fn update_catalog_root(
        &self,
        request: UpdateCatalogRootRequestDto,
    ) -> Result<AppSettingsDto, IpcError> {
        self.execute(move |application| {
            application.update_catalog_root(PathBuf::from(request.catalog_root))
        })
        .map(Into::into)
    }

    pub fn search_registry(
        &self,
        request: RegistrySearchRequestDto,
    ) -> Result<RegistryResultDto, IpcError> {
        let query = request.query.trim().to_owned();
        let result = self.registry.search(&query, request.limit)?;
        Ok(RegistryResultDto::from_search(query, result))
    }

    pub fn get_registry_leaderboard(
        &self,
        request: RegistryLeaderboardRequestDto,
    ) -> Result<RegistryResultDto, IpcError> {
        let result = self.registry.leaderboard(request.leaderboard.into())?;
        Ok(RegistryResultDto::from_leaderboard(result))
    }
}

pub fn parse_request<T>(request: Option<Value>) -> Result<T, IpcError>
where
    T: DeserializeOwned,
{
    let request =
        request.ok_or_else(|| IpcError::invalid_request_payload("request payload is required"))?;
    serde_json::from_value(request).map_err(IpcError::invalid_request_payload)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestRequest {
        value: String,
    }

    #[test]
    fn request_parser_preserves_the_frontend_envelope_contract() {
        let missing = parse_request::<TestRequest>(None).unwrap_err();
        assert_eq!(missing.code, "request.invalid");

        let malformed = parse_request::<TestRequest>(Some(serde_json::json!({
            "value": "ok",
            "unexpected": true
        })))
        .unwrap_err();
        assert!(malformed
            .context
            .get("reason")
            .is_some_and(|reason| reason.contains("unknown field")));

        let valid =
            parse_request::<TestRequest>(Some(serde_json::json!({ "value": "ok" }))).unwrap();
        assert_eq!(valid.value, "ok");
    }
}
