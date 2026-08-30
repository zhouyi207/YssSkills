use std::path::Path;

use skill_core::SkillId;
use skill_local::{
    copy_skill, delete_skill, link_skill, read_skill, scan_directory, ExistingDestination,
    LinkKind, LocalError, OperationResult, ScanMode, ScanReport, ScannedSkill,
};

use crate::{
    error::CatalogFailure,
    model::{CentralSkillSnapshot, DeploymentBinding, DeploymentMode, WorkspaceId},
};

pub trait LocalSkillPort {
    fn scan(&self, root: &Path, mode: ScanMode) -> Result<ScanReport, LocalError>;
    fn read(&self, path: &Path) -> Result<ScannedSkill, LocalError>;
    fn deploy(
        &self,
        source: &Path,
        target: &Path,
        mode: DeploymentMode,
    ) -> Result<OperationResult, LocalError>;
    fn delete(&self, target: &Path) -> Result<OperationResult, LocalError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLocalSkillPort;

impl LocalSkillPort for SystemLocalSkillPort {
    fn scan(&self, root: &Path, mode: ScanMode) -> Result<ScanReport, LocalError> {
        scan_directory(root, mode)
    }

    fn read(&self, path: &Path) -> Result<ScannedSkill, LocalError> {
        read_skill(path)
    }

    fn deploy(
        &self,
        source: &Path,
        target: &Path,
        mode: DeploymentMode,
    ) -> Result<OperationResult, LocalError> {
        match mode {
            DeploymentMode::Copy => copy_skill(source, target, ExistingDestination::Replace),
            DeploymentMode::SymbolicLink => link_skill(
                source,
                target,
                LinkKind::Symbolic,
                ExistingDestination::Replace,
            ),
            DeploymentMode::Junction => link_skill(
                source,
                target,
                LinkKind::Junction,
                ExistingDestination::Replace,
            ),
        }
    }

    fn delete(&self, target: &Path) -> Result<OperationResult, LocalError> {
        delete_skill(target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CentralMatch {
    None,
    Unique(SkillId),
    Ambiguous(Vec<SkillId>),
}

pub trait CentralCatalogPort {
    fn list(&self) -> Result<Vec<CentralSkillSnapshot>, CatalogFailure>;
    fn bindings(&self, workspace_id: WorkspaceId)
        -> Result<Vec<DeploymentBinding>, CatalogFailure>;
    fn resolve_match(
        &self,
        scanned: &ScannedSkill,
        target_path: &Path,
    ) -> Result<CentralMatch, CatalogFailure>;
    fn import_local(
        &mut self,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure>;
    fn update_from_local(
        &mut self,
        skill_id: &SkillId,
        scanned: &ScannedSkill,
    ) -> Result<CentralSkillSnapshot, CatalogFailure>;
    fn associate(&mut self, binding: DeploymentBinding) -> Result<(), CatalogFailure>;
}
