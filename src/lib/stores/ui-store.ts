import { create } from "zustand";
import { commands } from "../tauri";
import type { ClassicAccountDto, GameCredentialsDto } from "../types";
import { useConfigStore } from "./config-store";
import { useAuthStore } from "./auth-store";
import { ANNOUNCEMENT_ID } from "../announcement";

/** The announcement banner is chrome the backend sizes pages around. */
export function announcementBarShown(): boolean {
  return useConfigStore.getState().config?.announcementDismissedId !== ANNOUNCEMENT_ID;
}

/** Whether the "update available" strip is on screen. App owns it. */
let updateBar = false;
export function setUpdateBarShown(shown: boolean): void {
  updateBar = shown;
}

/**
 * Size the window for `page`, counting whichever banners are on screen.
 *
 * Every resize goes through here. Callers that left a flag out used to get the
 * backend's default for it, so the same page came out 28px taller or shorter
 * depending on which path sized it last — which is how the compact launcher
 * could open one account row short on the first login and fine on the next.
 */
export function resizeWindow(page: string): Promise<unknown> {
  return commands.resizeWindow(mainVariant(page), announcementBarShown(), updateBar);
}

/** The compact launcher shows whole rows for up to five game accounts. */
const TALL_MAIN_ACCOUNTS = 5;

/**
 * The main page grows only for a session that needs it: one with five or more
 * game accounts gets the taller window, so the fifth row is not cut in half
 * above the OTP card. Everyone else keeps the smaller one.
 */
function mainVariant(page: string): string {
  if (page !== "main") return page;
  const { sessions, activeSessionId } = useAuthStore.getState();
  const count = activeSessionId ? (sessions.get(activeSessionId)?.gameAccounts.length ?? 0) : 0;
  return count >= TALL_MAIN_ACCOUNTS ? "main-tall" : page;
}

type Page = "login" | "main" | "toolbox" | "web_launch";
export type ThemeMode = "system" | "dark" | "light";
export type Language = "en-US" | "zh-TW" | "zh-CN";

export interface UiState {
  currentPage: Page;
  previousPage: Page;
  theme: ThemeMode;
  language: Language;
  gamePid: number | null;
  gameRunning: boolean;
  /** When true, LoginPage won't auto-redirect to main even if authenticated. */
  addingSession: boolean;
  /**
   * MapleStory Classic (懷舊服) login mode. Ephemeral (per session) — when set,
   * a successful login opens the classic portal webview instead of the regular
   * game account grid. Phase 1 is HK id-pass only.
   */
  classicMode: boolean;
  /**
   * Classic launch progress, shown as an overlay after a classic login since the
   * flow runs in a hidden window with no page of its own.
   */
  classicStatus: "idle" | "launching" | "launched" | "failed" | "needs_login";
  /**
   * GamaPass game accounts awaiting a pick. Set when the classic sign-in offers
   * more than one; null the rest of the time (a single account goes straight
   * through without asking).
   */
  classicAccounts: ClassicAccountDto[] | null;
  /** First-run guide is on screen. Also settable from the toolbox to replay it. */
  onboardingOpen: boolean;
  /** Announcement overlay is on screen. Openable from the toolbox archive. */
  announcementOpen: boolean;
  /**
   * Persisted login view so QR form survives page switches. Empty string
   * means "not yet set this session" — LoginPage falls back to the user's
   * configured default (config.defaultLoginView) only in that case, so a
   * mid-session choice back to "normal" isn't overridden on remount.
   */
  loginView: string;
  /** Persisted QR login state so it survives qr-viewer round-trip. */
  qrSessionId: string | null;
  qrData: { sessionKey: string; qrImageUrl: string; verificationToken: string } | null;
  /**
   * When the current code was issued, so the countdown survives leaving the QR
   * view and coming back — without it, a code with thirty seconds left would
   * come back showing a fresh three minutes.
   */
  qrIssuedAt: number | null;
  /**
   * The last OTP fetched for each game account, so a session tab (or the
   * toolbox round-trip) shows the code it had rather than a blank readout.
   */
  otpByAccount: Record<string, GameCredentialsDto>;
  setOtp: (accountId: string, data: GameCredentialsDto) => void;
  setPage: (page: Page) => void;
  goBack: () => void;
  setTheme: (theme: ThemeMode) => void;
  setLanguage: (language: Language) => void;
  setGamePid: (pid: number | null) => void;
  setGameRunning: (running: boolean) => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  currentPage: "login",
  previousPage: "login",
  theme: "dark",
  language: "zh-TW",
  gamePid: null,
  gameRunning: false,
  addingSession: false,
  classicMode: false,
  classicStatus: "idle",
  classicAccounts: null,
  onboardingOpen: false,
  announcementOpen: false,
  loginView: "",
  qrSessionId: null,
  qrData: null,
  qrIssuedAt: null,
  otpByAccount: {},
  setOtp: (accountId, data) =>
    set((state) => ({ otpByAccount: { ...state.otpByAccount, [accountId]: data } })),
  setPage: (page) => {
    const current = get().currentPage;
    // Remember a non-overlay page so goBack() returns to it from toolbox/web_launch.
    const prev = current !== "toolbox" && current !== "web_launch" ? current : get().previousPage;
    set({ currentPage: page, previousPage: prev });
    resizeWindow(page).catch((e) => {
      commands.logFrontendError("warn", "ui-store", `resize failed for ${page}: ${e}`);
    });
  },
  goBack: () => {
    const prev = get().previousPage;
    set({ currentPage: prev });
    resizeWindow(prev).catch((e) => {
      commands.logFrontendError("warn", "ui-store", `resize failed for ${prev}: ${e}`);
    });
  },
  setTheme: (theme) => set({ theme }),
  setLanguage: (language) => set({ language }),
  setGamePid: (pid) => set({ gamePid: pid }),
  setGameRunning: (running) => {
    set({ gameRunning: running });
    if (!running) set({ gamePid: null });
  },
}));
