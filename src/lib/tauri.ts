import { invoke } from "@tauri-apps/api/core";
import type {
  SessionDto,
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
  // Auth
  login: (account: string, password: string) => invoke<SessionDto>("login", { account, password }),
  qrLoginStart: () => invoke<QrCodeData>("qr_login_start"),
  qrLoginPoll: (sessionKey: string) => invoke<QrPollResult>("qr_login_poll", { sessionKey }),
  totpVerify: (code: string) => invoke<SessionDto>("totp_verify", { code }),
  getAdvanceCheck: (url?: string) =>
    invoke<AdvanceCheckState>("get_advance_check", { url: url ?? null }),
  submitAdvanceCheck: (params: {
    viewstate: string;
    viewstateGenerator: string;
    eventValidation: string;
    samplecaptcha: string;
    submitUrl: string;
    verifyCode: string;
    captchaCode: string;
  }) => invoke<boolean>("submit_advance_check", params),
  refreshAdvanceCheckCaptcha: (samplecaptcha: string) =>
    invoke<string>("refresh_advance_check_captcha", { samplecaptcha }),
  logout: () => invoke("logout"),

  // Saved accounts
  getSavedAccounts: () => invoke<SavedAccountDto[]>("get_saved_accounts"),
  getAllSavedAccounts: () => invoke<SavedAccountDto[]>("get_all_saved_accounts"),
  getLastSavedAccount: () => invoke<LastSavedAccountDto | null>("get_last_saved_account"),
  getSavedAccountDetail: (account: string) =>
    invoke<LastSavedAccountDto | null>("get_saved_account_detail", { account }),
  deleteSavedAccount: (account: string, region?: string) =>
    invoke<boolean>("delete_saved_account", { account, region }),
  saveLoginCredentials: (account: string, password: string, rememberPassword: boolean) =>
    invoke("save_login_credentials", { account, password, rememberPassword }),

  // Accounts
  getGameAccounts: () => invoke<GameAccountDto[]>("get_game_accounts"),
  refreshAccounts: () => invoke<GameAccountDto[]>("refresh_accounts"),
  getGameCredentials: (accountId: string) =>
    invoke<GameCredentialsDto>("get_game_credentials", { accountId }),
  autoPasteOtp: (accountId: string) => invoke<boolean>("auto_paste_otp", { accountId }),

  // Account context menu actions
  changeAccountDisplayName: (accountId: string, newName: string) =>
    invoke<boolean>("change_account_display_name", { accountId, newName }),
  getAuthEmail: () => invoke<string>("get_auth_email"),

  // Launcher
  launchGame: (accountId: string, otp?: string) =>
    invoke<number>("launch_game", { accountId, otp: otp ?? null }),
  isGameRunning: () => invoke<boolean>("is_game_running"),
  getProcessStatus: (pid: number) => invoke<boolean>("get_process_status", { pid }),
  killGame: () => invoke("kill_game"),

  // Config
  getConfig: () => invoke<AppConfigDto>("get_config"),
  setConfig: (key: string, value: string) => invoke("set_config", { key, value }),

  // Update
  checkUpdate: () => invoke<UpdateInfoDto | null>("check_update", { manual: true }),
  applyUpdate: (downloadUrl: string, useProxy?: boolean) =>
    invoke<string>("apply_update", { downloadUrl, useProxy }),
  testGithubAccess: () => invoke<boolean>("test_github_access"),

  // System
  resizeWindow: (page: string) => invoke("resize_window", { page }),
  openFileDialog: () => invoke<string | null>("open_file_dialog"),
  getAppVersion: () => invoke<string>("get_app_version"),
  logFrontendError: (level: string, module: string, message: string) =>
    invoke("log_frontend_error", { level, module, message }),
  toggleDebugWindow: (enable: boolean) => invoke("toggle_debug_window", { enable }),
  openLogFolder: () => invoke("open_log_folder"),
  getRecentLogs: () => invoke<string>("get_recent_logs"),
  openWebPopup: (url: string, title: string) => invoke("open_web_popup", { url, title }),
  getWebToken: () => invoke<string>("get_web_token"),

  // Beanfun points
  openGashPopup: () => invoke("open_gash_popup"),
  openMemberPopup: () => invoke("open_member_popup"),
  openCustomerService: () => invoke("open_customer_service"),
  pingSession: () => invoke<boolean>("ping_session"),
  getRemainPoint: () => invoke<number>("get_remain_point"),

  // Game path detection
  detectGamePath: () => invoke<string | null>("detect_game_path"),

  // Cleanup
  cleanupGameCache: () => invoke<string>("cleanup_game_cache"),

  // GamePass login (TW only)
  openGamePassLogin: () => invoke("open_gamepass_login"),
} as const;
