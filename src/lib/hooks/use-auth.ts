import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import { useUiStore } from "../stores/ui-store";
import type { SessionDto, QrCodeData, QrPollResult } from "../types";

/** Login with account + password. Updates auth store on success. */
export function useLogin() {
  const queryClient = useQueryClient();
  const { setSession, setGameAccounts, setPendingCredentials } = useAuthStore.getState();

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
      // Store credentials before login attempt — if TOTP is required,
      // they'll be saved after verification succeeds.
      setPendingCredentials({ account, password, rememberPassword });

      try {
        const session = await commands.login(account, password);
        // Save credentials after successful login (account always saved,
        // password only if rememberPassword is true).
        try {
          await commands.saveLoginCredentials(account, password, rememberPassword);
        } catch {
          /* credential save failure is non-critical */
        }
        // Clear pending since we saved successfully
        setPendingCredentials(null);
        return session;
      } catch (err: unknown) {
        // Tauri returns ErrorDto as a plain object or string.
        // Check if it indicates TOTP required.
        const errStr = typeof err === "object" && err !== null ? JSON.stringify(err) : String(err);
        if (errStr.includes("TOTP") || errStr.includes("totp") || errStr.includes("Totp")) {
          const totpError = new Error("TOTP_REQUIRED");
          totpError.name = "TotpRequired";
          throw totpError;
        }
        if (errStr.includes("ADVANCE_CHECK") || errStr.includes("advance_check")) {
          const errObj =
            typeof err === "object" && err !== null ? (err as Record<string, unknown>) : null;
          const url = errObj?.message ? String(errObj.message) : undefined;
          const advError = new Error("ADVANCE_CHECK") as Error & { advanceUrl?: string };
          advError.name = "AdvanceCheck";
          advError.advanceUrl = url || undefined;
          throw advError;
        }
        setPendingCredentials(null);
        throw new Error(
          typeof err === "object" && err !== null && "message" in err
            ? String((err as Record<string, unknown>).message)
            : String(err),
        );
      }
    },
    onSuccess: async (session: SessionDto) => {
      setSession(session);
      try {
        const accounts = await commands.getGameAccounts();
        setGameAccounts(accounts);
      } catch {
        /* accounts fetch failure is non-critical */
      }
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
    },
  });
}

/** Start QR code login flow (TW region). */
export function useQrLoginStart() {
  return useMutation<QrCodeData, Error>({
    mutationFn: () => commands.qrLoginStart(),
  });
}

/** Poll QR login status. Updates auth store when confirmed. */
export function useQrLoginPoll() {
  const { setSession } = useAuthStore.getState();
  const queryClient = useQueryClient();

  return useMutation<QrPollResult, Error, { sessionKey: string; verificationToken: string }>({
    mutationFn: ({ sessionKey, verificationToken }) =>
      commands.qrLoginPoll(sessionKey, verificationToken),
    onSuccess: async (result: QrPollResult) => {
      if (result.status === "confirmed" && result.session) {
        setSession(result.session);
        const accounts = await commands.getGameAccounts();
        useAuthStore.getState().setGameAccounts(accounts);
        await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      }
    },
  });
}

/** Verify TOTP code (HK region). Updates auth store on success. */
export function useTotpVerify() {
  const { setSession } = useAuthStore.getState();
  const queryClient = useQueryClient();

  return useMutation<SessionDto, Error, string>({
    mutationFn: (code: string) => commands.totpVerify(code),
    onSuccess: async (session: SessionDto) => {
      setSession(session);

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

      const accounts = await commands.getGameAccounts();
      useAuthStore.getState().setGameAccounts(accounts);
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
    },
  });
}

/** Logout. Clears auth store and navigates to login page. */
export function useLogout() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => commands.logout(),
    onSuccess: async () => {
      useAuthStore.getState().clearSession();
      queryClient.removeQueries({ queryKey: ["gameAccounts"] });
      useUiStore.getState().setPage("login");
    },
  });
}
