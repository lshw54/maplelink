import { useState, type ReactNode } from "react";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { useGameCredentials } from "../../lib/hooks/use-accounts";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useErrorToastStore } from "../../lib/stores/error-toast-store";
import { commands } from "../../lib/tauri";
import type { ErrorDto, GameCredentialsDto } from "../../lib/types";

interface OtpPanelProps {
  selectedAccountId: string | null;
  onOtpFetched?: (accountId: string, otp: string) => void;
  /** Compact launcher: tighter paddings and a smaller OTP readout. */
  compact?: boolean;
  /** Rendered at the end of the OTP row (the compact layout puts Play there). */
  actions?: ReactNode;
}

export function OtpPanel({ selectedAccountId, onOtpFetched, compact, actions }: OtpPanelProps) {
  const credentialsMutation = useGameCredentials();
  const [credentials, setCredentials] = useState<GameCredentialsDto | null>(null);
  const [copied, setCopied] = useState(false);
  // Persisted: as component state this reset to on every time the panel
  // remounted, which is why it appeared to tick itself back on.
  const autoInput = useConfigStore((s) => s.config?.otpAutoInput ?? true);
  const setConfig = useSetConfig();
  const setAutoInput = (on: boolean) => {
    useConfigStore.getState().updateConfigField("otpAutoInput", on);
    setConfig.mutate({ key: "otp_auto_input", value: String(on) });
  };
  const [pasting, setPasting] = useState(false);
  const { t } = useTranslation();
  const addToast = useErrorToastStore((s) => s.addToast);

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
    setCredentials(data);
    onOtpFetched?.(accountId, data.otp);
    const copied = await commands.copyToClipboard(data.otp).catch(() => false);
    setCopied(copied);
    if (copied) setTimeout(() => setCopied(false), 2000);
  }

  async function handleGetOtp() {
    if (!selectedAccountId) return;

    if (autoInput) {
      // Auto-paste mode: get OTP + auto-input to game window
      setPasting(true);
      try {
        await commands.autoPasteOtp(
          useAuthStore.getState().sessionIdForAccount(selectedAccountId) ?? "",
          selectedAccountId,
        );
        // Always fetch credentials to display OTP regardless of paste result
        credentialsMutation.mutate(selectedAccountId, {
          onSuccess: (data) => {
            void applyOtp(selectedAccountId, data);
          },
          onError: handleOtpError,
        });
      } catch {
        // Error — fall back to regular OTP
        credentialsMutation.mutate(selectedAccountId, {
          onSuccess: (data) => {
            void applyOtp(selectedAccountId, data);
          },
          onError: handleOtpError,
        });
      } finally {
        setPasting(false);
      }
    } else {
      // Manual mode: just get OTP and display
      credentialsMutation.mutate(selectedAccountId, {
        onSuccess: (data) => {
          void applyOtp(selectedAccountId, data);
        },
        onError: handleOtpError,
      });
    }
  }

  async function handleCopyOtp() {
    if (!credentials) return;
    const copied = await commands.copyToClipboard(credentials.otp).catch(() => false);
    setCopied(copied);
    if (copied) setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div
      className={`shrink-0 rounded-xl border border-border bg-[var(--surface)] shadow-[0_-4px_20px_rgba(0,0,0,0.1),0_0_0_1px_var(--border)] backdrop-blur-sm ${
        compact ? "mx-2 mb-2 p-2.5" : "mx-3 mb-3 p-3.5"
      }`}
    >
      {/* Header */}
      <div className={`flex items-center justify-between ${compact ? "mb-1.5" : "mb-2.5"}`}>
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-bold tracking-[2.5px] text-text-dim uppercase">
            🔐 {t("launcher.otp")}
          </span>
        </div>
        <label className="flex cursor-pointer items-center gap-1.5">
          <span className="text-[12px] tracking-[0.5px] text-text-faint">
            {t("launcher.auto_input")}
          </span>
          <button
            type="button"
            onClick={() => setAutoInput(!autoInput)}
            className={`relative h-[18px] w-8 shrink-0 rounded-[9px] transition-colors ${
              autoInput ? "bg-[rgba(232,162,58,0.3)]" : "bg-[var(--surface-hover)]"
            }`}
          >
            <span
              className={`absolute top-0.5 h-3.5 w-3.5 rounded-full transition-all ${
                autoInput ? "left-4 bg-accent" : "left-0.5 bg-text-dim"
              }`}
            />
          </button>
        </label>
      </div>

      {/* OTP display row */}
      <div className={`flex items-center ${compact ? "gap-2" : "gap-2.5"}`}>
        <button
          type="button"
          onClick={handleCopyOtp}
          disabled={!credentials}
          className={`relative flex flex-1 items-center justify-center rounded-[10px] border font-mono font-bold transition-all ${
            compact
              ? "min-w-0 px-2 py-2 text-[17px] tracking-[2px]"
              : "px-4 py-2.5 text-[22px] tracking-[4px]"
          } ${
            copied
              ? "border-[rgba(74,222,128,0.4)] bg-[rgba(74,222,128,0.04)] text-green-400"
              : credentials
                ? "border-[rgba(232,162,58,0.08)] bg-[rgba(232,162,58,0.04)] text-accent shadow-[0_0_20px_rgba(232,162,58,0.06)_inset,0_2px_8px_rgba(0,0,0,0.3)_inset] hover:border-[rgba(232,162,58,0.2)] hover:bg-[rgba(232,162,58,0.06)]"
                : "cursor-default border-[rgba(232,162,58,0.08)] bg-[rgba(232,162,58,0.04)] text-text-faint"
          }`}
        >
          {credentials?.otp ?? "••••••••••"}
          {/* Copy / Check icon — always visible */}
          <span
            className={`absolute top-1/2 right-2.5 -translate-y-1/2 transition-colors ${
              copied ? "text-green-400" : "text-text-faint"
            }`}
          >
            {copied ? (
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <rect x="9" y="9" width="13" height="13" rx="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
          </span>
        </button>

        <button
          onClick={handleGetOtp}
          disabled={!selectedAccountId || credentialsMutation.isPending || pasting}
          title={t("launcher.get_otp")}
          className={`flex shrink-0 items-center justify-center rounded-[10px] bg-gradient-to-br from-accent to-[#c47a1a] text-base text-white shadow-[0_2px_10px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_16px_var(--accent-glow)] active:scale-[0.92] disabled:transform-none disabled:cursor-not-allowed disabled:opacity-40 ${
            compact ? "h-9 w-9" : "h-10 w-10"
          }`}
        >
          ↻
        </button>
        {actions}
      </div>
    </div>
  );
}
