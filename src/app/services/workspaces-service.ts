import { invokeCommand } from "./ipc-client";
import {
  workspaceObservationDtoSchema,
  workspaceReconcileOutcomeDtoSchema,
  workspaceSummaryDtoSchema,
  workspacesOverviewDtoSchema,
  type CreateWorkspaceRequestDto,
  type WorkspaceIdRequestDto,
  type WorkspaceObservationDto,
  type WorkspaceReconcileOutcomeDto,
  type WorkspaceSummaryDto,
  type WorkspacesOverviewDto,
} from "@/shared/types/workspaces";

export const workspacesService = {
  getWorkspacesOverview(): Promise<WorkspacesOverviewDto> {
    return invokeCommand("get_workspaces_overview", workspacesOverviewDtoSchema);
  },

  createWorkspace(request: CreateWorkspaceRequestDto): Promise<WorkspaceSummaryDto> {
    return invokeCommand("create_workspace", workspaceSummaryDtoSchema, { request });
  },

  observeWorkspace(request: WorkspaceIdRequestDto): Promise<WorkspaceObservationDto> {
    return invokeCommand("observe_workspace", workspaceObservationDtoSchema, { request });
  },

  reconcileWorkspace(request: WorkspaceIdRequestDto): Promise<WorkspaceReconcileOutcomeDto> {
    return invokeCommand("reconcile_workspace", workspaceReconcileOutcomeDtoSchema, {
      request,
    });
  },
};
