import { useState } from "react";
import { useTranslation } from "../i18n";
import { useConfigStore } from "../stores/config-store";
import { useSetConfig } from "./use-config";
import { useGameCredentials } from "./use-accounts";
import { useAuthStore } from "../stores/auth-store";
import { useUiStore } from "../stores/ui-store";
import { useErrorToastStore } from "../stores/error-toast-store";
import { commands } from "../tauri";
import type { ErrorDto, GameCredentialsDto } from "../types";

/**
 * One-time-password state and actions shared by every OTP surface (the full
 * OtpPanel card and the compact launcher's single row): fetch (with optional
 * auto-input into the game window), copy, and the persisted auto-input flag.
 */
export function useOtp(
  selectedAccountId: string | null,
  onOtpFetched?: (accountId: string, otp: string) => void,
) {
  const { t } = useTranslation();
  const credentialsMutation = useGameCredentials();
  // An OTP belongs to the account it was fetched for: each account keeps its
  // own readout, so switching tabs shows that tab's code, not the last one.
  const credentials = useUiStore((s) =>
    selectedAccountId ? (s.otpByAccount[selectedAccountId] ?? null) : null,
  );
  const setOtp = useUiStore((s) => s.setOtp);
  const [copied, setCopied] = useState(false);
  const [pasting, setPasting] = useState(false);
  // Persisted: as component state this reset to on every time the panel
  // remounted, which is why it appeared to tick itself back on.
  const autoInput = useConfigStore((s) => s.config?.otpAutoInput ?? true);
  const setConfig = useSetConfig();
  const addToast = useErrorToastStore((s) => s.addToast);

  const setAutoInput = (on: boolean) => {
    setConfig.mutate({ key: "otpAutoInput", value: on });
  };

  function handleOtpError(error: Error) {
    const msg = error.message || t("launcher.otp_error");
    // Tauri rejects with the raw ErrorDto object, so the machine-readable
    // code is available even though the mutation types the error as Error.
    const code = (error as Partial<ErrorDto>).code ?? "";
    const isSessionGone =
      code === "AUTH_NOT_AUTHENTICATED" ||
      code === "AUTH_SESSION_EXPIRED" ||
      code === "AUTH_INVALID_CREDENTIALS";

    if (isSessionGone) {
      // Session is dead — remove it and redirect to login
      const sessionId = useAuthStore.getState().activeSessionId;
      if (sessionId) {
        commands.logout(sessionId).catch(() => {});
        useAuthStore.getState().removeSession(sessionId);
      }
      addToast({
        message: t("errors.AUTH_SESSION_EXPIRED"),
        category: "authentication",
        critical: true,
      });
    } else {
      addToast({ message: msg, category: "authentication", critical: false });
    }
  }

  /** Show the OTP and put it on the clipboard, every time one is fetched — the
   *  old launcher did the same. The copy goes through the backend because
   *  auto-input has just handed focus to the game, and the webview's own
   *  clipboard refuses to write when the document isn't focused. */
  async function applyOtp(accountId: string, data: GameCredentialsDto) {
    setOtp(accountId, data);
    onOtpFetched?.(accountId, data.otp);
    const ok = await commands.copyToClipboard(data.otp).catch(() => false);
    setCopied(ok);
    if (ok) setTimeout(() => setCopied(false), 2000);
  }

  async function getOtp() {
    if (!selectedAccountId) return;
    const fetchAndShow = () =>
      credentialsMutation.mutate(selectedAccountId, {
        onSuccess: (data) => {
          void applyOtp(selectedAccountId, data);
        },
        onError: handleOtpError,
      });

    if (!autoInput) {
      // Manual mode: just get OTP and display
      fetchAndShow();
      return;
    }
    // Auto-paste mode: get OTP + auto-input to game window, then always fetch
    // credentials to display the OTP regardless of the paste result.
    setPasting(true);
    try {
      await commands.autoPasteOtp(
        useAuthStore.getState().sessionIdForAccount(selectedAccountId) ?? "",
        selectedAccountId,
      );
    } catch {
      /* fall through to the regular fetch */
    } finally {
      setPasting(false);
    }
    fetchAndShow();
  }

  /** Fetch a fresh OTP and put "account⏎otp" on the clipboard — the same
   *  thing the account context menu's "copy one-time credentials" does. The
   *  OTP is shown too, so the readout never disagrees with the clipboard. */
  async function copyCredentials() {
    if (!selectedAccountId) return;
    const loadingId = addToast({
      message: t("launcher.context.credentials_loading"),
      category: "loading",
      critical: false,
    });
    try {
      const data = await commands.getGameCredentials(
        useAuthStore.getState().sessionIdForAccount(selectedAccountId) ?? "",
        selectedAccountId,
      );
      setOtp(selectedAccountId, data);
      onOtpFetched?.(selectedAccountId, data.otp);
      await commands.copyToClipboard(`${data.accountId}
${data.otp}`);
      useErrorToastStore.getState().removeToast(loadingId);
      addToast({
        message: t("launcher.context.credentials_copied"),
        category: "success",
        critical: false,
      });
    } catch (err) {
      useErrorToastStore.getState().removeToast(loadingId);
      handleOtpError(err as Error);
    }
  }

  async function copyOtp() {
    if (!credentials) return;
    const ok = await commands.copyToClipboard(credentials.otp).catch(() => false);
    setCopied(ok);
    if (ok) setTimeout(() => setCopied(false), 1500);
  }

  return {
    credentials,
    copied,
    autoInput,
    setAutoInput,
    /** Fetch is in flight (network or auto-paste). */
    busy: credentialsMutation.isPending || pasting,
    getOtp,
    copyOtp,
    copyCredentials,
  };
}
