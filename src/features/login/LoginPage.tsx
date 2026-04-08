import { useState, useCallback, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useTranslation } from "../../lib/i18n";
import { useUiStore } from "../../lib/stores/ui-store";
import { useErrorToastStore } from "../../lib/stores/error-toast-store";
import { StatusBar } from "../shared/StatusBar";
import { NormalLoginForm } from "./NormalLoginForm";
import { QrLoginForm } from "./QrLoginForm";
import { TotpForm } from "./TotpForm";
import { VerifyForm } from "./VerifyForm";
import { commands } from "../../lib/tauri";
import type { SessionDto } from "../../lib/types";

type LoginView = "normal" | "qr" | "totp" | "verify" | "gamepass";

export function LoginPage() {
  const { t } = useTranslation();
  const loginError = useAuthStore((s) => s.loginError);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const setPage = useUiStore((s) => s.setPage);
  const queryClient = useQueryClient();
  const [view, setView] = useState<LoginView>("normal");
  const [advanceCheckUrl, setAdvanceCheckUrl] = useState<string | undefined>();

  // Navigate to main page when authenticated
  useEffect(() => {
    if (isAuthenticated) {
      setPage("main");
    }
  }, [isAuthenticated, setPage]);

  // Listen for GamePass login completion event from backend
  useEffect(() => {
    const { setSession, setGameAccounts } = useAuthStore.getState();

    const unlistenComplete = listen<SessionDto>("gamepass-login-complete", async (event) => {
      setSession(event.payload);
      try {
        const accounts = await commands.getGameAccounts();
        setGameAccounts(accounts);
      } catch {
        /* non-critical */
      }
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
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
  }, [queryClient]);

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
              <div className="text-[12px] uppercase tracking-[4px] text-text-dim">
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
      <div className="shrink-0 pb-2 text-center font-mono text-[12px] text-text-faint">
        MapleLink v0.1.0 · Tauri 2
      </div>
    </div>
  );
}
