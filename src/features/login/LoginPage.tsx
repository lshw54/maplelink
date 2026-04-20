import { useState, useCallback, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useUiStore } from "../../lib/stores/ui-store";
import { useErrorToastStore } from "../../lib/stores/error-toast-store";
import { StatusBar } from "../shared/StatusBar";
import { Modal } from "../shared/Modal";
import { NormalLoginForm } from "./NormalLoginForm";
import { QrLoginForm } from "./QrLoginForm";
import { TotpForm } from "./TotpForm";
import { VerifyForm } from "./VerifyForm";
import type { SessionDto } from "../../lib/types";

type LoginView = "normal" | "qr" | "totp" | "verify" | "gamepass";

export function LoginPage() {
  const { t } = useTranslation();
  const loginError = useAuthStore((s) => s.loginError);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const setPage = useUiStore((s) => s.setPage);
  const queryClient = useQueryClient();
  const persistedView = useUiStore((s) => s.loginView);
  const [view, setViewLocal] = useState<LoginView>((persistedView as LoginView) || "normal");
  const setView = (v: LoginView) => {
    setViewLocal(v);
    useUiStore.setState({ loginView: v });
  };
  const [advanceCheckUrl, setAdvanceCheckUrl] = useState<string | undefined>();
  const [appVersion, setAppVersion] = useState("...");
  const [showRelaunchConfirm, setShowRelaunchConfirm] = useState(false);

  useEffect(() => {
    commands
      .getAppVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  // Navigate to main page when authenticated (unless adding a new session)
  useEffect(() => {
    if (isAuthenticated && !useUiStore.getState().addingSession) {
      setPage("main");
    }
  }, [isAuthenticated, setPage]);

  // Listen for GamePass login completion event from backend
  useEffect(() => {
    const unlistenComplete = listen<SessionDto>("gamepass-login-complete", async (event) => {
      useAuthStore.getState().addSession(event.payload);
      try {
        const accounts = await commands.getGameAccounts(event.payload.sessionId);
        useAuthStore.getState().updateGameAccounts(event.payload.sessionId, accounts);
      } catch {
        /* non-critical */
      }
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      useUiStore.setState({ addingSession: false, loginView: "normal" });
      setPage("main");
    });

    const unlistenError = listen<string>("gamepass-login-error", (event) => {
      useErrorToastStore.getState().addToast({
        message: event.payload,
        category: "authentication",
        critical: false,
      });
      setView("normal");
    });

    const unlistenCancelled = listen("gamepass-login-cancelled", () => {
      setView("normal");
    });

    return () => {
      unlistenComplete.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenCancelled.then((fn) => fn());
    };
  }, [queryClient, setPage]);

  const handleTotpRequired = useCallback(() => {
    setView("totp");
  }, []);

  const handleAdvanceCheck = useCallback((url?: string) => {
    setAdvanceCheckUrl(url);
    setView("verify");
  }, []);

  const handleGamePass = useCallback(() => {
    setView("gamepass");
    commands.openGamePassLogin().catch((err) => {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as Record<string, unknown>).message)
          : String(err);
      useErrorToastStore.getState().addToast({
        message: msg,
        category: "authentication",
        critical: false,
      });
      setView("normal");
    });
  }, []);

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 flex-col items-center justify-center px-9">
        {view === "normal" && (
          <>
            <div className="mb-6 flex flex-col items-center">
              <img
                src="/app-icon.png"
                alt="MapleLink"
                className="mb-2.5 h-10 w-10 rounded-[10px] shadow-[0_4px_20px_var(--accent-glow)]"
              />
              <div className="text-[12px] tracking-[4px] text-text-dim uppercase">
                {t("app.name")}
              </div>
            </div>

            {loginError && <p className="mb-2 w-full text-xs text-[var(--danger)]">{loginError}</p>}

            <NormalLoginForm
              onShowQr={() => setView("qr")}
              onTotpRequired={handleTotpRequired}
              onAdvanceCheck={handleAdvanceCheck}
              onGamePass={handleGamePass}
            />

            {useUiStore.getState().addingSession && (
              <button
                type="button"
                onClick={() => {
                  useUiStore.getState().addingSession = false;
                  setPage("main");
                }}
                className="mt-3 w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
              >
                {t("login.back_to_accounts")}
              </button>
            )}
          </>
        )}

        {view === "qr" && <QrLoginForm onBack={() => setView("normal")} />}
        {view === "totp" && <TotpForm onBack={() => setView("normal")} />}
        {view === "verify" && (
          <VerifyForm
            advanceCheckUrl={advanceCheckUrl}
            onBack={() => setView("normal")}
            onVerified={() => setView("normal")}
            onAdvanceCheck={handleAdvanceCheck}
            onTotpRequired={handleTotpRequired}
          />
        )}

        {view === "gamepass" && (
          <div className="flex w-full flex-col items-center gap-4">
            <img
              src="/app-icon.png"
              alt="MapleLink"
              className="mb-2.5 h-10 w-10 rounded-[10px] shadow-[0_4px_20px_var(--accent-glow)]"
            />
            <div className="text-sm font-medium text-[var(--text)]">
              {t("login.gamepass_waiting")}
            </div>
            <div className="text-[12px] text-text-dim">{t("login.gamepass_instruction")}</div>
            <div className="mt-2 h-1 w-32 overflow-hidden rounded-full bg-[var(--surface)]">
              <div className="h-full w-1/3 animate-[shimmer_1.5s_ease-in-out_infinite] rounded-full bg-accent" />
            </div>
            <button
              onClick={() => setView("normal")}
              className="mt-4 text-[12px] text-text-dim transition-colors hover:text-accent"
            >
              {t("login.back_normal")}
            </button>
          </div>
        )}
      </div>

      <StatusBar />
      <div className="shrink-0 pb-2 text-center">
        <button
          type="button"
          onClick={async () => {
            try {
              const running = await commands.isGameRunning();
              if (running) {
                setShowRelaunchConfirm(true);
                return;
              }
            } catch {
              /* ignore, proceed */
            }
            await doDirectLaunch();
          }}
          className="mb-1.5 rounded-md px-3 py-1 text-[11px] text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
        >
          ▶ {t("login.launch_game_direct")}
        </button>
        <GameRunningIndicator />
        <div className="font-mono text-[12px] text-text-faint">MapleLink v{appVersion}</div>
      </div>

      <Modal
        isOpen={showRelaunchConfirm}
        onClose={() => setShowRelaunchConfirm(false)}
        title={t("launcher.relaunch_title")}
      >
        <div className="flex flex-col gap-4">
          <p className="text-xs text-text-dim">{t("launcher.relaunch_message")}</p>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowRelaunchConfirm(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={async () => {
                setShowRelaunchConfirm(false);
                try {
                  await commands.killGame();
                  useUiStore.getState().setGamePid(null);
                  useUiStore.getState().setGameRunning(false);
                  await new Promise((r) => setTimeout(r, 500));
                } catch {
                  /* proceed anyway */
                }
                await doDirectLaunch();
              }}
              className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("launcher.relaunch_confirm")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

async function doDirectLaunch() {
  const { setGamePid, setGameRunning } = useUiStore.getState();
  try {
    const processId = await commands.launchGameDirect();
    if (processId > 0) {
      setGamePid(processId);
      setGameRunning(true);
    }
  } catch (err) {
    const msg =
      typeof err === "object" && err !== null && "message" in err
        ? String((err as Record<string, unknown>).message)
        : String(err);
    useErrorToastStore.getState().addToast({
      message: msg,
      category: "process",
      critical: false,
    });
  }
}

/** Small indicator that polls game running state and shows PID. */
function GameRunningIndicator() {
  const { t } = useTranslation();
  const gamePid = useUiStore((s) => s.gamePid);
  const gameRunning = useUiStore((s) => s.gameRunning);
  const setGamePid = useUiStore((s) => s.setGamePid);
  const setGameRunning = useUiStore((s) => s.setGameRunning);

  useEffect(() => {
    if (gamePid === null && !gameRunning) return;
    const interval = setInterval(async () => {
      try {
        const running = await commands.isGameRunning();
        setGameRunning(running);
        if (running) {
          const realPid = await commands.getGamePid();
          if (realPid > 0) setGamePid(realPid);
        }
      } catch {
        setGameRunning(false);
      }
    }, 3000);
    return () => clearInterval(interval);
  }, [gamePid, gameRunning, setGamePid, setGameRunning]);

  if (!gameRunning && gamePid === null) return null;

  return (
    <div className="mb-1 text-[11px] text-accent">
      {t("launcher.running")}
      {gamePid !== null ? ` (PID: ${gamePid})` : ""}
    </div>
  );
}
