import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useUiStore } from "../../lib/stores/ui-store";
import { useConfigStore } from "../../lib/stores/config-store";
import { autoLaunchGameIfEnabled } from "../../lib/hooks/use-auth";
import type { QrCodeData, QrPollResult } from "../../lib/types";

/**
 * The QR image as a PNG blob, decoded from the `data:` URL it arrives in.
 *
 * Decoded rather than fetched, and synchronously, because `clipboard.write`
 * needs the click that called it to still be in progress. Awaiting a `fetch`
 * first — even of a `data:` URL, even for microseconds — spends that activation,
 * and Chromium then rejects the write with `NotAllowedError`. That was the whole
 * bug: the button worked exactly as written and the browser refused it.
 */
function pngFromDataUrl(url: string): Blob | null {
  const comma = url.indexOf(",");
  if (comma < 0 || !url.startsWith("data:image/png;base64,")) return null;
  try {
    const binary = atob(url.slice(comma + 1));
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return new Blob([bytes], { type: "image/png" });
  } catch {
    return null;
  }
}

/**
 * How long to keep asking whether a code has been scanned.
 *
 * A code lasts three minutes — measured, not guessed: issued 04:42:03, refused
 * at 04:45:03 — and beanfun says so itself, answering `Token Expired`, which is
 * what normally ends the polling. This is the backstop for the case where that
 * answer never arrives, with enough headroom that it never fires first.
 */
const POLL_LIFETIME_MS = 5 * 60 * 1000;

/**
 * How long a code is good for. Measured, not guessed: issued 04:42:03, refused
 * at 04:45:03. beanfun says so itself by answering `Token Expired`, and the two
 * agree — this is what lets the window count down rather than only find out
 * afterwards.
 */
const QR_LIFETIME_MS = 3 * 60 * 1000;

