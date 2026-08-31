import { invokeCommand } from "./ipc-client";
import {
  addDetectedAgentsResponseDtoSchema,
  copyProjectAgentSkillsResponseDtoSchema,
  deleteAgentsResponseDtoSchema,
  deleteProjectAgentsResponseDtoSchema,
  detectAgentsResponseDtoSchema,
  saveAgentResponseDtoSchema,
  workspaceObservationDtoSchema,
  workspaceReconcileOutcomeDtoSchema,
  workspaceSummaryDtoSchema,
  workspacesOverviewDtoSchema,
  type AddDetectedAgentsRequestDto,
  type AddDetectedAgentsResponseDto,
  type CopyProjectAgentSkillsRequestDto,
  type CopyProjectAgentSkillsResponseDto,
  type CreateWorkspaceRequestDto,
  type DeleteAgentsRequestDto,
  type DeleteAgentsResponseDto,
  type DeleteProjectAgentsRequestDto,
  type DeleteProjectAgentsResponseDto,
  type DetectAgentsResponseDto,
  type SaveAgentRequestDto,
  type SaveAgentResponseDto,
  type WorkspaceIdRequestDto,
  type WorkspaceObservationDto,
  type WorkspaceReconcileOutcomeDto,
  type WorkspaceSummaryDto,
  type WorkspacesOverviewDto,
} from "@/shared/types/workspaces";

export function countWorkspaceReconcileIssues(outcome: WorkspaceReconcileOutcomeDto) {
  return (
    outcome.requested.finalReport.diagnostics.length +
    outcome.propagated.reduce(
      (count, propagation) =>
        count +
        (propagation.error ? 1 : 0) +
        (propagation.report?.finalReport.diagnostics.length ?? 0),
      0,
    )
  );
}

export const workspacesService = {
  getWorkspacesOverview(): Promise<WorkspacesOverviewDto> {
    return invokeCommand("get_workspaces_overview", workspacesOverviewDtoSchema);
  },

  detectAgents(): Promise<DetectAgentsResponseDto> {
    return invokeCommand("detect_agents", detectAgentsResponseDtoSchema);
  },

  addDetectedAgents(request: AddDetectedAgentsRequestDto): Promise<AddDetectedAgentsResponseDto> {
    return invokeCommand("add_detected_agents", addDetectedAgentsResponseDtoSchema, { request });
  },

  deleteAgents(request: DeleteAgentsRequestDto): Promise<DeleteAgentsResponseDto> {
    return invokeCommand("delete_agents", deleteAgentsResponseDtoSchema, { request });
  },

  copyProjectAgentSkills(
    request: CopyProjectAgentSkillsRequestDto,
  ): Promise<CopyProjectAgentSkillsResponseDto> {
    return invokeCommand("copy_project_agent_skills", copyProjectAgentSkillsResponseDtoSchema, {
      request,
    });
  },

  deleteProjectAgents(
    request: DeleteProjectAgentsRequestDto,
  ): Promise<DeleteProjectAgentsResponseDto> {
    return invokeCommand("delete_project_agents", deleteProjectAgentsResponseDtoSchema, {
      request,
    });
  },

  createWorkspace(request: CreateWorkspaceRequestDto): Promise<WorkspaceSummaryDto> {
    return invokeCommand("create_workspace", workspaceSummaryDtoSchema, { request });
  },

  saveAgent(request: SaveAgentRequestDto): Promise<SaveAgentResponseDto> {
    return invokeCommand("save_agent", saveAgentResponseDtoSchema, { request });
  },

  observeWorkspace(request: WorkspaceIdRequestDto): Promise<WorkspaceObservationDto> {
    return invokeCommand("observe_workspace", workspaceObservationDtoSchema, { request });
  },

  reconcileWorkspace(request: WorkspaceIdRequestDto): Promise<WorkspaceReconcileOutcomeDto> {
    return invokeCommand("reconcile_workspace", workspaceReconcileOutcomeDtoSchema, {
      request,
    });
  },

  async reconcileAgentsWorkspace(): Promise<WorkspaceReconcileOutcomeDto> {
    const overview = await invokeCommand("get_workspaces_overview", workspacesOverviewDtoSchema);
    return invokeCommand("reconcile_workspace", workspaceReconcileOutcomeDtoSchema, {
      request: { workspaceId: overview.agentsWorkspaceId },
    });
  },
};
