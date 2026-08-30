import { openUrl } from "@tauri-apps/plugin-opener";

import { invokeCommand } from "./ipc-client";
import {
  registryResultDtoSchema,
  type RegistryLeaderboardRequestDto,
  type RegistryResultDto,
  type RegistrySearchRequestDto,
} from "@/shared/types/registry";

export const registryService = {
  searchRegistry(request: RegistrySearchRequestDto): Promise<RegistryResultDto> {
    return invokeCommand("search_registry", registryResultDtoSchema, { request });
  },

  getRegistryLeaderboard(request: RegistryLeaderboardRequestDto): Promise<RegistryResultDto> {
    return invokeCommand("get_registry_leaderboard", registryResultDtoSchema, { request });
  },

  openDetails(url: string): Promise<void> {
    return openUrl(url);
  },
};
