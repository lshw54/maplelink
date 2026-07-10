import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  SessionDto,
  SessionInfo,
  QrCodeData,
  QrPollResult,
  GameAccountDto,
  GameCredentialsDto,
  AppConfigDto,
  UpdateInfoDto,
  SavedAccountDto,
  LastSavedAccountDto,
  AdvanceCheckState,
  WebLaunchStatus,
  WebLaunchTestCode,
  DnsStatus,
  DnsTestResult,
  GameDownloadDto,
} from "./types";

/** Typed Tauri command invoker — all backend IPC goes through here. */
export const commands = {
  // Session management
  createSession: () => invoke<string>("create_session"),
  listSessions: () => invoke<SessionInfo[]>("list_sessions"),

  // Auth (session-specific)
  login: (sessionId: string, account: string, password: string) =>
    invoke<SessionDto>("login", { sessionId, account, password }),
  // TW Regular (帳密) two-phase reCAPTCHA login
  twLoginCheck: (sessionId: string, account: string, recaptchaCheck: string) =>
    invoke("tw_login_check", { sessionId, account, recaptchaCheck }),
  twLoginSubmit: (sessionId: string, password: string, recaptchaLogin: string) =>
    invoke<SessionDto>("tw_login_submit", { sessionId, password, recaptchaLogin }),
  // TW regular login done fully inside a beanfun webview (credentials prefilled)
  openRegularWebLogin: (sessionId: string, account: string, password: string) =>
    invoke("open_regular_web_login", { sessionId, account, password }),
  openRecaptchaWindow: (step: "check" | "login") => invoke("open_recaptcha_window", { step }),
  closeRecaptchaWindow: () => invoke("close_recaptcha_window"),
  qrLoginStart: (sessionId: string) => invoke<QrCodeData>("qr_login_start", { sessionId }),
  qrLoginPoll: (sessionId: string, sessionKey: string, verificationToken: string) =>
    invoke<QrPollResult>("qr_login_poll", { sessionId, sessionKey, verificationToken }),
  totpVerify: (sessionId: string, code: string) =>
    invoke<SessionDto>("totp_verify", { sessionId, code }),
  getAdvanceCheck: (sessionId: string, url?: string) =>
    invoke<AdvanceCheckState>("get_advance_check", { sessionId, url: url ?? null }),
  submitAdvanceCheck: (
    sessionId: string,
    params: {
      viewstate: string;
      viewstateGenerator: string;
      eventValidation: string;
      samplecaptcha: string;
      submitUrl: string;
      verifyCode: string;
      captchaCode: string;
    },
  ) => invoke<boolean>("submit_advance_check", { sessionId, ...params }),
  refreshAdvanceCheckCaptcha: (sessionId: string, samplecaptcha: string) =>
    invoke<string>("refresh_advance_check_captcha", { sessionId, samplecaptcha }),
  logout: (sessionId: string) => invoke("logout", { sessionId }),

  // Saved accounts (global — no sessionId)
  getSavedAccounts: () => invoke<SavedAccountDto[]>("get_saved_accounts"),
  getAllSavedAccounts: () => invoke<SavedAccountDto[]>("get_all_saved_accounts"),
  getLastSavedAccount: () => invoke<LastSavedAccountDto | null>("get_last_saved_account"),
  getSavedAccountDetail: (account: string) =>
    invoke<LastSavedAccountDto | null>("get_saved_account_detail", { account }),
  deleteSavedAccount: (account: string, region?: string) =>
    invoke<boolean>("delete_saved_account", { account, region }),
  saveVerifyInfo: (account: string, verifyInfo: string) =>
    invoke("save_verify_info", { account, verifyInfo }),
  saveLoginCredentials: (account: string, password: string, rememberPassword: boolean) =>
    invoke("save_login_credentials", { account, password, rememberPassword }),

  // Accounts (session-specific)
  getGameAccounts: (sessionId: string) =>
    invoke<GameAccountDto[]>("get_game_accounts", { sessionId }),
  refreshAccounts: (sessionId: string) =>
    invoke<GameAccountDto[]>("refresh_accounts", { sessionId }),
  getGameCredentials: (sessionId: string, accountId: string) =>
    invoke<GameCredentialsDto>("get_game_credentials", { sessionId, accountId }),
  getAccountCreateTime: (sessionId: string, accountId: string) =>
    invoke<string>("get_account_create_time", { sessionId, accountId }),
  autoPasteOtp: (sessionId: string, accountId: string) =>
    invoke<boolean>("auto_paste_otp", { sessionId, accountId }),
  changeAccountDisplayName: (sessionId: string, accountId: string, newName: string) =>
    invoke<boolean>("change_account_display_name", { sessionId, accountId, newName }),
  setDisplayOverride: (accountId: string, displayName: string) =>
    invoke("set_display_override", { accountId, displayName }),
  getDisplayOverrides: () => invoke<Record<string, string>>("get_display_overrides"),
  setAccountOrder: (order: string[]) => invoke("set_account_order", { order }),
  getAuthEmail: (sessionId: string) => invoke<string>("get_auth_email", { sessionId }),

  // Launcher
  launchGame: (sessionId: string, accountId: string, otp?: string) =>
    invoke<number>("launch_game", { sessionId, accountId, otp: otp ?? null }),
  launchGameDirect: () => invoke<number>("launch_game_direct"),
  isGameRunning: () => invoke<boolean>("is_game_running"),
  getGamePid: () => invoke<number>("get_game_pid"),
  getProcessStatus: (sessionId: string, pid: number) =>
    invoke<boolean>("get_process_status", { sessionId, pid }),
  killGame: () => invoke("kill_game"),

  // Config (global)
  getConfig: () => invoke<AppConfigDto>("get_config"),
  setConfig: (key: string, value: string) => invoke("set_config", { key, value }),

  // Update (global)
  checkUpdate: () => invoke<UpdateInfoDto | null>("check_update", { manual: true }),
  applyUpdate: (downloadUrl: string, useProxy?: boolean) =>
    invoke<string>("apply_update", { downloadUrl, useProxy }),
  testGithubAccess: () => invoke<boolean>("test_github_access"),
  restartApp: () => invoke("restart_app"),

  // System (global unless noted)
  resizeWindow: (page: string) => invoke("resize_window", { page }),
  openFileDialog: () => invoke<string | null>("open_file_dialog"),
  getAppVersion: () => invoke<string>("get_app_version"),
  getTextScaleFactor: () => invoke<number>("get_text_scale_factor"),
  getPlatformInfo: () => invoke<string>("get_platform_info"),
  logFrontendError: (level: string, module: string, message: string) =>
    invoke("log_frontend_error", { level, module, message }),
  // Web-login game-launch interception (opt-in registry toggle)
  setWebLaunchIntercept: (enabled: boolean) => invoke("set_web_launch_intercept", { enabled }),
  getWebLaunchInterceptStatus: () => invoke<boolean>("get_web_launch_intercept_status"),
  getWebLaunchStatus: () => invoke<WebLaunchStatus>("get_web_launch_status"),
  webLaunchTestGame: () => invoke<WebLaunchTestCode>("web_launch_test_game"),
  webLaunchTestGamania: () => invoke<WebLaunchTestCode>("web_launch_test_gamania"),
  toggleDebugWindow: (enable: boolean) => invoke("toggle_debug_window", { enable }),
  openLogFolder: () => invoke("open_log_folder"),
  getRecentLogs: () => invoke<string>("get_recent_logs"),
  openWebPopup: (url: string, title: string) => invoke("open_web_popup", { url, title }),
  getWebToken: (sessionId: string) => invoke<string>("get_web_token", { sessionId }),

  // Beanfun points (session-specific)
  openGashPopup: (sessionId: string) => invoke("open_gash_popup", { sessionId }),
  openMemberPopup: (sessionId: string) => invoke("open_member_popup", { sessionId }),
  openCustomerService: (sessionId: string) => invoke("open_customer_service", { sessionId }),
  openAuthPopup: (sessionId: string, url: string, title: string) =>
    invoke("open_auth_popup", { sessionId, url, title }),
  pingSession: (sessionId: string) => invoke<boolean>("ping_session", { sessionId }),
  getRemainPoint: (sessionId: string) => invoke<number>("get_remain_point", { sessionId }),

  // Game path detection (global)
  detectGamePath: () => invoke<string | null>("detect_game_path"),

  // Cleanup (global)
  cleanupGameCache: () => invoke<string>("cleanup_game_cache"),
  resetWebviewData: () => invoke("reset_webview_data"),

  // Official client download list (global)
  getGameDownloadList: () => invoke<GameDownloadDto[]>("get_game_download_list"),

  // Announcement "seen" state (global; stored outside config.ini)
  announcementIsSeen: (id: string) => invoke<boolean>("announcement_is_seen", { id }),
  announcementMarkSeen: (id: string) => invoke("announcement_mark_seen", { id }),

  // Window close behaviour ("quit" | "tray")
  resolveAppClose: (action: "quit" | "tray") => invoke("resolve_app_close", { action }),

  // Network / DNS (global)
  getDnsStatus: () => invoke<DnsStatus>("get_dns_status"),
  testDns: () => invoke<DnsTestResult>("test_dns"),
  setRecommendedDns: () => invoke("set_recommended_dns"),
  resetDnsAuto: () => invoke("reset_dns_auto"),

  // GamePass login (TW only — creates its own session, returns sessionId)
  openGamePassLogin: () => invoke<string>("open_gamepass_login"),
} as const;

