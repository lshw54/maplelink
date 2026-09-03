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

export type ConfigKey = keyof AppConfigDto;
export type ConfigValue = string | boolean | number;

/**
 * Keys the backend only accepts in snake_case. Every key added since accepts
 * both spellings, so the camelCase name goes through as-is.
 */
const KEY_MAP: Partial<Record<ConfigKey, string>> = {
  gamePath: "game_path",
  autoUpdate: "auto_update",
  updateChannel: "update_channel",
  fontSize: "font_size",
  skipPlayConfirm: "skip_play_confirm",
  autoStart: "auto_start",
  debugLogging: "debug_logging",
  autoLaunchGame: "auto_launch_game",
  autoKillPatcher: "auto_kill_patcher",
  traditionalLogin: "traditional_login",
  gamepassIncognito: "gamepass_incognito",
};

/** The backend takes every value as a string; the store keeps the field's real type. */
function coerce(current: unknown, value: ConfigValue): unknown {
  if (typeof current === "boolean") return typeof value === "boolean" ? value : value === "true";
  if (typeof current === "number") return typeof value === "number" ? value : Number(value);
  return String(value);
}

/**
 * Apply a config field to the store at once, then persist it; the store is put
 * back if the write fails. Usable outside React — `useSetConfig` wraps it.
 */
export async function writeConfig(key: ConfigKey, value: ConfigValue): Promise<undefined> {
  const store = useConfigStore.getState();
  const prev = store.config;
  if (prev) store.updateConfigField(key, coerce(prev[key], value) as never);
  try {
    await commands.setConfig(KEY_MAP[key] ?? key, String(value));
  } catch (e) {
    if (prev) store.setConfig(prev);
    throw e;
  }
}

/** Set a config field with an optimistic store update; see [`writeConfig`]. */
export function useSetConfig() {
  const queryClient = useQueryClient();

  return useMutation<undefined, Error, { key: ConfigKey; value: ConfigValue }>({
    mutationFn: ({ key, value }) => writeConfig(key, value),
    onSettled: () => {
      queryClient.setQueryData(["config"], useConfigStore.getState().config);
    },
  });
}
