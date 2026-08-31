// @vitest-environment node

import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { dashboardService } from "./dashboard-service";
import { registryService } from "./registry-service";
import { settingsService } from "./settings-service";
import { skillsService } from "./skills-service";
import { workspacesService } from "./workspaces-service";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const openUrlMock = vi.mocked(openUrl);

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
  location: path,
  updatedAtEpochMillis: null,
  deploymentCount: 0,
};

const catalogIndex = {
  freshness: "fresh",
  revision: 1,
  lastReconciledAtEpochMillis: null,
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

const settings = {
  catalogRoot: path,
};

describe("IPC services", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("uses the backend command names and request envelope", async () => {
    invokeMock
      .mockResolvedValueOnce(dashboardResponse)
      .mockResolvedValueOnce({ skills: [catalogSkill], diagnostics: [], index: catalogIndex })
      .mockResolvedValueOnce(rebuildCatalogIndexOutcome)
      .mockResolvedValueOnce({ skill: catalogSkill, body: "# Example" })
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
    openUrlMock.mockResolvedValueOnce(undefined);

    const skillRequest = { skillId: catalogSkill.id };
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
      ["get_workspaces_overview"],
      ["create_workspace", { request: createRequest }],
      ["observe_workspace", { request: workspaceRequest }],
      ["reconcile_workspace", { request: workspaceRequest }],
      ["search_registry", { request: searchRequest }],
      ["get_registry_leaderboard", { request: leaderboardRequest }],
      ["get_app_settings"],
      ["update_catalog_root", { request: settingsRequest }],
    ]);
    expect(openUrlMock).toHaveBeenCalledWith("https://skills.sh/example");
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