/** Payload of the `recaptcha-token` event emitted when the helper window captures a token. */
interface RecaptchaTokenEvent {
  token: string;
  step: "check" | "login";
}

/**
 * Open the external reCAPTCHA helper window for the given step and resolve with
 * the token once the user solves the challenge. Rejects if the user closes the
 * window first (`recaptcha-cancelled`).
 */
export async function solveRecaptcha(
  step: "check" | "login",
  timeoutMs = 180_000,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let settled = false;
    const cleanups: Array<() => void> = [];
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      cleanups.forEach((c) => c());
      fn();
    };

    void listen<RecaptchaTokenEvent>("recaptcha-token", (e) => {
      if (e.payload.step !== step) return;
      finish(() => resolve(e.payload.token));
    }).then((un) => cleanups.push(un));

    void listen("recaptcha-cancelled", () => {
      finish(() => reject(new Error("RECAPTCHA_CANCELLED")));
    }).then((un) => cleanups.push(un));

    // Safety net: never let a stuck/blank helper window hang the login forever.
    const timer = setTimeout(() => {
      void commands.closeRecaptchaWindow().catch(() => {});
      finish(() => reject(new Error("RECAPTCHA_TIMEOUT")));
    }, timeoutMs);
    cleanups.push(() => clearTimeout(timer));

    commands
      .openRecaptchaWindow(step)
      .catch((err) => finish(() => reject(err instanceof Error ? err : new Error(String(err)))));
  });
}

