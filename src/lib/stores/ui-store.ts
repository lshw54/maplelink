import { create } from "zustand";
import { commands } from "../tauri";

export type Page = "login" | "main" | "toolbox";
export type ThemeMode = "system" | "dark" | "light";
export type Language = "en-US" | "zh-TW" | "zh-CN";

export interface UiState {
  currentPage: Page;
  previousPage: Page;
  theme: ThemeMode;
  language: Language;
  sidebarOpen: boolean;
  setPage: (page: Page) => void;
  goBack: () => void;
  setTheme: (theme: ThemeMode) => void;
  setLanguage: (language: Language) => void;
  setSidebarOpen: (open: boolean) => void;
  toggleSidebar: () => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  currentPage: "login",
  previousPage: "login",
  theme: "dark",
  language: "zh-TW",
  sidebarOpen: false,
  setPage: (page) => {
    const current = get().currentPage;
    const prev = current !== "toolbox" ? current : get().previousPage;
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
}));
