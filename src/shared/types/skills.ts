import { z } from "zod";

import { ipcErrorSchema, pathDtoSchema } from "./ipc";

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

export const scanImportFolderRequestDtoSchema = z
  .object({
    root: z.string().min(1),
  })
  .strict();

export type ScanImportFolderRequestDto = z.infer<typeof scanImportFolderRequestDtoSchema>;

export const importCandidateDtoSchema = z
  .object({
    path: pathDtoSchema,
    name: z.string(),
    description: z.string(),
    version: z.string().nullable(),
  })
  .strict();

export type ImportCandidateDto = z.infer<typeof importCandidateDtoSchema>;

export const importFolderDiagnosticDtoSchema = z
  .object({
    path: pathDtoSchema,
    error: ipcErrorSchema,
  })
  .strict();

export type ImportFolderDiagnosticDto = z.infer<typeof importFolderDiagnosticDtoSchema>;

export const scanImportFolderResponseDtoSchema = z
  .object({
    root: pathDtoSchema,
    candidates: z.array(importCandidateDtoSchema),
    diagnostics: z.array(importFolderDiagnosticDtoSchema),
  })
  .strict();

export type ScanImportFolderResponseDto = z.infer<typeof scanImportFolderResponseDtoSchema>;

export const importLocalSkillsRequestDtoSchema = z
  .object({
    root: z.string().min(1),
    paths: z.array(z.string().min(1)).min(1),
  })
  .strict();

export type ImportLocalSkillsRequestDto = z.infer<typeof importLocalSkillsRequestDtoSchema>;

export const importLocalSkillsResponseDtoSchema = z
  .object({
    importedSkillIds: z.array(z.string()),
    skippedPaths: z.array(pathDtoSchema),
  })
  .strict();

export type ImportLocalSkillsResponseDto = z.infer<typeof importLocalSkillsResponseDtoSchema>;

export const exportCatalogSkillsRequestDtoSchema = z
  .object({
    destinationRoot: z.string().min(1),
    skillIds: z.array(z.string()).min(1),
  })
  .strict();

export type ExportCatalogSkillsRequestDto = z.infer<typeof exportCatalogSkillsRequestDtoSchema>;

export const exportCatalogSkillsResponseDtoSchema = z
  .object({
    exportRoot: pathDtoSchema,
    exportedSkillIds: z.array(z.string()),
  })
  .strict();

export type ExportCatalogSkillsResponseDto = z.infer<typeof exportCatalogSkillsResponseDtoSchema>;

export const skillIdRequestDtoSchema = z
  .object({
    skillId: z.string(),
  })
  .strict();

export type SkillIdRequestDto = z.infer<typeof skillIdRequestDtoSchema>;

export const deleteCatalogSkillsRequestDtoSchema = z
  .object({
    skillIds: z.array(z.string()).min(1),
  })
  .strict();

export type DeleteCatalogSkillsRequestDto = z.infer<typeof deleteCatalogSkillsRequestDtoSchema>;

export const deleteCatalogSkillsResponseDtoSchema = z
  .object({
    deletedSkillIds: z.array(z.string()),
  })
  .strict();

export type DeleteCatalogSkillsResponseDto = z.infer<typeof deleteCatalogSkillsResponseDtoSchema>;
