import { useCallback, useRef, useState } from "react";

import { isIpcError } from "@/app/services/ipc-client";
import { skillsService } from "@/app/services/skills-service";
import { workspacesService } from "@/app/services/workspaces-service";
import type { IpcError } from "@/shared/types/ipc";
import type {
  CatalogSkillDetailDto,
  ExportCatalogSkillsRequestDto,
  ExportCatalogSkillsResponseDto,
  ImportLocalSkillsRequestDto,
  ImportLocalSkillsResponseDto,
  ScanImportFolderResponseDto,
} from "@/shared/types/skills";
import type { WorkspaceReconcileOutcomeDto } from "@/shared/types/workspaces";
import { unexpectedClientError, useServiceResource } from "./use-service-resource";

const loadCatalogSkills = () => skillsService.listCatalogSkills();

export function useCatalogSkills() {
  const resource = useServiceResource(loadCatalogSkills);
  const [detail, setDetail] = useState<CatalogSkillDetailDto | null>(null);
  const [detailError, setDetailError] = useState<IpcError | null>(null);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [refreshError, setRefreshError] = useState<IpcError | null>(null);
  const [isReconcilingAgents, setIsReconcilingAgents] = useState(false);
  const [deleteError, setDeleteError] = useState<IpcError | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [importError, setImportError] = useState<IpcError | null>(null);
  const [isScanningImport, setIsScanningImport] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const detailRequestId = useRef(0);

  const refresh = useCallback(async (): Promise<WorkspaceReconcileOutcomeDto | null> => {
    setRefreshError(null);
    setIsReconcilingAgents(true);

    try {
      const outcome = await workspacesService.reconcileAgentsWorkspace();
      const refreshedCatalog = await resource.refresh();
      return refreshedCatalog === null ? null : outcome;
    } catch (caught: unknown) {
      setRefreshError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      return null;
    } finally {
      setIsReconcilingAgents(false);
    }
  }, [resource.refresh]);

  const loadDetail = useCallback(async (skillId: string) => {
    const requestId = ++detailRequestId.current;
    setDetail(null);
    setDetailError(null);
    setIsDetailLoading(true);

    try {
      const next = await skillsService.getCatalogSkill({ skillId });
      if (detailRequestId.current === requestId) {
        setDetail(next);
      }
    } catch (caught: unknown) {
      if (detailRequestId.current === requestId) {
        setDetailError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      }
    } finally {
      if (detailRequestId.current === requestId) {
        setIsDetailLoading(false);
      }
    }
  }, []);

  const closeDetail = useCallback(() => {
    detailRequestId.current += 1;
    setDetail(null);
    setDetailError(null);
    setIsDetailLoading(false);
  }, []);

  const deleteSkills = useCallback(
    async (skillIds: string[]): Promise<string[] | null> => {
      setDeleteError(null);
      setIsDeleting(true);
      try {
        const response = await skillsService.deleteCatalogSkills({ skillIds });
        await resource.refresh();
        return response.deletedSkillIds;
      } catch (caught: unknown) {
        setDeleteError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsDeleting(false);
      }
    },
    [resource.refresh],
  );

  const scanImportFolder = useCallback(
    async (root: string): Promise<ScanImportFolderResponseDto> => {
      setImportError(null);
      setIsScanningImport(true);
      try {
        return await skillsService.scanImportFolder({ root });
      } catch (caught: unknown) {
        const error = isIpcError(caught) ? caught : unexpectedClientError(caught);
        setImportError(error);
        throw error;
      } finally {
        setIsScanningImport(false);
      }
    },
    [],
  );

  const importLocalSkills = useCallback(
    async (request: ImportLocalSkillsRequestDto): Promise<ImportLocalSkillsResponseDto> => {
      setImportError(null);
      setIsImporting(true);
      try {
        const response = await skillsService.importLocalSkills(request);
        await resource.refresh();
        return response;
      } catch (caught: unknown) {
        const error = isIpcError(caught) ? caught : unexpectedClientError(caught);
        setImportError(error);
        throw error;
      } finally {
        setIsImporting(false);
      }
    },
    [resource.refresh],
  );

  const clearImportError = useCallback(() => setImportError(null), []);

  const exportCatalogSkills = useCallback(
    async (request: ExportCatalogSkillsRequestDto): Promise<ExportCatalogSkillsResponseDto> => {
      setIsExporting(true);
      try {
        return await skillsService.exportCatalogSkills(request);
      } catch (caught: unknown) {
        throw isIpcError(caught) ? caught : unexpectedClientError(caught);
      } finally {
        setIsExporting(false);
      }
    },
    [],
  );

  return {
    ...resource,
    error: deleteError ?? refreshError ?? resource.error,
    isRefreshing: isReconcilingAgents || resource.isRefreshing,
    refresh,
    skills: resource.data?.skills ?? [],
    indexDiagnostics: resource.data?.diagnostics ?? [],
    indexStatus: resource.data?.index ?? null,
    detail,
    detailError,
    isDetailLoading,
    loadDetail,
    closeDetail,
    deleteError,
    isDeleting,
    deleteSkills,
    importError,
    isScanningImport,
    isImporting,
    scanImportFolder,
    importLocalSkills,
    clearImportError,
    isExporting,
    exportCatalogSkills,
  };
}
