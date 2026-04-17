import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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
  const [ready, setReady] = useState(false);
  const [pendingUpdate, setPendingUpdate] = useState<UpdateInfoDto | null>(null);

  // Wait for config to load before showing UI
  useEffect(() => {
    if (!configLoading) setReady(true);
  }, [configLoading]);

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

  // Check for updates after app is ready
  useEffect(() => {
    if (!ready) return;

    const unlisten = listen<UpdateInfoDto>("update-available", (event) => {
      setPendingUpdate(event.payload);
    });

    // Small delay to ensure UI is rendered before showing update dialog
    const timer = setTimeout(() => {
      commands
        .checkUpdate()
        .then((info) => {
          if (info) setPendingUpdate(info);
        })
        .catch((e) => {
          commands.logFrontendError("warn", "App", `update check failed: ${e}`);
        });
    }, 1500);

    return () => {
      clearTimeout(timer);
      unlisten.then((f) => f());
    };
  }, [ready]);

  if (!ready) return <SplashScreen />;

  return (
    <div className="flex h-screen flex-col bg-[var(--bg)] text-[var(--text)]">
      <Titlebar />
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
