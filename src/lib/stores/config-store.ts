import { create } from "zustand";
import type { AppConfigDto } from "../types";

export interface ConfigState {
  config: AppConfigDto | null;
  isLoaded: boolean;
  setConfig: (config: AppConfigDto) => void;
  updateConfigField: <K extends keyof AppConfigDto>(key: K, value: AppConfigDto[K]) => void;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: null,
  isLoaded: false,
  setConfig: (config) => set({ config, isLoaded: true }),
  updateConfigField: (key, value) =>
    set((state) => {
      if (!state.config) return state;
      return { config: { ...state.config, [key]: value } };
    }),
}));
