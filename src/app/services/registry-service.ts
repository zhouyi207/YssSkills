import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { getUnknownErrorReason, invokeCommand } from "./ipc-client";
import {
  registryResultDtoSchema,
  type LeaderboardDto,
  type RegistryLeaderboardRequestDto,
  type RegistryResultDto,
  type RegistrySearchRequestDto,
} from "@/shared/types/registry";

const LEADERBOARD_CACHE_TTL_MILLIS = 5 * 60 * 1_000;
const REGISTRY_WINDOW_LABEL_PREFIX = "registry-details";
const REGISTRY_WINDOW_WIDTH = 1_000;
const REGISTRY_WINDOW_HEIGHT = 700;
const DEFAULT_REGISTRY_WINDOW_TITLE = "Registry details";

function registryWindowLabel(url: string) {
  let hash = 2_166_136_261;
  for (const character of url) {
    hash = Math.imul(hash ^ character.charCodeAt(0), 16_777_619);
  }

  return `${REGISTRY_WINDOW_LABEL_PREFIX}-${(hash >>> 0).toString(36)}`;
}

function registryWindowError(reason: unknown) {
  return new Error(getUnknownErrorReason(reason) ?? "Unable to open registry details.");
}

function waitForRegistryWindow(registryWindow: WebviewWindow): Promise<void> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let unlistenCreated: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    const cleanup = () => {
      unlistenCreated?.();
      unlistenError?.();
    };
    const complete = () => {
      if (settled) {
        return;
      }

      settled = true;
      cleanup();
      resolve();
    };
    const fail = (reason: unknown) => {
      if (settled) {
        return;
      }

      settled = true;
      cleanup();
      reject(reason instanceof Error ? reason : registryWindowError(reason));
    };

    void registryWindow
      .once("tauri://created", complete)
      .then((unlisten) => {
        unlistenCreated = unlisten;
        if (settled) {
          unlisten();
        }
      })
      .catch(fail);
    void registryWindow
      .once("tauri://error", (event) => fail(event.payload))
      .then((unlisten) => {
        unlistenError = unlisten;
        if (settled) {
          unlisten();
        }
      })
      .catch(fail);
  });
}

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

  async openDetails(url: string, title = DEFAULT_REGISTRY_WINDOW_TITLE): Promise<void> {
    const normalizedUrl = url.trim();
    if (!normalizedUrl) {
      throw new Error("Registry details URL is unavailable.");
    }

    const label = registryWindowLabel(normalizedUrl);
    const existingWindow = await WebviewWindow.getByLabel(label);
    if (existingWindow) {
      await existingWindow.setFocus();
      return;
    }

    const registryWindow = new WebviewWindow(label, {
      title: title.trim() || DEFAULT_REGISTRY_WINDOW_TITLE,
      url: normalizedUrl,
      width: REGISTRY_WINDOW_WIDTH,
      height: REGISTRY_WINDOW_HEIGHT,
      resizable: true,
      visible: false,
      focus: false,
    });

    await waitForRegistryWindow(registryWindow);
  },
};
