import { invoke } from "@tauri-apps/api/core";
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
} from "./types";

/** Typed Tauri command invoker — all backend IPC goes through here. */
export const commands = {
  // Session management
  createSession: () => invoke<string>("create_session"),
  listSessions: () => invoke<SessionInfo[]>("list_sessions"),

  // Auth (session-specific)
  login: (sessionId: string, account: string, password: string) =>
    invoke<SessionDto>("login", { sessionId, account, password }),
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
  saveLoginCredentials: (account: string, password: string, rememberPassword: boolean) =>
    invoke("save_login_credentials", { account, password, rememberPassword }),

  // Accounts (session-specific)
  getGameAccounts: (sessionId: string) =>
    invoke<GameAccountDto[]>("get_game_accounts", { sessionId }),
  refreshAccounts: (sessionId: string) =>
    invoke<GameAccountDto[]>("refresh_accounts", { sessionId }),
  getGameCredentials: (sessionId: string, accountId: string) =>
    invoke<GameCredentialsDto>("get_game_credentials", { sessionId, accountId }),
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
  toggleDebugWindow: (enable: boolean) => invoke("toggle_debug_window", { enable }),
  openLogFolder: () => invoke("open_log_folder"),
  getRecentLogs: () => invoke<string>("get_recent_logs"),
  openWebPopup: (url: string, title: string) => invoke("open_web_popup", { url, title }),
  getWebToken: (sessionId: string) => invoke<string>("get_web_token", { sessionId }),

  // Beanfun points (session-specific)
  openGashPopup: (sessionId: string) => invoke("open_gash_popup", { sessionId }),
  openMemberPopup: (sessionId: string) => invoke("open_member_popup", { sessionId }),
  openCustomerService: () => invoke("open_customer_service"),
  openAuthPopup: (sessionId: string, url: string, title: string) =>
    invoke("open_auth_popup", { sessionId, url, title }),
  pingSession: (sessionId: string) => invoke<boolean>("ping_session", { sessionId }),
  getRemainPoint: (sessionId: string) => invoke<number>("get_remain_point", { sessionId }),

  // Game path detection (global)
  detectGamePath: () => invoke<string | null>("detect_game_path"),

  // Cleanup (global)
  cleanupGameCache: () => invoke<string>("cleanup_game_cache"),

  // GamePass login (TW only — creates its own session, returns sessionId)
  openGamePassLogin: () => invoke<string>("open_gamepass_login"),
} as const;
