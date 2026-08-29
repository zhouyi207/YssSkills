export const SUPPORTED_LANGUAGES = ["zh-CN", "en-US"] as const;

export type AppLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const DEFAULT_LANGUAGE: AppLanguage = "zh-CN";
