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
    deploymentCount: nonNegativeIntegerSchema,
    probe: harnessProbeDtoSchema.nullable(),
    error: ipcErrorSchema.nullable(),
  })
  .strict();

export type HarnessSummaryDto = z.infer<typeof harnessSummaryDtoSchema>;

export const deploymentModeDtoSchema = z.enum(["copy", "symbolicLink", "junction"]);

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

export const workspaceObservationDtoSchema = z
  .object({
    workspace: workspaceSummaryDtoSchema,
    resolution: workspaceResolutionDtoSchema,
    report: workspaceReportDtoSchema,
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