/**
 * Run the TW regular login entirely inside a beanfun webview (credentials
 * prefilled; the user solves reCAPTCHA + any advance check). Resolves with the
 * session once the backend harvests cookies, rejects on error/cancel/timeout.
 */
export async function webLogin(
  sessionId: string,
  account: string,
  password: string,
  timeoutMs = 300_000,
): Promise<SessionDto> {
  return new Promise<SessionDto>((resolve, reject) => {
    let settled = false;
    const cleanups: Array<() => void> = [];
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      cleanups.forEach((c) => c());
      fn();
    };

    void listen<SessionDto>("regular-login-complete", (e) => {
      finish(() => resolve(e.payload));
    }).then((un) => cleanups.push(un));

    void listen<string>("regular-login-error", (e) => {
      finish(() => reject(new Error(e.payload || "WEBLOGIN_ERROR")));
    }).then((un) => cleanups.push(un));

    void listen("regular-login-cancelled", () => {
      finish(() => reject(new Error("WEBLOGIN_CANCELLED")));
    }).then((un) => cleanups.push(un));

    // Safety net so a stuck window never hangs the login button forever.
    const timer = setTimeout(() => finish(() => reject(new Error("WEBLOGIN_TIMEOUT"))), timeoutMs);
    cleanups.push(() => clearTimeout(timer));

    commands
      .openRegularWebLogin(sessionId, account, password)
      .catch((err) => finish(() => reject(err instanceof Error ? err : new Error(String(err)))));
  });
}
