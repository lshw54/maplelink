import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useUiStore } from "../../lib/stores/ui-store";
import type { QrCodeData, QrPollResult } from "../../lib/types";

interface QrLoginFormProps {
  onBack: () => void;
}

export function QrLoginForm({ onBack }: QrLoginFormProps) {
  const { t } = useTranslation();
  const [qrData, setQrData] = useState<QrCodeData | null>(null);
  const [status, setStatus] = useState<string>("loading");
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startedRef = useRef(false);

  function stopPolling() {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }

  async function startQr() {
    if (startedRef.current) return;
    startedRef.current = true;
    stopPolling();
    setStatus("loading");
    setError(null);

    try {
      // Create a new session for this QR login flow
      const sessionId = await commands.createSession();
      const data = await commands.qrLoginStart(sessionId);
      setQrData(data);
      setStatus("pending");

      // Start polling
      intervalRef.current = setInterval(async () => {
        try {
          const result: QrPollResult = await commands.qrLoginPoll(
            sessionId,
            data.sessionKey,
            data.verificationToken,
          );
          if (result.status === "confirmed") {
            stopPolling();
            setStatus("confirmed");
            // result.session is the raw Session from backend — add sessionId manually
            const confirmedSession = result.session ? { ...result.session, sessionId } : null;
            if (confirmedSession) {
              useAuthStore.getState().addSession(confirmedSession);
              const accounts = await commands.getGameAccounts(sessionId);
              useAuthStore.getState().updateGameAccounts(sessionId, accounts);
              // Navigate to main
              useUiStore.getState().addingSession = false;
              useUiStore.getState().setPage("main");
            }
          } else if (result.status === "expired") {
            stopPolling();
            setStatus("expired");
          }
        } catch {
          // Poll error — ignore, will retry
        }
      }, 2000);
    } catch (err) {
      setError(
        typeof err === "object" && err !== null && "message" in err
          ? String((err as Record<string, unknown>).message)
          : String(err),
      );
      setStatus("error");
      startedRef.current = false;
    }
  }

  function handleRefresh() {
    startedRef.current = false;
    startQr();
  }

  useEffect(() => {
    startQr();
    return stopPolling;
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="flex w-full flex-col items-center">
      <div className="mb-5 flex flex-col items-center">
        <img
          src="/app-icon.png"
          alt="MapleLink"
          className="mb-2.5 h-10 w-10 rounded-[10px] shadow-[0_4px_20px_var(--accent-glow)]"
        />
        <div className="text-[12px] uppercase tracking-[4px] text-text-dim">
          {t("login.qr.title")}
        </div>
        <div className="mt-1.5 text-[12px] tracking-[0.5px] text-text-faint">
          {t("login.qr.instruction")}
        </div>
      </div>

      <div className="flex w-full flex-col items-center gap-3 rounded-[14px] border border-border bg-[var(--surface)] p-5">
        <div className="flex h-[180px] w-[180px] items-center justify-center rounded-xl bg-white p-4 shadow-[0_2px_12px_rgba(0,0,0,0.08)]">
          {status === "loading" ? (
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-accent border-t-transparent" />
          ) : qrData?.qrImageUrl ? (
            <img
              src={qrData.qrImageUrl}
              alt="QR Code"
              className="block rounded"
              width="150"
              height="150"
            />
          ) : (
            <div className="text-xs text-text-faint">—</div>
          )}
        </div>
        <div className="animate-pulse text-[12px] tracking-[1px] text-text-dim">
          {status === "expired"
            ? t("login.qr.expired")
            : status === "error"
              ? (error ?? "Error")
              : t("login.qr.waiting")}
        </div>
      </div>

      {status === "expired" && (
        <button
          type="button"
          onClick={handleRefresh}
          className="mt-4 w-full rounded-lg bg-accent px-4 py-2.5 text-[12px] font-semibold uppercase tracking-[1.5px] text-white hover:opacity-90"
        >
          {t("login.qr.refresh")}
        </button>
      )}

      <button
        type="button"
        onClick={onBack}
        className="mt-4 w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
      >
        {t("login.back_normal")}
      </button>
    </div>
  );
}
