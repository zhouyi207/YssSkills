import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES, type AppLanguage } from "@/shared/types/settings";
import { zhCN } from "./locales/zh-CN";
import { enUS } from "./locales/en-US";

export { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES, type AppLanguage };

void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { translation: zhCN },
    "en-US": { translation: enUS },
  },
  lng: DEFAULT_LANGUAGE,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
});

export { i18n };
