import { z } from "zod";

import { ipcErrorSchema, pathDtoSchema } from "./ipc";

const nonNegativeIntegerSchema = z.number().int().nonnegative();

export const harnessCategoryDtoSchema = z.enum(["coding", "lobster"]);

export type HarnessCategoryDto = z.infer<typeof harnessCategoryDtoSchema>;

export const detectionStatusDtoSchema = z.enum([
  "installed",
  "notInstalled",
  "explicitlyConfigured",
]);

export type DetectionStatusDto = z.infer<typeof detectionStatusDtoSchema>;

export const harnessCapabilitiesDtoSchema = z
  .object({
    globalScope: z.boolean(),
    projectScope: z.boolean(),
    recursiveGlobalDiscovery: z.boolean(),
    configurationPath: z.boolean(),
  })
  .strict();

export type HarnessCapabilitiesDto = z.infer<typeof harnessCapabilitiesDtoSchema>;

export const harnessProbeDtoSchema = z
  .object({
    detectionStatus: detectionStatusDtoSchema,
    checkedPaths: z.array(pathDtoSchema),
    agentPath: pathDtoSchema,
    globalSkillsPath: pathDtoSchema,
  })
  .strict();

export type HarnessProbeDto = z.infer<typeof harnessProbeDtoSchema>;

export const harnessSummaryDtoSchema = z
  .object({
    id: z.string(),
    displayName: z.string(),
    category: harnessCategoryDtoSchema,
    custom: z.boolean(),
    capabilities: harnessCapabilitiesDtoSchema,
    skillCount: nonNegativeIntegerSchema,
    linkedSkillIds: z.array(z.string()),
    probe: harnessProbeDtoSchema.nullable(),
    error: ipcErrorSchema.nullable(),
  })
  .strict();

export type HarnessSummaryDto = z.infer<typeof harnessSummaryDtoSchema>;

export const detectedAgentDtoSchema = z
  .object({
    detectorId: z.string(),
    displayName: z.string(),
    agentRoot: pathDtoSchema,
    skillCount: nonNegativeIntegerSchema,
    configured: z.boolean(),
  })
  .strict();

export type DetectedAgentDto = z.infer<typeof detectedAgentDtoSchema>;

export const agentDetectionDiagnosticDtoSchema = z
  .object({
    detectorId: z.string(),
    displayName: z.string(),
    error: ipcErrorSchema,
  })
  .strict();

export type AgentDetectionDiagnosticDto = z.infer<typeof agentDetectionDiagnosticDtoSchema>;

export const detectAgentsResponseDtoSchema = z
  .object({
    agents: z.array(detectedAgentDtoSchema),
    diagnostics: z.array(agentDetectionDiagnosticDtoSchema),
  })
  .strict();

export type DetectAgentsResponseDto = z.infer<typeof detectAgentsResponseDtoSchema>;

export const addDetectedAgentsRequestDtoSchema = z
  .object({ detectorIds: z.array(z.string()).min(1) })
  .strict();

export type AddDetectedAgentsRequestDto = z.infer<typeof addDetectedAgentsRequestDtoSchema>;

export const addDetectedAgentsResponseDtoSchema = z
  .object({ addedAgentIds: z.array(z.string()) })
  .strict();

export type AddDetectedAgentsResponseDto = z.infer<typeof addDetectedAgentsResponseDtoSchema>;

export const deleteAgentsRequestDtoSchema = z
  .object({ agentIds: z.array(z.string()).min(1) })
  .strict();

export type DeleteAgentsRequestDto = z.infer<typeof deleteAgentsRequestDtoSchema>;

export const deleteAgentsResponseDtoSchema = z
  .object({
    deletedAgentIds: z.array(z.string()),
    deletedSkillCount: nonNegativeIntegerSchema,
  })
  .strict();

export type DeleteAgentsResponseDto = z.infer<typeof deleteAgentsResponseDtoSchema>;

export const copyProjectAgentSkillsRequestDtoSchema = z
  .object({
    workspaceId: z.string(),
    agentRoot: z.string().min(1),
    skillIds: z.array(z.string()).min(1),
  })
  .strict();

export type CopyProjectAgentSkillsRequestDto = z.infer<
  typeof copyProjectAgentSkillsRequestDtoSchema
>;

export const copyProjectAgentSkillsResponseDtoSchema = z
  .object({
    skillsRoot: pathDtoSchema,
    copiedSkillIds: z.array(z.string()),
  })
  .strict();

export type CopyProjectAgentSkillsResponseDto = z.infer<
  typeof copyProjectAgentSkillsResponseDtoSchema
>;

export const deleteProjectAgentsRequestDtoSchema = z
  .object({
    workspaceId: z.string(),
    agentIds: z.array(z.string()).min(1),
  })
  .strict();

