import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { useGameCredentials } from "../../lib/hooks/use-accounts";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useErrorToastStore } from "../../lib/stores/error-toast-store";
import { commands } from "../../lib/tauri";
import type { GameCredentialsDto } from "../../lib/types";

interface OtpPanelProps {
  selectedAccountId: string | null;
  onOtpFetched?: (accountId: string, otp: string) => void;
}

export function OtpPanel({ selectedAccountId, onOtpFetched }: OtpPanelProps) {
  const credentialsMutation = useGameCredentials();
  const [credentials, setCredentials] = useState<GameCredentialsDto | null>(null);
  const [copied, setCopied] = useState(false);
  const [autoInput, setAutoInput] = useState(true);
  const [pasting, setPasting] = useState(false);
  const { t } = useTranslation();
  const addToast = useErrorToastStore((s) => s.addToast);

  function handleOtpError(error: Error) {
    const msg = error.message || t("launcher.otp_error");
    const isSessionGone =
      msg.includes("Not authenticated") ||
      msg.includes("expired") ||
      msg.includes("閒置過久") ||
      msg.includes("重新登入") ||
      msg.includes("Invalid credentials");

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

  async function handleGetOtp() {
    if (!selectedAccountId) return;

    if (autoInput) {
      // Auto-paste mode: get OTP + auto-input to game window
      setPasting(true);
      try {
        const pasted = await commands.autoPasteOtp(
          useAuthStore.getState().activeSessionId ?? "",
          selectedAccountId,
        );
        // Always fetch credentials to display OTP regardless of paste result
        credentialsMutation.mutate(selectedAccountId, {
          onSuccess: async (data) => {
            setCredentials(data);
            onOtpFetched?.(selectedAccountId, data.otp);
            setCopied(false);
            if (!pasted) {
              // Window not found — copy to clipboard as fallback
              await navigator.clipboard.writeText(data.otp);
              setCopied(true);
              setTimeout(() => setCopied(false), 2000);
            }
          },
          onError: handleOtpError,
        });
      } catch {
        // Error — fall back to regular OTP
        credentialsMutation.mutate(selectedAccountId, {
          onSuccess: (data) => {
            setCredentials(data);
            onOtpFetched?.(selectedAccountId, data.otp);
            setCopied(false);
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
          setCredentials(data);
          onOtpFetched?.(selectedAccountId, data.otp);
          setCopied(false);
        },
        onError: handleOtpError,
      });
    }
  }

  async function handleCopyOtp() {
    if (!credentials) return;
    await navigator.clipboard.writeText(credentials.otp);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="mx-3 mb-3 shrink-0 rounded-xl border border-border bg-[var(--surface)] p-3.5 shadow-[0_-4px_20px_rgba(0,0,0,0.1),0_0_0_1px_var(--border)] backdrop-blur-sm">
      {/* Header */}
      <div className="mb-2.5 flex items-center justify-between">
        <span className="text-[12px] font-semibold tracking-[2px] text-text-dim uppercase">
          🔐 {t("launcher.otp")}
        </span>
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
      <div className="flex items-center gap-2.5">
        <button
          type="button"
          onClick={handleCopyOtp}
          disabled={!credentials}
          className={`relative flex flex-1 items-center justify-center rounded-[10px] border px-4 py-2.5 font-mono text-[22px] font-bold tracking-[4px] transition-all ${
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
          className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[10px] bg-gradient-to-br from-accent to-[#c47a1a] text-base text-white shadow-[0_2px_10px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_16px_var(--accent-glow)] active:scale-[0.92] disabled:transform-none disabled:cursor-not-allowed disabled:opacity-40"
        >
          ↻
        </button>
      </div>
    </div>
  );
}
