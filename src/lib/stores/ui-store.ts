import { create } from "zustand";
import { commands } from "../tauri";

export type Page = "login" | "main" | "toolbox" | "web_launch";
export type ThemeMode = "system" | "dark" | "light";
export type Language = "en-US" | "zh-TW" | "zh-CN";

export interface UiState {
  currentPage: Page;
  previousPage: Page;
  theme: ThemeMode;
  language: Language;
  sidebarOpen: boolean;
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
  classicStatus: "idle" | "launching" | "launched" | "failed";
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
  setPage: (page: Page) => void;
  goBack: () => void;
  setTheme: (theme: ThemeMode) => void;
  setLanguage: (language: Language) => void;
  setSidebarOpen: (open: boolean) => void;
  toggleSidebar: () => void;
  setGamePid: (pid: number | null) => void;
  setGameRunning: (running: boolean) => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  currentPage: "login",
  previousPage: "login",
  theme: "dark",
  language: "zh-TW",
  sidebarOpen: false,
  gamePid: null,
  gameRunning: false,
  addingSession: false,
  classicMode: false,
  classicStatus: "idle",
  loginView: "",
  qrSessionId: null,
  qrData: null,
  setPage: (page) => {
    const current = get().currentPage;
    // Remember a non-overlay page so goBack() returns to it from toolbox/web_launch.
    const prev = current !== "toolbox" && current !== "web_launch" ? current : get().previousPage;
    set({ currentPage: page, previousPage: prev });
    commands.resizeWindow(page).catch((e) => {
      commands.logFrontendError("warn", "ui-store", `resize failed for ${page}: ${e}`);
    });
  },
  goBack: () => {
    const prev = get().previousPage;
    set({ currentPage: prev });
    commands.resizeWindow(prev).catch((e) => {
      commands.logFrontendError("warn", "ui-store", `resize failed for ${prev}: ${e}`);
    });
  },
  setTheme: (theme) => set({ theme }),
  setLanguage: (language) => set({ language }),
  setSidebarOpen: (sidebarOpen) => set({ sidebarOpen }),
  toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
  setGamePid: (pid) => set({ gamePid: pid }),
  setGameRunning: (running) => {
    set({ gameRunning: running });
    if (!running) set({ gamePid: null });
  },
}));