export type DeleteProjectAgentsRequestDto = z.infer<typeof deleteProjectAgentsRequestDtoSchema>;

export const deleteProjectAgentsResponseDtoSchema = z
  .object({
    deletedAgentIds: z.array(z.string()),
    deletedSkillCount: nonNegativeIntegerSchema,
  })
  .strict();

export type DeleteProjectAgentsResponseDto = z.infer<typeof deleteProjectAgentsResponseDtoSchema>;

export const deploymentModeDtoSchema = z.enum(["copy", "link"]);

export type DeploymentModeDto = z.infer<typeof deploymentModeDtoSchema>;

export const workspaceKindDtoSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("agents") }).strict(),
  z
    .object({
      kind: z.literal("project"),
      root: pathDtoSchema,
    })
    .strict(),
  z
    .object({
      kind: z.literal("linked"),
      root: pathDtoSchema,
      disabledRoot: pathDtoSchema.nullable(),
    })
    .strict(),
]);

export type WorkspaceKindDto = z.infer<typeof workspaceKindDtoSchema>;

export const workspaceSummaryDtoSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    kind: workspaceKindDtoSchema,
    deploymentMode: deploymentModeDtoSchema,
    deploymentCount: nonNegativeIntegerSchema,
  })
  .strict();

export type WorkspaceSummaryDto = z.infer<typeof workspaceSummaryDtoSchema>;

export const workspacesOverviewDtoSchema = z
  .object({
    agentsWorkspaceId: z.string(),
    harnesses: z.array(harnessSummaryDtoSchema),
    workspaces: z.array(workspaceSummaryDtoSchema),
  })
  .strict();

export type WorkspacesOverviewDto = z.infer<typeof workspacesOverviewDtoSchema>;

export const createWorkspaceKindDtoSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("project"),
      root: z.string(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("linked"),
      root: z.string(),
      disabledRoot: z.string().nullable().optional(),
    })
    .strict(),
]);

export type CreateWorkspaceKindDto = z.infer<typeof createWorkspaceKindDtoSchema>;

export const createWorkspaceRequestDtoSchema = z
  .object({
    name: z.string(),
    kind: createWorkspaceKindDtoSchema,
    deploymentMode: deploymentModeDtoSchema,
  })
  .strict();

export type CreateWorkspaceRequestDto = z.infer<typeof createWorkspaceRequestDtoSchema>;

export const workspaceIdRequestDtoSchema = z
  .object({
    workspaceId: z.string(),
  })
  .strict();

export type WorkspaceIdRequestDto = z.infer<typeof workspaceIdRequestDtoSchema>;

export const saveAgentRequestDtoSchema = z
  .object({
    agentId: z.string().nullable(),
    displayName: z.string().min(1),
    agentRoot: z.string().min(1),
    skillIds: z.array(z.string()),
  })
  .strict();

export type SaveAgentRequestDto = z.infer<typeof saveAgentRequestDtoSchema>;

export const saveAgentResponseDtoSchema = z
  .object({
    agentId: z.string(),
    displayName: z.string(),
    agentRoot: pathDtoSchema,
    skillsRoot: pathDtoSchema,
    linkedSkillIds: z.array(z.string()),
    removedSkillIds: z.array(z.string()),
  })
  .strict();

export type SaveAgentResponseDto = z.infer<typeof saveAgentResponseDtoSchema>;

export const targetRoleDtoSchema = z.enum(["primary", "disabled"]);

export type TargetRoleDto = z.infer<typeof targetRoleDtoSchema>;

export const scanModeDtoSchema = z.enum(["flat", "recursive"]);

export type ScanModeDto = z.infer<typeof scanModeDtoSchema>;

export const workspaceTargetDtoSchema = z
  .object({
    harnessId: z.string(),
    path: pathDtoSchema,
    role: targetRoleDtoSchema,
    scanMode: scanModeDtoSchema,
    deploymentMode: deploymentModeDtoSchema,
  })
  .strict();

export type WorkspaceTargetDto = z.infer<typeof workspaceTargetDtoSchema>;

export const discoveryRootDtoSchema = z
  .object({
    path: pathDtoSchema,
    scanMode: scanModeDtoSchema,
  })
  .strict();

export type DiscoveryRootDto = z.infer<typeof discoveryRootDtoSchema>;

export const unsupportedWorkspaceTargetDtoSchema = z
  .object({
    harnessId: z.string(),
    path: pathDtoSchema,
    reason: z.string(),
  })
  .strict();

export type UnsupportedWorkspaceTargetDto = z.infer<typeof unsupportedWorkspaceTargetDtoSchema>;

