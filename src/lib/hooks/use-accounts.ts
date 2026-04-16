import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import type { GameAccountDto, GameCredentialsDto } from "../types";

/** Fetch game accounts for the active session. Enabled only when authenticated. */
export function useGameAccounts() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const activeSessionId = useAuthStore((s) => s.activeSessionId);

  return useQuery<GameAccountDto[]>({
    queryKey: ["gameAccounts", activeSessionId],
    queryFn: async () => {
      if (!activeSessionId) return [];
      const accounts = await commands.getGameAccounts(activeSessionId);
      useAuthStore.getState().updateGameAccounts(activeSessionId, accounts);
      return accounts;
    },
    enabled: isAuthenticated && !!activeSessionId,
  });
}

/** Retrieve one-time credentials for a game account. */
export function useGameCredentials() {
  return useMutation<GameCredentialsDto, Error, string>({
    mutationFn: (accountId: string) => {
      const sessionId = useAuthStore.getState().activeSessionId ?? "";
      return commands.getGameCredentials(sessionId, accountId);
    },
  });
}

/** Returns a function to re-fetch game accounts from the server and update the cache. */
export function useRefreshAccounts() {
  const queryClient = useQueryClient();

  return async () => {
    const sessionId = useAuthStore.getState().activeSessionId;
    if (!sessionId) return;
    try {
      const accounts = await commands.refreshAccounts(sessionId);
      useAuthStore.getState().updateGameAccounts(sessionId, accounts);
      queryClient.setQueryData(["gameAccounts", sessionId], accounts);
    } catch {
      queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
    }
  };
}
