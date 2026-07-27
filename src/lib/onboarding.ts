/**
 * First-run guide.
 *
 * Bump `ONBOARDING_ID` to show the guide again to everyone — worth doing when a
 * page is added or something it describes changes. The copy lives in i18n under
 * `onboarding.*`; each page here just names its icon and key suffix.
 */
export const ONBOARDING_ID = "v1";

export interface OnboardingPage {
  /** Suffix of the `onboarding.<key>_title` / `_body` i18n keys. */
  key: string;
  icon: string;
}

export const ONBOARDING_PAGES: OnboardingPage[] = [
  { key: "login", icon: "🔑" },
  { key: "play", icon: "▶️" },
  { key: "toolbox", icon: "🛠" },
  { key: "classic", icon: "🍁" },
  { key: "cafe", icon: "🖥️" },
  { key: "captcha", icon: "🛡️" },
];
