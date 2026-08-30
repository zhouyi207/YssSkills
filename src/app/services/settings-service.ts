import { invokeCommand } from "./ipc-client";
import {
  appSettingsDtoSchema,
  type AppSettingsDto,
  type UpdateCatalogRootRequestDto,
} from "@/shared/types/settings";

export const settingsService = {
  getAppSettings(): Promise<AppSettingsDto> {
    return invokeCommand("get_app_settings", appSettingsDtoSchema);
  },

  updateCatalogRoot(request: UpdateCatalogRootRequestDto): Promise<AppSettingsDto> {
    return invokeCommand("update_catalog_root", appSettingsDtoSchema, { request });
  },
};
