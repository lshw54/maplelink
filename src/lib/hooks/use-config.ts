import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useConfigStore } from "../stores/config-store";
import type { AppConfigDto } from "../types";

/** Fetch app config. Updates config store on success. */
export function useConfig() {
  return useQuery<AppConfigDto>({
    queryKey: ["config"],
    queryFn: async () => {
      const config = await commands.getConfig();
      useConfigStore.getState().setConfig(config);
      return config;
    },
  });
}

/** Set a config field. After success, refetches config to update store. */
export function useSetConfig() {
  const queryClient = useQueryClient();

  return useMutation<undefined, Error, { key: string; value: string }>({
    mutationFn: async ({ key, value }) => {
      const backendKey = toSnakeCase(key);
      await commands.setConfig(backendKey, value);
    },
    onSuccess: async () => {
      // Refetch config from backend and update store
      const config = await commands.getConfig();
      useConfigStore.getState().setConfig(config);
      queryClient.setQueryData(["config"], config);
    },
  });
}

/** Map camelCase frontend keys to snake_case backend keys. */
const KEY_MAP: Record<string, string> = {
  gamePath: "game_path",
  autoUpdate: "auto_update",
  updateChannel: "update_channel",
  fontSize: "font_size",
  skipPlayConfirm: "skip_play_confirm",
  autoStart: "auto_start",
  debugLogging: "debug_logging",
  windowX: "window_x",
  windowY: "window_y",
  windowWidth: "window_width",
  windowHeight: "window_height",
  __reset__: "__reset__",
};

function toSnakeCase(key: string): string {
  return KEY_MAP[key] ?? key;
}
