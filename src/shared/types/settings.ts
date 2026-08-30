import { z } from "zod";

import { pathDtoSchema } from "./ipc";

export const appSettingsDtoSchema = z
  .object({
    catalogRoot: pathDtoSchema,
  })
  .strict();

export type AppSettingsDto = z.infer<typeof appSettingsDtoSchema>;

export const updateCatalogRootRequestDtoSchema = z
  .object({
    catalogRoot: z.string(),
  })
  .strict();

export type UpdateCatalogRootRequestDto = z.infer<typeof updateCatalogRootRequestDtoSchema>;

export const SUPPORTED_LANGUAGES = ["zh-CN", "en-US"] as const;

export type AppLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const DEFAULT_LANGUAGE: AppLanguage = "zh-CN";
