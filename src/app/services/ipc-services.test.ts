// @vitest-environment node

import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { dashboardService } from "./dashboard-service";
import { registryService } from "./registry-service";
import { settingsService } from "./settings-service";
import { skillsService } from "./skills-service";
import { workspacesService } from "./workspaces-service";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const webviewWindowMocks = vi.hoisted(() => ({
  constructor: vi.fn(),
  getByLabel: vi.fn(),
  once: vi.fn(),
  setFocus: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => {
  class MockWebviewWindow {
    static getByLabel = webviewWindowMocks.getByLabel;

    constructor(...args: unknown[]) {
      webviewWindowMocks.constructor(...args);
    }

    once = webviewWindowMocks.once;
    setFocus = webviewWindowMocks.setFocus;
  }

  return { WebviewWindow: MockWebviewWindow };
});

const invokeMock = vi.mocked(invoke);

const path = {
  value: "/catalog",
  display: "/catalog",
};

const dashboardResponse = {
  counts: {
    skills: 1,
    deployments: 1,
    detectedHarnesses: 1,
    workspaces: 1,
  },
  activity: [],
  diagnostics: [],
};

const catalogSkill = {
  id: "skill-1",
  name: "Example",
  description: "Example skill",
  version: null,
  source: {
    kind: "local",
    path,
  },
  sourceMetadata: {
    source: "owner/repository",
    sourceType: "github",
    sourceUrl: "https://github.com/owner/repository.git",
    skillPath: "skills/example/SKILL.md",
    skillFolderHash: "hash",
    pluginName: null,
    ref: null,
    installedAt: "2026-08-01T00:00:00.000Z",
    updatedAt: "2026-08-31T00:00:00.000Z",
  },
  location: path,
  updatedAtEpochMillis: null,
  deploymentCount: 0,
};

const catalogIndex = {
  freshness: "fresh",
  revision: 1,
  lastReconciledAtEpochMillis: null,
};

const skillSet = {
  id: "set-1",
  name: "Example Set",
  skillIds: [catalogSkill.id],
};

const updateCatalogSkillsOutcome = {
  updatedSkillIds: [catalogSkill.id],
  unchangedSkillIds: [],
  unavailableSkillIds: [],
  failures: [],
};

const rebuildCatalogIndexOutcome = {
  inserted: 1,
  updated: 0,
  removed: 0,
  unchanged: 0,
  invalid: 0,
  revision: 1,
};

const workspace = {
  id: "workspace-1",
  name: "Agents",
  kind: { kind: "agents" },
  deploymentMode: "link",
  deploymentCount: 0,
};

const workspaceReport = {
  workspaceId: workspace.id,
  observations: [],
  unmatchedLocal: [],
  diagnostics: [],
};

const workspaceObservation = {
  workspace,
  resolution: {
    targets: [],
    discoveryRoots: [],
    unsupported: [],
  },
  report: workspaceReport,
  projectAgents: [],
};

const reconcileReport = {
  workspaceId: workspace.id,
  imported: [],
  centerUpdated: [],
  propagated: [],
  finalReport: workspaceReport,
};

const reconcileOutcome = {
  requestedWorkspace: workspace,
  requested: reconcileReport,
  propagated: [],
};

const searchResult = {
  mode: "search",
  leaderboard: null,
  query: "example",
  skills: [],
};

const leaderboardResult = {
  mode: "leaderboard",
  leaderboard: "allTime",
  query: null,
  skills: [],
};

const registrySkill = {
  id: { source: "owner/repository", skillId: "example" },
  name: "Example",
  installs: 42,
  sourceKind: "github",
  official: false,
  detailsUrl: "https://skills.sh/owner/repository/example",
  rank: 1,
};

const settings = {
  catalogRoot: path,
};

describe("IPC services", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    registryService.invalidateLeaderboardCache();
    webviewWindowMocks.getByLabel.mockResolvedValue(null);
    webviewWindowMocks.setFocus.mockResolvedValue(undefined);
    webviewWindowMocks.once.mockImplementation(
      (event: string, handler: (event: { payload?: unknown }) => void) => {
        if (event === "tauri://created") {
          handler({ payload: null });
        }
        return Promise.resolve(webviewWindowMocks.unlisten);
      },
    );
  });

  it("uses the backend command names and request envelope", async () => {
    invokeMock
      .mockResolvedValueOnce(dashboardResponse)
      .mockResolvedValueOnce({
        skills: [catalogSkill],
        sets: [skillSet],
        diagnostics: [],
        index: catalogIndex,
      })
      .mockResolvedValueOnce(rebuildCatalogIndexOutcome)
      .mockResolvedValueOnce({ skill: catalogSkill, body: "# Example" })
      .mockResolvedValueOnce(skillSet)
      .mockResolvedValueOnce(skillSet)
      .mockResolvedValueOnce({ deletedSetIds: [skillSet.id] })
      .mockResolvedValueOnce(updateCatalogSkillsOutcome)
      .mockResolvedValueOnce({
        agentsWorkspaceId: workspace.id,
        harnesses: [],
        workspaces: [workspace],
      })
      .mockResolvedValueOnce(workspace)
      .mockResolvedValueOnce(workspaceObservation)
      .mockResolvedValueOnce(reconcileOutcome)
      .mockResolvedValueOnce(searchResult)
      .mockResolvedValueOnce(leaderboardResult)
      .mockResolvedValueOnce(settings)
      .mockResolvedValueOnce(settings);

    const skillRequest = { skillId: catalogSkill.id };
    const createSetRequest = { name: skillSet.name, skillIds: skillSet.skillIds };
    const updateSetRequest = { setId: skillSet.id, ...createSetRequest };
    const deleteSetsRequest = { setIds: [skillSet.id] };
    const updateSkillsRequest = { skillIds: [catalogSkill.id], setIds: [] };
    const createRequest = {
      name: "Project",
      kind: { kind: "project" as const, root: "/project" },
      deploymentMode: "copy" as const,
    };
    const workspaceRequest = { workspaceId: workspace.id };
    const searchRequest = { query: "example", limit: 20 };
    const leaderboardRequest = { leaderboard: "allTime" as const };
    const settingsRequest = { catalogRoot: "/next-catalog" };

    await dashboardService.getDashboardOverview();
    await skillsService.listCatalogSkills();
    await skillsService.rebuildCatalogIndex();
    await skillsService.getCatalogSkill(skillRequest);
    await skillsService.createSkillSet(createSetRequest);
    await skillsService.updateSkillSet(updateSetRequest);
    await skillsService.deleteSkillSets(deleteSetsRequest);
    await skillsService.updateCatalogSkills(updateSkillsRequest);
    await workspacesService.getWorkspacesOverview();
    await workspacesService.createWorkspace(createRequest);
    await workspacesService.observeWorkspace(workspaceRequest);
    await workspacesService.reconcileWorkspace(workspaceRequest);
    await registryService.searchRegistry(searchRequest);
    await registryService.getRegistryLeaderboard(leaderboardRequest);
    await settingsService.getAppSettings();
    await settingsService.updateCatalogRoot(settingsRequest);
    await registryService.openDetails("https://skills.sh/example");

    expect(invokeMock.mock.calls).toEqual([
      ["get_dashboard_overview"],
      ["list_catalog_skills"],
      ["rebuild_catalog_index"],
      ["get_catalog_skill", { request: skillRequest }],
      ["create_skill_set", { request: createSetRequest }],
      ["update_skill_set", { request: updateSetRequest }],
      ["delete_skill_sets", { request: deleteSetsRequest }],
      ["update_catalog_skills", { request: updateSkillsRequest }],
      ["get_workspaces_overview"],
      ["create_workspace", { request: createRequest }],
      ["observe_workspace", { request: workspaceRequest }],
      ["reconcile_workspace", { request: workspaceRequest }],
      ["search_registry", { request: searchRequest }],
      ["get_registry_leaderboard", { request: leaderboardRequest }],
      ["get_app_settings"],
      ["update_catalog_root", { request: settingsRequest }],
    ]);
    expect(webviewWindowMocks.constructor).toHaveBeenCalledWith(
      expect.stringMatching(/^registry-details-/),
      expect.objectContaining({
        title: "Registry details",
        url: "https://skills.sh/example",
        width: 1_000,
        height: 700,
        resizable: true,
        visible: false,
        focus: false,
      }),
    );
  });

  it("reuses an existing registry webview window and focuses it", async () => {
    const existingWindow = {
      setFocus: vi.fn().mockResolvedValue(undefined),
    };
    webviewWindowMocks.getByLabel.mockResolvedValueOnce(existingWindow);

    await registryService.openDetails("https://skills.sh/example");

    expect(existingWindow.setFocus).toHaveBeenCalledOnce();
    expect(webviewWindowMocks.constructor).not.toHaveBeenCalled();
  });

  it("maps a strictly invalid response to ipc.invalid_response", async () => {
    invokeMock.mockResolvedValueOnce({
      ...dashboardResponse,
      counts: {
        ...dashboardResponse.counts,
        unexpected: true,
      },
    });

    await expect(dashboardService.getDashboardOverview()).rejects.toEqual(
      expect.objectContaining({
        code: "ipc.invalid_response",
        message: "The application returned an invalid response.",
        retryable: false,
        context: expect.objectContaining({
          command: "get_dashboard_overview",
          reason: expect.stringContaining("unexpected"),
        }),
      }),
    );
  });

  it("silently preloads and reuses the registry leaderboard request", async () => {
    invokeMock.mockResolvedValueOnce(leaderboardResult);

    registryService.preloadRegistry();

    await expect(
      registryService.getRegistryLeaderboard({ leaderboard: "allTime" }),
    ).resolves.toEqual(leaderboardResult);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_registry_leaderboard", {
      request: { leaderboard: "allTime" },
    });
  });

  it("bypasses the registry cache for an explicit refresh", async () => {
    const refreshed = { ...leaderboardResult, skills: [registrySkill] };
    invokeMock.mockResolvedValueOnce(leaderboardResult).mockResolvedValueOnce(refreshed);

    await registryService.getRegistryLeaderboard({ leaderboard: "allTime" });

    await expect(
      registryService.refreshRegistryLeaderboard({ leaderboard: "allTime" }),
    ).resolves.toEqual(refreshed);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("reconciles the agents workspace through its reported workspace id", async () => {
    invokeMock
      .mockResolvedValueOnce({
        agentsWorkspaceId: workspace.id,
        harnesses: [],
        workspaces: [],
      })
      .mockResolvedValueOnce(reconcileOutcome);

    await expect(workspacesService.reconcileAgentsWorkspace()).resolves.toEqual(reconcileOutcome);
    expect(invokeMock.mock.calls).toEqual([
      ["get_workspaces_overview"],
      ["reconcile_workspace", { request: { workspaceId: workspace.id } }],
    ]);
  });

  it("preserves a structured backend error and retry metadata", async () => {
    const backendError = {
      code: "registry.rate_limited",
      message: "The registry rate limit was exceeded.",
      retryable: true,
      context: { status: "429" },
      retryAfter: { kind: "delay", seconds: 15 },
    };
    invokeMock.mockRejectedValueOnce(backendError);

    await expect(registryService.searchRegistry({ query: "example", limit: 20 })).rejects.toEqual(
      backendError,
    );
  });

  it("maps an unknown rejection to a stable fallback while preserving its safe reason", async () => {
    invokeMock.mockRejectedValueOnce(
      new Error('{"code":"registry.rate_limited","message":"not a transport contract"}'),
    );

    await expect(settingsService.getAppSettings()).rejects.toEqual({
      code: "ipc.invoke_failed",
      message: "The application request failed.",
      retryable: false,
      context: {
        command: "get_app_settings",
        reason: '{"code":"registry.rate_limited","message":"not a transport contract"}',
      },
    });
  });
});
