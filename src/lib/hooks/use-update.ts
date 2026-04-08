import { useMutation, useQuery } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useConfigStore } from "../stores/config-store";
import type { UpdateInfoDto } from "../types";

/** Check for available updates. Enabled only when autoUpdate is on. */
export function useCheckUpdate() {
  const autoUpdate = useConfigStore((s) => s.config?.autoUpdate ?? false);

  return useQuery<UpdateInfoDto | null>({
    queryKey: ["checkUpdate"],
    queryFn: () => commands.checkUpdate(),
    enabled: autoUpdate,
    staleTime: Infinity,
    refetchOnMount: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

/** Apply a downloaded update. */
export function useApplyUpdate() {
  return useMutation<undefined, Error>({
    mutationFn: async () => {
      await commands.applyUpdate();
    },
  });
}
