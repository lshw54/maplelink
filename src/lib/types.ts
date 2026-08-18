/** Mirrors of Rust models for type-safe IPC. */

export interface SessionDto {
  sessionId: string;
  token: string;
  region: "TW" | "HK";
  accountName: string;
  expiresAt: string;
}

export interface SessionInfo {
  id: string;
  accountName: string;
  region: string;
}

export interface GameAccountDto {
  id: string;
  displayName: string;
  gameType: string;
  sn: string;
  status: string;
  createdAt: string;
}

export interface GameCredentialsDto {
  accountId: string;
  otp: string;
  retrievedAt: string;
}

export interface GameDownloadDto {
  id: number;
  name: string;
  size: string;
  url: string;
  kind: "game" | "patch" | "other";
}

/** One selectable game account from GamaPass's classic sign-in chooser. */
export interface ClassicAccountDto {
  value: string;
  label: string;
}

export interface ClassicCheckDto {
  ngmRegistered: boolean;
  ngmExe: string | null;
  ngmExeExists: boolean;
  webview2Version: string | null;
  gameExe: string | null;
}

export interface AppConfigDto {
  gamePath: string;
  locale: string;
  theme: "system" | "dark" | "light";
  language: "en-US" | "zh-TW" | "zh-CN";
  autoUpdate: boolean;
  skipPlayConfirm: boolean;
  autoStart: boolean;
  region: "TW" | "HK";
  debugLogging: boolean;
  gamepassIncognito: boolean;
  updateChannel: "release" | "pre-release";
  fontSize: "small" | "medium" | "large" | "extra-large";
  traditionalLogin: boolean;
  autoKillPatcher: boolean;
  accountViewMode: "card" | "list";
  autoLogin: boolean;
  autoLaunchGame: boolean;
  webLaunchAutoLaunch: boolean;
  webLaunchAutoPaste: boolean;
  closeBehavior: "ask" | "quit" | "tray";
  hideAccountNames: boolean;
  beanfunRenameDismissed: boolean;
  cafeMode: boolean;
  classicNgmPath: string;
  announcementDismissedId: string;
  webviewViaProxy: boolean;
  otpAutoInput: boolean;
  defaultLoginView: "normal" | "qr";
  githubHosts: boolean;
  compactUi: boolean;
}

/** Result of the startup "rename exe to Beanfun.exe" check (China-IP users). */
export interface BeanfunRenameCheck {
  suggest: boolean;
  collision: boolean;
  currentName: string;
  targetName: string;
}

export interface ErrorDto {
  code: string;
  message: string;
  category: "authentication" | "network" | "filesystem" | "process" | "configuration" | "update";
  details?: string;
}

export interface UpdateInfoDto {
  version: string;
  changelog: string;
  downloadUrl: string;
  isPrerelease: boolean;
}

export interface QrCodeData {
  sessionKey: string;
  qrImageUrl: string;
  verificationToken: string;
  deeplink: string;
}

export interface QrPollResult {
  status: "pending" | "scanned" | "confirmed" | "expired";
  session?: SessionDto;
}

export interface SavedAccountDto {
  account: string;
  region: string;
  hasPassword: boolean;
  rememberPassword: boolean;
}

export interface LastSavedAccountDto {
  account: string;
  password: string;
  rememberPassword: boolean;
  verifyInfo?: string | null;
}

export interface WebLaunchStatus {
  registered: boolean;
  gamePath: string;
  gamePathOk: boolean;
  lrReady: boolean;
  gamaniaInstalled: boolean;
  exeName: string;
  exeNameOk: boolean;
}

/** Stable codes returned by the live launch tests, mapped to i18n in the UI. */
export type WebLaunchTestCode =
  | "ok"
  | "skipped_running"
  | "no_game_path"
  | "spawn_failed"
  | "not_found";

export interface AdvanceCheckState {
  viewstate: string;
  viewstateGenerator: string;
  eventValidation: string;
  samplecaptcha: string;
  submitUrl: string;
  captchaImageBase64: string;
  authHint: string;
}
