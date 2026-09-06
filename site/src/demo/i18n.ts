import useDocusaurusContext from "@docusaurus/useDocusaurusContext";

export type Locale = "zh-TW" | "zh-CN" | "en";

/** Three-way string picker for the demo components: zh-TW, zh-CN, en. */
export function useT() {
  const { i18n } = useDocusaurusContext();
  const key = i18n.currentLocale === "zh-CN" ? 1 : i18n.currentLocale === "en" ? 2 : 0;
  return (zhTW: string, zhCN: string, en: string) => [zhTW, zhCN, en][key];
}

/** The current locale, for the few places that branch on it directly. */
export function useLocale(): Locale {
  const { i18n } = useDocusaurusContext();
  return (i18n.currentLocale as Locale) ?? "zh-TW";
}