export const workspaceResolutionDtoSchema = z
  .object({
    targets: z.array(workspaceTargetDtoSchema),
    discoveryRoots: z.array(discoveryRootDtoSchema),
    unsupported: z.array(unsupportedWorkspaceTargetDtoSchema),
  })
  .strict();

export type WorkspaceResolutionDto = z.infer<typeof workspaceResolutionDtoSchema>;

export const deploymentKeyDtoSchema = z
  .object({
    skillId: z.string(),
    harnessId: z.string(),
    workspaceId: z.string(),
  })
  .strict();

export type DeploymentKeyDto = z.infer<typeof deploymentKeyDtoSchema>;

export const deploymentStatusDtoSchema = z.enum([
  "notDeployed",
  "inSync",
  "localNewer",
  "centerNewer",
  "missing",
  "unsupported",
  "error",
]);

export type DeploymentStatusDto = z.infer<typeof deploymentStatusDtoSchema>;

export const observedSkillDtoSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    description: z.string(),
  })
  .strict();

export type ObservedSkillDto = z.infer<typeof observedSkillDtoSchema>;

export const deploymentObservationDtoSchema = z
  .object({
    key: deploymentKeyDtoSchema,
    targetPath: pathDtoSchema,
    role: targetRoleDtoSchema,
    status: deploymentStatusDtoSchema,
    center: observedSkillDtoSchema.nullable(),
    localModifiedAtEpochMillis: z.number().int().nullable(),
  })
  .strict();

export type DeploymentObservationDto = z.infer<typeof deploymentObservationDtoSchema>;

export const skillMarkerDtoSchema = z.enum(["canonical", "legacy"]);

export type SkillMarkerDto = z.infer<typeof skillMarkerDtoSchema>;

export const unmatchedLocalSkillDtoSchema = z
  .object({
    name: z.string(),
    description: z.string(),
    path: pathDtoSchema,
    marker: skillMarkerDtoSchema,
    target: workspaceTargetDtoSchema.nullable(),
  })
  .strict();

export type UnmatchedLocalSkillDto = z.infer<typeof unmatchedLocalSkillDtoSchema>;

export const workspaceDiagnosticDtoSchema = z
  .object({
    path: pathDtoSchema,
    status: deploymentStatusDtoSchema,
    error: ipcErrorSchema,
  })
  .strict();

export type WorkspaceDiagnosticDto = z.infer<typeof workspaceDiagnosticDtoSchema>;

export const workspaceReportDtoSchema = z
  .object({
    workspaceId: z.string(),
    observations: z.array(deploymentObservationDtoSchema),
    unmatchedLocal: z.array(unmatchedLocalSkillDtoSchema),
    diagnostics: z.array(workspaceDiagnosticDtoSchema),
  })
  .strict();

export type WorkspaceReportDto = z.infer<typeof workspaceReportDtoSchema>;

export const projectAgentDtoSchema = z
  .object({
    id: z.string(),
    displayName: z.string(),
    path: pathDtoSchema,
    skillCount: nonNegativeIntegerSchema,
    error: ipcErrorSchema.nullable(),
  })
  .strict();

export type ProjectAgentDto = z.infer<typeof projectAgentDtoSchema>;

export const workspaceObservationDtoSchema = z
  .object({
    workspace: workspaceSummaryDtoSchema,
    resolution: workspaceResolutionDtoSchema,
    report: workspaceReportDtoSchema,
    projectAgents: z.array(projectAgentDtoSchema),
  })
  .strict();

export type WorkspaceObservationDto = z.infer<typeof workspaceObservationDtoSchema>;

export const reconcileReportDtoSchema = z
  .object({
    workspaceId: z.string(),
    imported: z.array(z.string()),
    centerUpdated: z.array(z.string()),
    propagated: z.array(deploymentKeyDtoSchema),
    finalReport: workspaceReportDtoSchema,
  })
  .strict();

export type ReconcileReportDto = z.infer<typeof reconcileReportDtoSchema>;

export const propagationOutcomeDtoSchema = z
  .object({
    workspace: workspaceSummaryDtoSchema,
    report: reconcileReportDtoSchema.nullable(),
    error: ipcErrorSchema.nullable(),
  })
  .strict();

export type PropagationOutcomeDto = z.infer<typeof propagationOutcomeDtoSchema>;

export const workspaceReconcileOutcomeDtoSchema = z
  .object({
    requestedWorkspace: workspaceSummaryDtoSchema,
    requested: reconcileReportDtoSchema,
    propagated: z.array(propagationOutcomeDtoSchema),
  })
  .strict();

export type WorkspaceReconcileOutcomeDto = z.infer<typeof workspaceReconcileOutcomeDtoSchema>;
