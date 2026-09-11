import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useUiStore } from "../stores/ui-store";
import { useConfig } from "./use-config";
import { applyAccent } from "../accent";

/**
 * Theme, language and accent, applied from the saved config.
 *
 * Every window mounts these so the app looks the same everywhere; keeping them
 * out of `App.tsx` lets the client-manager window share the behaviour rather
 * than carry a second copy that drifts.
 */
export function useThemeEffect() {
  const theme = useUiStore((state) => state.theme);

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

/** Push the saved config into the UI store. Returns whether it is still loading. */
export function useInitialConfigSync() {
  const { data: config, isLoading } = useConfig();
  const setTheme = useUiStore((s) => s.setTheme);
  const setLanguage = useUiStore((s) => s.setLanguage);
  const queryClient = useQueryClient();

  // Config lives in the backend but is cached per window, so a change made in
  // another window only lands here once the backend says it happened.
  useEffect(() => {
    const off = listen("config-changed", () => {
      void queryClient.invalidateQueries({ queryKey: ["config"] });
    });
    return () => {
      off.then((un) => un());
    };
  }, [queryClient]);

  useEffect(() => {
    if (!config) return;
    setTheme(config.theme);
    setLanguage(config.language);
    applyAccent(config.accentColor ?? "");
  }, [config, setTheme, setLanguage]);

  return isLoading;
}
