import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands } from "./lib/tauri";
import { useUiStore } from "./lib/stores/ui-store";
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
  const { data: config } = useConfig();
  const setTheme = useUiStore((s) => s.setTheme);
  const setLanguage = useUiStore((s) => s.setLanguage);

  useEffect(() => {
    if (!config) return;
    setTheme(config.theme);
    setLanguage(config.language);
  }, [config, setTheme, setLanguage]);
}

export function App() {
  useThemeEffect();
  useInitialConfigSync();
  const [pendingUpdate, setPendingUpdate] = useState<UpdateInfoDto | null>(null);

  // Listen for update-available event from backend startup check
  // Also do a frontend check after mount (in case backend event was missed)
  useEffect(() => {
    const unlisten = listen<UpdateInfoDto>("update-available", (event) => {
      setPendingUpdate(event.payload);
    });

    // Frontend check as backup (backend event may fire before listener is ready)
    commands.checkUpdate().then((info) => {
      if (info) setPendingUpdate(info);
    }).catch(() => {});

    return () => { unlisten.then((f) => f()); };
  }, []);

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
