import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import type { AccountLimitDto, GameAccountDto, GameCredentialsDto } from "../types";

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

/** The account-limit notice that came with the active session's account
 *  list. Backed by session state, so it costs no request of its own — it is
 *  re-read whenever the list itself is reloaded. */
export function useAccountLimit() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const activeSessionId = useAuthStore((s) => s.activeSessionId);

  return useQuery<AccountLimitDto>({
    queryKey: ["accountLimit", activeSessionId],
    queryFn: () => commands.getAccountLimit(activeSessionId ?? ""),
    enabled: isAuthenticated && !!activeSessionId,
  });
}

/** Retrieve one-time credentials for a game account. */
export function useGameCredentials() {
  return useMutation<GameCredentialsDto, Error, string>({
    mutationFn: (accountId: string) => {
      // Use the session that OWNS this account, not the globally-active one —
      // otherwise switching account tabs fetches an account against the wrong
      // session ("account not found" → the wrong account gets logged out).
      const sessionId = useAuthStore.getState().sessionIdForAccount(accountId) ?? "";
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
    queryClient.invalidateQueries({ queryKey: ["accountLimit"] });
  };
}
