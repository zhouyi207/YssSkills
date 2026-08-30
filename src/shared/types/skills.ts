import { z } from "zod";

import { pathDtoSchema } from "./ipc";

export const skillSourceDtoSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("local"),
      path: pathDtoSchema,
    })
    .strict(),
  z
    .object({
      kind: z.literal("registry"),
      registry: z.string(),
      skill: z.string(),
      version: z.string().nullable(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("git"),
      url: z.string(),
      revision: z.string().nullable(),
      subdirectory: pathDtoSchema.nullable(),
    })
    .strict(),
]);

export type SkillSourceDto = z.infer<typeof skillSourceDtoSchema>;

export const catalogSkillSummaryDtoSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    description: z.string(),
    version: z.string().nullable(),
    source: skillSourceDtoSchema,
    location: pathDtoSchema,
    updatedAtEpochMillis: z.number().int().nullable(),
    deploymentCount: z.number().int().nonnegative(),
  })
  .strict();

export type CatalogSkillSummaryDto = z.infer<typeof catalogSkillSummaryDtoSchema>;

export const catalogSkillsResponseDtoSchema = z
  .object({
    skills: z.array(catalogSkillSummaryDtoSchema),
  })
  .strict();

export type CatalogSkillsResponseDto = z.infer<typeof catalogSkillsResponseDtoSchema>;

export const catalogSkillDetailDtoSchema = z
  .object({
    skill: catalogSkillSummaryDtoSchema,
    body: z.string(),
  })
  .strict();

export type CatalogSkillDetailDto = z.infer<typeof catalogSkillDetailDtoSchema>;

export const skillIdRequestDtoSchema = z
  .object({
    skillId: z.string(),
  })
  .strict();

export type SkillIdRequestDto = z.infer<typeof skillIdRequestDtoSchema>;
