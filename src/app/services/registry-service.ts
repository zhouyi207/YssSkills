import { openUrl } from "@tauri-apps/plugin-opener";

import { invokeCommand } from "./ipc-client";
import {
  registryResultDtoSchema,
  type LeaderboardDto,
  type RegistryLeaderboardRequestDto,
  type RegistryResultDto,
  type RegistrySearchRequestDto,
} from "@/shared/types/registry";

const LEADERBOARD_CACHE_TTL_MILLIS = 5 * 60 * 1_000;

type LeaderboardCacheEntry =
  | {
      kind: "loading";
      request: Promise<RegistryResultDto>;
    }
  | {
      kind: "ready";
      value: RegistryResultDto;
      expiresAt: number;
    };

const leaderboardCache = new Map<LeaderboardDto, LeaderboardCacheEntry>();

function loadLeaderboard(
  request: RegistryLeaderboardRequestDto,
  force: boolean,
): Promise<RegistryResultDto> {
  const cached = leaderboardCache.get(request.leaderboard);
  if (!force && cached) {
    if (cached.kind === "loading") {
      return cached.request;
    }
    if (cached.expiresAt > Date.now()) {
      return Promise.resolve(cached.value);
    }
  }

  const pending = invokeCommand("get_registry_leaderboard", registryResultDtoSchema, { request });
  const entry: LeaderboardCacheEntry = { kind: "loading", request: pending };
  leaderboardCache.set(request.leaderboard, entry);
  void pending.then(
    (value) => {
      if (leaderboardCache.get(request.leaderboard) === entry) {
        leaderboardCache.set(request.leaderboard, {
          kind: "ready",
          value,
          expiresAt: Date.now() + LEADERBOARD_CACHE_TTL_MILLIS,
        });
      }
    },
    () => {
      if (leaderboardCache.get(request.leaderboard) === entry) {
        leaderboardCache.delete(request.leaderboard);
      }
    },
  );
  return pending;
}

export const registryService = {
  searchRegistry(request: RegistrySearchRequestDto): Promise<RegistryResultDto> {
    return invokeCommand("search_registry", registryResultDtoSchema, { request });
  },

  getRegistryLeaderboard(request: RegistryLeaderboardRequestDto): Promise<RegistryResultDto> {
    return loadLeaderboard(request, false);
  },

  refreshRegistryLeaderboard(request: RegistryLeaderboardRequestDto): Promise<RegistryResultDto> {
    return loadLeaderboard(request, true);
  },

  preloadRegistry(): void {
    void loadLeaderboard({ leaderboard: "allTime" }, false).catch(() => undefined);
  },

  invalidateLeaderboardCache(): void {
    leaderboardCache.clear();
  },

  openDetails(url: string): Promise<void> {
    return openUrl(url);
  },
};
