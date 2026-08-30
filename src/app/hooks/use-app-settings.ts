import { useCallback, useState } from "react";

import { isIpcError } from "@/app/services/ipc-client";
import { settingsService } from "@/app/services/settings-service";
import type { IpcError } from "@/shared/types/ipc";
import { unexpectedClientError, useServiceResource } from "./use-service-resource";

const loadAppSettings = () => settingsService.getAppSettings();

export function useAppSettings() {
  const resource = useServiceResource(loadAppSettings);
  const [updateError, setUpdateError] = useState<IpcError | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);

  const updateCatalogRoot = useCallback(
    async (catalogRoot: string) => {
      setUpdateError(null);
      setIsUpdating(true);
      try {
        const settings = await settingsService.updateCatalogRoot({ catalogRoot });
        await resource.refresh();
        return settings;
      } catch (caught: unknown) {
        setUpdateError(isIpcError(caught) ? caught : unexpectedClientError());
        return null;
      } finally {
        setIsUpdating(false);
      }
    },
    [resource.refresh],
  );

  return { ...resource, settings: resource.data, updateError, isUpdating, updateCatalogRoot };
}