/** `m:ss`, or `0:00` once there is nothing left. */
function asClock(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

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
  const [linkCopied, setLinkCopied] = useState(false);
  const [enlarged, setEnlarged] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startedRef = useRef(false);
  /** Consecutive failed polls, to tell a blink apart from a dead session. */
  const failures = useRef(0);
  /** Milliseconds left on the current code, or null when there isn't one. */
  const [remaining, setRemaining] = useState<number | null>(null);
  const sessionIdRef = useRef<string | null>(useUiStore.getState().qrSessionId);

  function stopPolling() {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }

  function startPolling(sessionId: string, data: QrCodeData) {
    stopPolling();
    failures.current = 0;
    const startedAt = Date.now();
    intervalRef.current = setInterval(async () => {
      // Normally unreachable: beanfun answers `Token Expired` at three minutes
      // and the poll stops there. This catches the case where it never says so
      // — a request that keeps failing, or an answer that never comes — which
      // would otherwise leave this running for as long as the window is open.
      if (Date.now() - startedAt > POLL_LIFETIME_MS) {
        stopPolling();
        setStatus("expired");
        useUiStore.setState({ qrSessionId: null, qrData: null, qrIssuedAt: null });
        return;
      }
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
            useAuthStore.getState().addSession(confirmedSession, undefined, "qr");
            const accounts = await commands.getGameAccounts(sessionId);
            useAuthStore.getState().updateGameAccounts(sessionId, accounts);
            // Clear persisted QR state
            useUiStore.setState({
              qrSessionId: null,
              qrData: null,
              qrIssuedAt: null,
              loginView: "normal",
              addingSession: false,
            });
            // Reset window size if enlarged
            commands.resizeWindow("login").catch(() => {});
            useUiStore.getState().setPage("main");
            autoLaunchGameIfEnabled(sessionId);
          }
        } else if (result.status === "expired") {
          stopPolling();
          setStatus("expired");
          useUiStore.setState({ qrSessionId: null, qrData: null, qrIssuedAt: null });
        }
        failures.current = 0;
      } catch {
        // One failed poll is nothing — the network blinks. A run of them means
        // the session is no longer answering, and saying so is more honest than
        // showing a code that cannot be scanned.
        failures.current += 1;
        if (failures.current >= 5) {
          stopPolling();
          setStatus("expired");
          useUiStore.setState({ qrSessionId: null, qrData: null, qrIssuedAt: null });
        }
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
      useUiStore.setState({ qrSessionId: sessionId, qrData: data, qrIssuedAt: Date.now() });

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
    stopPolling();
    sessionIdRef.current = null;
    // Cleared, not just replaced: if the new code never arrives, the old one
    // must not sit there next to an error message looking scannable.
    setQrData(null);
    useUiStore.setState({ qrSessionId: null, qrData: null, qrIssuedAt: null });
    startedRef.current = false;
    startQr();
  }

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- intentional: initializing QR session on mount
    startQr();
    return stopPolling;
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // The clock, ticking off the issue time rather than off a counter, so leaving
  // the view and coming back shows what is actually left rather than restarting
  // at three minutes.
  useEffect(() => {
    const tick = () => {
      const issuedAt = useUiStore.getState().qrIssuedAt;
      if (issuedAt === null) {
        setRemaining(null);
        return;
      }
      const left = issuedAt + QR_LIFETIME_MS - Date.now();
      setRemaining(left);
      // Said here as well as by the server: beanfun answers `Token Expired` at
      // the same moment, but only when asked, and the window should not show a
      // live-looking code for the two seconds until the next poll.
      if (left <= 0) {
        stopPolling();
        setStatus("expired");
      }
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [status, qrData]);

  // Compact UI: the shorter window drops the logo and shows a smaller code
  // (still comfortably scannable; "enlarge" is a click away).
  const compact = useConfigStore((s) => s.config?.compactUi ?? false);
  const expired = status === "expired" || status === "error";
  const qrBox = enlarged ? "p-4" : compact ? "h-[172px] w-[172px] p-2" : "h-[228px] w-[228px] p-2";
  const qrPx = enlarged ? 396 : compact ? 156 : 212;

  return (
    <div className="flex w-full flex-col items-center">
      {/* Header */}
      <div className={`flex flex-col items-center ${enlarged || compact ? "mb-3" : "mb-5"}`}>
        {!compact && (
          <img
            src="/app-logo.png"
            alt="MapleLink"
            className="mb-2.5 h-10 w-10 rounded-[10px] shadow-[0_4px_20px_var(--accent-glow)]"
          />
        )}
        <div className="text-[12px] tracking-[4px] text-text-dim uppercase">
          {t("login.qr.title")}
        </div>
        {!enlarged && (
          <div className="mt-1.5 text-[12px] tracking-[0.5px] text-text-faint">
            {t("login.qr.instruction")}
          </div>
        )}
      </div>

      <div
        className={`flex w-full flex-col items-center gap-3 rounded-[14px] border border-border bg-[var(--surface)] ${compact ? "p-3" : "p-5"}`}
      >
        <div
          className={`relative flex items-center justify-center rounded-xl bg-white shadow-[0_2px_12px_rgba(0,0,0,0.08)] ${qrBox}`}
        >
          {status === "loading" ? (
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-accent border-t-transparent" />
          ) : qrData?.qrImageUrl ? (
            <img
              src={qrData.qrImageUrl}
              alt="QR Code"
              className={`block rounded transition-all ${expired ? "opacity-25 blur-[2px]" : ""}`}
              style={{
                width: qrPx,
                height: qrPx,
                imageRendering: "pixelated",
              }}
            />
          ) : (
            <div className="text-xs text-text-faint">—</div>
          )}

          {/* An expired code is answered where it is, rather than by a button
              further down the window — which is also how the old client did it,
              and which stops the layout growing a row it did not have. */}
          {expired && (
            <button
              type="button"
              onClick={handleRefresh}
              title={t("login.qr.refresh")}
              className="absolute inset-0 flex flex-col items-center justify-center gap-2 rounded-xl"
            >
              <span className="flex h-11 w-11 items-center justify-center rounded-full bg-accent text-[var(--on-accent)] shadow-md transition-transform hover:scale-105">
                <svg width="26" height="26" viewBox="0 0 44 44" fill="currentColor">
                  <path d="M32.4,11.6C29.7,9,26.1,7.3,22,7.3C13.9,7.3,7.4,13.9,7.4,22c0,8.1,6.5,14.7,14.6,14.7 c6.8,0,12.5-4.7,14.2-11h-3.8C30.9,29.9,26.8,33,22,33c-6.1,0-11-4.9-11-11c0-6.1,4.9-11,11-11c3,0,5.8,1.3,7.7,3.3l-5.9,5.9h12.8 V7.3L32.4,11.6z" />
                </svg>
              </span>
              <span className="text-[11px] font-semibold text-[#555]">
                {t("login.qr.expired_short")}
              </span>
            </button>
          )}
        </div>

        {/* Copy & Enlarge buttons */}
        {qrData?.qrImageUrl && status !== "loading" && (
          <div className="flex flex-wrap items-center justify-center gap-2">
            <button
              type="button"
              onClick={async () => {
                const blob = qrData?.qrImageUrl ? pngFromDataUrl(qrData.qrImageUrl) : null;
                if (!blob) return;
                try {
                  await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 1500);
                } catch (e) {
                  // Silence here is why this went unreported as broken for so
                  // long: the button simply did nothing.
                  commands
                    .logFrontendError("warn", "QrLoginForm", `copying the QR image failed: ${e}`)
                    .catch(() => {});
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
            {qrData?.deeplink && (
              <button
                type="button"
                onClick={async () => {
                  if (!qrData?.deeplink) return;
                  await navigator.clipboard.writeText(qrData.deeplink);
                  setLinkCopied(true);
                  setTimeout(() => setLinkCopied(false), 1500);
                }}
                title={t("login.qr.copy_deeplink")}
                className={`flex items-center gap-1 rounded-md px-2 py-1 text-[11px] transition-colors ${
                  linkCopied
                    ? "text-green-400"
                    : "text-text-dim hover:bg-[var(--surface-hover)] hover:text-accent"
                }`}
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
                  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
                  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
                </svg>
                {linkCopied ? t("common.copied") : t("login.qr.copy_deeplink")}
              </button>
            )}
          </div>
        )}

        {!enlarged && (
          <div className="flex flex-col items-center gap-0.5">
            <div className="animate-pulse text-[12px] tracking-[1px] text-text-dim">
              {status === "expired"
                ? t("login.qr.expired")
                : status === "error"
                  ? (error ?? "Error")
                  : t("login.qr.waiting")}
            </div>
            {/* Steady, unlike the line above it: a clock that fades in and out
                is harder to read than one that does not. */}
            {status === "pending" && remaining !== null && remaining > 0 && (
              <div className="text-[11px] tracking-[0.5px] text-text-faint tabular-nums">
                {t("login.qr.valid_for")}: {asClock(remaining)}
              </div>
            )}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={() => {
          if (enlarged) {
            commands.resizeWindow("login").catch(() => {});
            setEnlarged(false);
          }
          onBack();
        }}
        className={`w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent ${compact ? "mt-2.5" : "mt-4"}`}
      >
        {t("login.back_normal")}
      </button>
    </div>
  );
}
