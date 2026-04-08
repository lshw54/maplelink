import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import type { GameAccountDto, GameCredentialsDto } from "../types";

/** Fetch game accounts. Enabled only when authenticated. */
export function useGameAccounts() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);

  return useQuery<GameAccountDto[]>({
    queryKey: ["gameAccounts"],
    queryFn: async () => {
      const accounts = await commands.getGameAccounts();
      useAuthStore.getState().setGameAccounts(accounts);
      return accounts;
    },
    enabled: isAuthenticated,
  });
}

/** Retrieve one-time credentials for a game account. */
export function useGameCredentials() {
  return useMutation<GameCredentialsDto, Error, string>({
    mutationFn: (accountId: string) => commands.getGameCredentials(accountId),
  });
}

/** Returns a function to re-fetch game accounts from the server and update the cache. */
export function useRefreshAccounts() {
  const queryClient = useQueryClient();

  return async () => {
    try {
      const accounts = await commands.refreshAccounts();
      useAuthStore.getState().setGameAccounts(accounts);
      queryClient.setQueryData(["gameAccounts"], accounts);
    } catch {
      // If refresh fails, at least try to invalidate the cached query
      queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
    }
  };
}
