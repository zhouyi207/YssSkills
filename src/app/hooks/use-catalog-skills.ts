import { useCallback, useRef, useState } from "react";

import { isIpcError } from "@/app/services/ipc-client";
import { skillsService } from "@/app/services/skills-service";
import type { IpcError } from "@/shared/types/ipc";
import type { CatalogSkillDetailDto } from "@/shared/types/skills";
import { unexpectedClientError, useServiceResource } from "./use-service-resource";

const loadCatalogSkills = () => skillsService.listCatalogSkills();

export function useCatalogSkills() {
  const resource = useServiceResource(loadCatalogSkills);
  const [detail, setDetail] = useState<CatalogSkillDetailDto | null>(null);
  const [detailError, setDetailError] = useState<IpcError | null>(null);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const detailRequestId = useRef(0);

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
        setDetailError(isIpcError(caught) ? caught : unexpectedClientError());
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

  return {
    ...resource,
    skills: resource.data?.skills ?? [],
    detail,
    detailError,
    isDetailLoading,
    loadDetail,
    closeDetail,
  };
}
