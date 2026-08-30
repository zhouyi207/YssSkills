import { z } from "zod";

const nonNegativeIntegerSchema = z.number().int().nonnegative();

export const dashboardCountsDtoSchema = z
  .object({
    skills: nonNegativeIntegerSchema,
    deployments: nonNegativeIntegerSchema,
    detectedHarnesses: nonNegativeIntegerSchema,
    workspaces: nonNegativeIntegerSchema,
  })
  .strict();

export type DashboardCountsDto = z.infer<typeof dashboardCountsDtoSchema>;

export const dashboardActivityDtoSchema = z
  .object({
    periodStartEpochMillis: z.number().int(),
    imported: nonNegativeIntegerSchema,
    updated: nonNegativeIntegerSchema,
  })
  .strict();

export type DashboardActivityDto = z.infer<typeof dashboardActivityDtoSchema>;

export const simpleDiagnosticDtoSchema = z
  .object({
    code: z.string(),
    message: z.string(),
  })
  .strict();

export type SimpleDiagnosticDto = z.infer<typeof simpleDiagnosticDtoSchema>;

export const dashboardOverviewDtoSchema = z
  .object({
    counts: dashboardCountsDtoSchema,
    activity: z.array(dashboardActivityDtoSchema),
    diagnostics: z.array(simpleDiagnosticDtoSchema),
  })
  .strict();

export type DashboardOverviewDto = z.infer<typeof dashboardOverviewDtoSchema>;
