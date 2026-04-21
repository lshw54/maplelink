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
  const [qrData, setQrData] = useState<QrCodeData | null>(
    useUiStore.getState().qrData as QrCodeData | null,
  );
  const [status, setStatus] = useState<string>(
    useUiStore.getState().qrData ? "pending" : "loading",
  );
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [enlarged, setEnlarged] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startedRef = useRef(false);
  const sessionIdRef = useRef<string | null>(useUiStore.getState().qrSessionId);

  function stopPolling() {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }

  function startPolling(sessionId: string, data: QrCodeData) {
    stopPolling();
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
          commands.logFrontendError(
            "info",
            "QrLoginForm",
            `confirmed! session=${JSON.stringify(result.session)?.slice(0, 100)}, sessionId=${sessionId}`,
          );
          const confirmedSession = result.session ? { ...result.session, sessionId } : null;
          if (confirmedSession) {
            useAuthStore.getState().addSession(confirmedSession);
            const accounts = await commands.getGameAccounts(sessionId);
            useAuthStore.getState().updateGameAccounts(sessionId, accounts);
            // Clear persisted QR state
            useUiStore.setState({ qrSessionId: null, qrData: null, loginView: "normal" });
            useUiStore.getState().addingSession = false;
            // Reset window size if enlarged
            commands.resizeWindow("login").catch(() => {});
            useUiStore.getState().setPage("main");
          }
        } else if (result.status === "expired") {
          stopPolling();
          setStatus("expired");
          useUiStore.setState({ qrSessionId: null, qrData: null });
        }
      } catch {
        // Poll error — ignore, will retry
      }
    }, 2000);
  }

  async function startQr() {
    if (startedRef.current) return;
    startedRef.current = true;

    // Resume existing QR session if available
    const existingSessionId = sessionIdRef.current;
    const existingData = qrData;
    if (existingSessionId && existingData) {
      setStatus("pending");
      startPolling(existingSessionId, existingData);
      return;
    }

    stopPolling();
    setStatus("loading");
    setError(null);

    try {
      const sessionId = await commands.createSession();
      const data = await commands.qrLoginStart(sessionId);
      sessionIdRef.current = sessionId;
      setQrData(data);
      setStatus("pending");

      // Persist for session resume
      useUiStore.setState({ qrSessionId: sessionId, qrData: data });

      startPolling(sessionId, data);
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
    sessionIdRef.current = null;
    useUiStore.setState({ qrSessionId: null, qrData: null });
    startedRef.current = false;
    startQr();
  }

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- intentional: initializing QR session on mount
    startQr();
    return stopPolling;
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="flex w-full flex-col items-center">
      {/* Header — hide when enlarged */}
      {!enlarged && (
        <div className="mb-5 flex flex-col items-center">
          <img
            src="/app-icon.png"
            alt="MapleLink"
            className="mb-2.5 h-10 w-10 rounded-[10px] shadow-[0_4px_20px_var(--accent-glow)]"
          />
          <div className="text-[12px] tracking-[4px] text-text-dim uppercase">
            {t("login.qr.title")}
          </div>
          <div className="mt-1.5 text-[12px] tracking-[0.5px] text-text-faint">
            {t("login.qr.instruction")}
          </div>
        </div>
      )}

      <div className="flex w-full flex-col items-center gap-3 rounded-[14px] border border-border bg-[var(--surface)] p-5">
        <div
          className={`flex items-center justify-center rounded-xl bg-white shadow-[0_2px_12px_rgba(0,0,0,0.08)] ${enlarged ? "p-5" : "h-[180px] w-[180px] p-4"}`}
        >
          {status === "loading" ? (
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-accent border-t-transparent" />
          ) : qrData?.qrImageUrl ? (
            <img
              src={qrData.qrImageUrl}
              alt="QR Code"
              className="block rounded"
              style={{
                width: enlarged ? 380 : 150,
                height: enlarged ? 380 : 150,
                imageRendering: "pixelated",
              }}
            />
          ) : (
            <div className="text-xs text-text-faint">—</div>
          )}
        </div>

        {/* Copy & Enlarge buttons */}
        {qrData?.qrImageUrl && status !== "loading" && (
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={async () => {
                if (!qrData?.qrImageUrl) return;
                try {
                  const resp = await fetch(qrData.qrImageUrl);
                  const blob = await resp.blob();
                  await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 1500);
                } catch {
                  /* clipboard write failed */
                }
              }}
              title={t("login.qr.copy")}
              className={`flex items-center gap-1 rounded-md px-2 py-1 text-[11px] transition-colors ${
                copied
                  ? "text-green-400"
                  : "text-text-dim hover:bg-[var(--surface-hover)] hover:text-accent"
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
              {copied ? t("common.copied") : t("login.qr.copy")}
            </button>
            <button
              type="button"
              onClick={() => {
                if (qrData?.qrImageUrl) {
                  if (!enlarged) {
                    commands.resizeWindow("login-enlarged").catch(() => {});
                    setEnlarged(true);
                  } else {
                    commands.resizeWindow("login").catch(() => {});
                    setEnlarged(false);
                  }
                }
              }}
              title={enlarged ? t("login.qr.shrink") : t("login.qr.enlarge")}
              className="flex items-center gap-1 rounded-md px-2 py-1 text-[11px] text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
            >
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
                {enlarged ? (
                  <>
                    <polyline points="4 14 10 14 10 20" />
                    <polyline points="20 10 14 10 14 4" />
                    <line x1="14" y1="10" x2="21" y2="3" />
                    <line x1="3" y1="21" x2="10" y2="14" />
                  </>
                ) : (
                  <>
                    <polyline points="15 3 21 3 21 9" />
                    <polyline points="9 21 3 21 3 15" />
                    <line x1="21" y1="3" x2="14" y2="10" />
                    <line x1="3" y1="21" x2="10" y2="14" />
                  </>
                )}
              </svg>
              {enlarged ? t("login.qr.shrink") : t("login.qr.enlarge")}
            </button>
          </div>
        )}

        {!enlarged && (
          <div className="animate-pulse text-[12px] tracking-[1px] text-text-dim">
            {status === "expired"
              ? t("login.qr.expired")
              : status === "error"
                ? (error ?? "Error")
                : t("login.qr.waiting")}
          </div>
        )}
      </div>

      {!enlarged && status === "expired" && (
        <button
          type="button"
          onClick={handleRefresh}
          className="mt-4 w-full rounded-lg bg-accent px-4 py-2.5 text-[12px] font-semibold tracking-[1.5px] text-white uppercase hover:opacity-90"
        >
          {t("login.qr.refresh")}
        </button>
      )}

      {!enlarged && (
        <button
          type="button"
          onClick={() => {
            onBack();
          }}
          className="mt-4 w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
        >
          {t("login.back_normal")}
        </button>
      )}
    </div>
  );
}
