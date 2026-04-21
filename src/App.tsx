import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "./lib/i18n";
import { commands } from "./lib/tauri";
import { useUiStore } from "./lib/stores/ui-store";
import { useUpdateStore } from "./lib/stores/update-store";
import { useConfig } from "./lib/hooks/use-config";
import { Titlebar } from "./features/shared/Titlebar";
import { ErrorToastContainer } from "./features/shared/ErrorToast";
import { UpdateDialog } from "./features/shared/UpdateDialog";
import { LoginPage } from "./features/login/LoginPage";
import { MainPage } from "./features/launcher/MainPage";
import { ToolboxPage } from "./features/toolbox/ToolboxPage";
import type { UpdateInfoDto } from "./lib/types";

function PageRouter() {
  const currentPage = useUiStore((s) => s.currentPage);

  switch (currentPage) {
    case "login":
      return <LoginPage />;
    case "main":
      return <MainPage />;
    case "toolbox":
      return <ToolboxPage />;
  }
}

function useThemeEffect() {
  const theme = useUiStore((s) => s.theme);

  useEffect(() => {
    const root = document.documentElement;

    function applyTheme(mode: "dark" | "light") {
      if (mode === "light") {
        root.classList.add("light");
      } else {
        root.classList.remove("light");
      }
    }

    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: light)");
      applyTheme(mq.matches ? "light" : "dark");
      const handler = (e: MediaQueryListEvent) => applyTheme(e.matches ? "light" : "dark");
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }

    applyTheme(theme);
  }, [theme]);
}

function useInitialConfigSync() {
  const { data: config, isLoading } = useConfig();
  const setTheme = useUiStore((s) => s.setTheme);
  const setLanguage = useUiStore((s) => s.setLanguage);

  useEffect(() => {
    if (!config) return;
    setTheme(config.theme);
    setLanguage(config.language);
  }, [config, setTheme, setLanguage]);

  return isLoading;
}

function SplashScreen() {
  return (
    <div className="flex h-screen flex-col items-center justify-center bg-[var(--bg)]">
      <img src="/app-icon.png" alt="" className="mb-4 h-16 w-16 animate-pulse rounded-[16px]" />
      <div className="text-[12px] tracking-[2px] text-text-dim">MAPLELINK</div>
    </div>
  );
}

export function App() {
  useThemeEffect();
  const configLoading = useInitialConfigSync();
  const { t } = useTranslation();
  const ready = !configLoading;
  const [pendingUpdate, setPendingUpdate] = useState<UpdateInfoDto | null>(null);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const availableUpdate = useUpdateStore((s) => s.availableUpdate);
  // Show banner on all pages when update available and dialog dismissed
  const showBanner = !pendingUpdate && availableUpdate && !bannerDismissed;

  // Adjust window height when update banner appears or disappears
  const bannerHeight = 28;
  useEffect(() => {
    // Small delay to let the DOM render before measuring
    const timer = setTimeout(async () => {
      try {
        const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        const size = await win.innerSize();
        const scaleFactor = await win.scaleFactor();
        const logicalW = size.width / scaleFactor;
        const logicalH = size.height / scaleFactor;
        const newH = showBanner ? logicalH + bannerHeight : logicalH - bannerHeight;
        await win.setSize(new LogicalSize(logicalW, newH));
      } catch {
        /* non-critical */
      }
    }, 50);
    return () => clearTimeout(timer);
  }, [showBanner]);

  // Global listener for download progress events (works even when UpdateDialog is closed)
  useEffect(() => {
    const unlisten = listen<{ downloaded: number; total: number; speed: number }>(
      "update-download-progress",
      (event) => {
        const store = useUpdateStore.getState();
        if (store.status === "downloading") {
          store.updateProgress(event.payload.downloaded, event.payload.total, event.payload.speed);
        }
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Listen for backend update-available event + fallback frontend check.
  // listen() is async so the backend event may fire before the listener
  // is registered. The delayed checkUpdate() catches that race.
  useEffect(() => {
    function onUpdate(info: UpdateInfoDto) {
      setPendingUpdate(info);
      useUpdateStore.getState().setAvailableUpdate(info);
    }

    const unlisten = listen<UpdateInfoDto>("update-available", (event) => {
      onUpdate(event.payload);
    });

    // Fallback: if backend event was missed, check after a short delay
    const timer = setTimeout(() => {
      if (useUpdateStore.getState().availableUpdate) return; // already got it
      commands
        .checkUpdate()
        .then((info) => {
          if (info) onUpdate(info);
        })
        .catch(() => {});
    }, 3000);

    return () => {
      clearTimeout(timer);
      unlisten.then((f) => f());
    };
  }, []);

  if (!ready) return <SplashScreen />;

  return (
    <div className="flex h-screen flex-col bg-[var(--bg)] text-[var(--text)]">
      <Titlebar />
      {showBanner && (
        <div className="flex shrink-0 items-center justify-between bg-[rgba(232,162,58,0.12)] px-3 py-1.5 backdrop-blur-sm">
          <button
            onClick={() => setPendingUpdate(availableUpdate)}
            className="flex-1 text-left text-[11px] text-accent transition-opacity hover:opacity-80"
          >
            🔔 {t("app.update_banner").replace("{{version}}", availableUpdate.version)}
          </button>
          <button
            onClick={() => setBannerDismissed(true)}
            className="ml-2 shrink-0 rounded p-0.5 text-text-faint transition-colors hover:text-[var(--text)]"
            aria-label="Dismiss"
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 12 12"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            >
              <path d="M3 3L9 9M9 3L3 9" />
            </svg>
          </button>
        </div>
      )}
      <main className="relative flex-1 overflow-hidden">
        <PageRouter />
      </main>
      <ErrorToastContainer />
      {pendingUpdate && (
        <UpdateDialog update={pendingUpdate} onClose={() => setPendingUpdate(null)} />
      )}
    </div>
  );
}
