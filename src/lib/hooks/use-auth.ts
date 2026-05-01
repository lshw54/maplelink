import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import { useConfigStore } from "../stores/config-store";
import { useUiStore } from "../stores/ui-store";
import type { SessionDto, QrPollResult } from "../types";

/** Login with account + password. Creates a new session, then authenticates. */
export function useLogin() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      account,
      password,
      rememberPassword,
    }: {
      account: string;
      password: string;
      rememberPassword: boolean;
    }) => {
      // Create a new session first
      const sessionId = await commands.createSession();

      useAuthStore.getState().setPendingCredentials({
        account,
        password,
        rememberPassword,
        sessionId,
      });

      try {
        const session = await commands.login(sessionId, account, password);
        try {
          await commands.saveLoginCredentials(account, password, rememberPassword);
        } catch {
          /* credential save failure is non-critical */
        }
        useAuthStore.getState().setPendingCredentials(null);
        return session;
      } catch (err: unknown) {
        const errStr = typeof err === "object" && err !== null ? JSON.stringify(err) : String(err);
        if (errStr.includes("TOTP") || errStr.includes("totp") || errStr.includes("Totp")) {
          const totpError = new Error("TOTP_REQUIRED") as Error & { sessionId: string };
          totpError.name = "TotpRequired";
          totpError.sessionId = sessionId;
          throw totpError;
        }
        if (errStr.includes("ADVANCE_CHECK") || errStr.includes("advance_check")) {
          const errObj =
            typeof err === "object" && err !== null ? (err as Record<string, unknown>) : null;
          const url = errObj?.message ? String(errObj.message) : undefined;
          const advError = new Error("ADVANCE_CHECK") as Error & {
            advanceUrl?: string;
            sessionId: string;
          };
          advError.name = "AdvanceCheck";
          advError.advanceUrl = url || undefined;
          advError.sessionId = sessionId;
          throw advError;
        }
        // Login failed — clean up the session
        useAuthStore.getState().setPendingCredentials(null);
        throw new Error(
          typeof err === "object" && err !== null && "message" in err
            ? String((err as Record<string, unknown>).message)
            : String(err),
          { cause: err },
        );
      }
    },
    onSuccess: async (session: SessionDto) => {
      useAuthStore.getState().addSession(session);
      try {
        const accounts = await commands.getGameAccounts(session.sessionId);
        useAuthStore.getState().updateGameAccounts(session.sessionId, accounts);
      } catch {
        /* accounts fetch failure is non-critical */
      }
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      // Clear addingSession flag, reset login view, and navigate to main
      useUiStore.setState({ addingSession: false, loginView: "normal" });
      useUiStore.getState().setPage("main");

      // Auto-launch game if enabled
      const cfg = useConfigStore.getState().config;
      if (cfg?.autoLaunchGame) {
        setTimeout(async () => {
          try {
            let pid = 0;
            if (cfg.traditionalLogin) {
              pid = await commands.launchGameDirect();
            } else {
              const entry = useAuthStore.getState().sessions.get(session.sessionId);
              const first = entry?.gameAccounts?.[0];
              if (first) {
                pid = await commands.launchGame(session.sessionId, first.id);
              }
            }
            if (pid > 0) {
              useUiStore.getState().setGamePid(pid);
              useUiStore.getState().setGameRunning(true);
            }
          } catch {
            /* auto-launch failure is non-critical */
          }
        }, 500);
      }
    },
  });
}

/** Poll QR login status. Updates auth store when confirmed. */
export function useQrLoginPoll() {
  const queryClient = useQueryClient();

  return useMutation<
    QrPollResult,
    Error,
    { sessionId: string; sessionKey: string; verificationToken: string }
  >({
    mutationFn: ({ sessionId, sessionKey, verificationToken }) =>
      commands.qrLoginPoll(sessionId, sessionKey, verificationToken),
    onSuccess: async (result: QrPollResult) => {
      if (result.status === "confirmed" && result.session) {
        useAuthStore.getState().addSession(result.session);
        const accounts = await commands.getGameAccounts(result.session.sessionId);
        useAuthStore.getState().updateGameAccounts(result.session.sessionId, accounts);
        await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      }
    },
  });
}

/** Verify TOTP code (HK region). Updates auth store on success. */
export function useTotpVerify() {
  const queryClient = useQueryClient();

  return useMutation<SessionDto, Error, { sessionId: string; code: string }>({
    mutationFn: ({ sessionId, code }) => commands.totpVerify(sessionId, code),
    onSuccess: async (session: SessionDto) => {
      useAuthStore.getState().addSession(session);

      // Save pending credentials from the login attempt
      const pending = useAuthStore.getState().pendingCredentials;
      if (pending) {
        try {
          await commands.saveLoginCredentials(
            pending.account,
            pending.password,
            pending.rememberPassword,
          );
        } catch {
          /* credential save failure is non-critical */
        }
        useAuthStore.getState().setPendingCredentials(null);
      }

      const accounts = await commands.getGameAccounts(session.sessionId);
      useAuthStore.getState().updateGameAccounts(session.sessionId, accounts);
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      // Reset login view, clear addingSession, navigate to main
      useUiStore.setState({ addingSession: false, loginView: "normal" });
      useUiStore.getState().setPage("main");
    },
  });
}

/** Logout the active session. Clears it from auth store. */
export function useLogout() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const sessionId = useAuthStore.getState().activeSessionId;
      if (sessionId) {
        await commands.logout(sessionId);
      }
    },
    onSuccess: async () => {
      const sessionId = useAuthStore.getState().activeSessionId;
      if (sessionId) {
        useAuthStore.getState().removeSession(sessionId);
      }
      queryClient.removeQueries({ queryKey: ["gameAccounts"] });
      // If no sessions left, go to login
      if (useAuthStore.getState().sessions.size === 0) {
        useUiStore.getState().setPage("login");
      }
    },
  });
}
