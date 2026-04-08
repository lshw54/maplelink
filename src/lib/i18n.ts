import { useCallback } from "react";
import { useUiStore, type Language } from "./stores/ui-store";
import enUS from "../locales/en-US.json";
import zhTW from "../locales/zh-TW.json";
import zhCN from "../locales/zh-CN.json";

type LocaleMessages = Record<string, string>;

const locales: Record<Language, LocaleMessages> = {
  "en-US": enUS,
  "zh-TW": zhTW,
  "zh-CN": zhCN,
};

/**
 * Interpolate `{{param}}` placeholders in a string with provided values.
 */
function interpolate(template: string, params?: Record<string, string>): string {
  if (!params) return template;
  return template.replace(/\{\{(\w+)\}\}/g, (_, key: string) => {
    return params[key] ?? `{{${key}}}`;
  });
}

/**
 * Look up a translation key for the given language.
 * Falls back to en-US if the key is missing in the selected locale.
 * Returns the raw key if not found in any locale.
 * Supports `{{param}}` interpolation.
 */
export function getTranslation(
  language: Language,
  key: string,
  params?: Record<string, string>,
): string {
  const messages = locales[language] ?? locales["en-US"];
  const value = messages[key] ?? locales["en-US"][key] ?? key;
  return interpolate(value, params);
}

/**
 * React hook that returns a `t()` function bound to the current language
 * from the UI store. Re-renders automatically when language changes.
 */
export function useTranslation() {
  const language = useUiStore((state) => state.language);

  const t = useCallback(
    (key: string, params?: Record<string, string>) => getTranslation(language, key, params),
    [language],
  );

  return { t, language };
}
