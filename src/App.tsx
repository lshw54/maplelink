import { useEffect } from "react";
import { useUiStore } from "./lib/stores/ui-store";
import { useConfig } from "./lib/hooks/use-config";
import { Titlebar } from "./features/shared/Titlebar";
import { ErrorToastContainer } from "./features/shared/ErrorToast";
import { LoginPage } from "./features/login/LoginPage";
import { MainPage } from "./features/launcher/MainPage";
import { ToolboxPage } from "./features/toolbox/ToolboxPage";

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

  return (
    <div className="flex h-screen flex-col bg-[var(--bg)] text-[var(--text)]">
      <Titlebar />
      <main className="relative flex-1 overflow-hidden">
        <PageRouter />
      </main>
      <ErrorToastContainer />
    </div>
  );
}
