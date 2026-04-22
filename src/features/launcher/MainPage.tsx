import { useState, useCallback, useEffect, useRef } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { useLogout } from "../../lib/hooks/use-auth";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useConfigStore } from "../../lib/stores/config-store";
import { useUiStore } from "../../lib/stores/ui-store";
import { AccountGrid } from "./AccountGrid";
import { OtpPanel } from "./OtpPanel";
import { SessionTabs } from "./SessionTabs";
import { StatusBar } from "../shared/StatusBar";
import { Modal } from "../shared/Modal";
import type { GameAccountDto } from "../../lib/types";

export function MainPage() {
  const { t } = useTranslation();
  const session = useAuthStore((s) => s.session);
  const activeSessionId = useAuthStore((s) => s.activeSessionId);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const setPage = useUiStore((s) => s.setPage);
  const region = useConfigStore((s) => s.config?.region ?? "HK");
  const logout = useLogout();
  const [appVersion, setAppVersion] = useState("0.0.0");
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [launching, setLaunching] = useState(false);
  const gamePid = useUiStore((s) => s.gamePid);
  const gameRunning = useUiStore((s) => s.gameRunning);
  const setGamePid = useUiStore((s) => s.setGamePid);
  const setGameRunning = useUiStore((s) => s.setGameRunning);
  // Latest OTP fetched by OtpPanel — used to skip HTTP round-trip on launch.
  const latestOtpRef = useRef<{ accountId: string; otp: string } | null>(null);

  // Poll game running status and update PID from backend's active_processes.
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

  const [remainPoint, setRemainPoint] = useState<number>(0);
  const [showRelaunchConfirm, setShowRelaunchConfirm] = useState(false);
  const [showLogoutConfirm, setShowLogoutConfirm] = useState(false);
  const [pendingLaunchId, setPendingLaunchId] = useState<string | null>(null);
  const [beansMenuOpen, setBeansMenuOpen] = useState(false);
  const beansRef = useRef<HTMLSpanElement>(null);

  // Redirect to login if all sessions are cleared (e.g. expired server-side)
  useEffect(() => {
    if (!isAuthenticated && useAuthStore.getState().sessions.size === 0) {
      setPage("login");
    }
  }, [isAuthenticated, setPage]);

  // Fetch remain points on mount + auto-detect game path if empty
  useEffect(() => {
    commands
      .getRemainPoint(activeSessionId ?? "")
      .then(setRemainPoint)
      .catch(() => {});
    commands
      .getAppVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, [activeSessionId]);

  // Auto-detect game path on first mount only
  useEffect(() => {
    const currentConfig = useConfigStore.getState().config;
    if (!currentConfig?.gamePath) {
      commands
        .detectGamePath()
        .then((path) => {
          if (path) {
            commands.setConfig("game_path", path).catch(() => {});
            const current = useConfigStore.getState().config;
            if (current) {
              useConfigStore.getState().setConfig({ ...current, gamePath: path });
            }
          }
        })
        .catch(() => {});
    }
  }, []);

  // Session keep-alive: frontend backup ping every 60 seconds.
  // The main ping loop runs in the Rust backend (not affected by browser throttling).
  // This frontend ping is a backup + wake-from-sleep detector.
  useEffect(() => {
    let lastPingTime = Date.now();

    async function checkSession() {
      const now = Date.now();
      const elapsed = now - lastPingTime;
      lastPingTime = now;

      // If more than 5 minutes since last tick, computer likely slept.
      // Verify session is still alive.
      if (elapsed > 5 * 60 * 1000) {
        try {
          await commands.getRemainPoint(useAuthStore.getState().activeSessionId ?? "");
        } catch {
          useAuthStore.getState().clearSession();
          return;
        }
      }

      commands.pingSession(useAuthStore.getState().activeSessionId ?? "").catch(() => {});
    }

    function onVisibilityChange() {
      if (document.visibilityState === "visible") {
        checkSession();
      }
    }
    document.addEventListener("visibilitychange", onVisibilityChange);

    const interval = setInterval(checkSession, 60_000);
    return () => {
      clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [activeSessionId]);

  const handleSelectAccount = useCallback((account: GameAccountDto) => {
    setSelectedAccountId(account.id);
  }, []);

  const handleLaunch = useCallback(
    async (accountId: string, otp?: string) => {
      setSelectedAccountId(accountId);
      setLaunching(true);
      try {
        const processId = await commands.launchGame(activeSessionId ?? "", accountId, otp);
        if (processId > 0) {
          setGamePid(processId);
          setGameRunning(true);
        }
      } finally {
        setLaunching(false);
      }
    },
    [activeSessionId, setGamePid, setGameRunning],
  );

  async function handlePlayClick() {
    const config = useConfigStore.getState().config;
    const traditionalLogin = config?.traditionalLogin ?? true;

    // In traditional login mode, launch directly without fetching OTP
    if (traditionalLogin) {
      // Check if game is already running
      try {
        const running = await commands.isGameRunning();
        if (running) {
          setPendingLaunchId("__direct__");
          setShowRelaunchConfirm(true);
          return;
        }
      } catch {
        /* ignore */
      }

      setLaunching(true);
      try {
        const processId = await commands.launchGameDirect();
        if (processId > 0) {
          setGamePid(processId);
          setGameRunning(true);
        }
      } catch (err) {
        // Show error via toast
        const msg =
          typeof err === "object" && err !== null && "message" in err
            ? String((err as Record<string, unknown>).message)
            : String(err);
        console.error("launch failed:", msg); // eslint-disable-line no-console
      } finally {
        setLaunching(false);
      }
      return;
    }

    // Non-traditional: need account + OTP
    let accountId = selectedAccountId;
    if (!accountId) {
      try {
        const accounts = await commands.getGameAccounts(activeSessionId ?? "");
        if (accounts.length > 0) {
          const first = accounts[0];
          if (!first) return;
          setSelectedAccountId(first.id);
          accountId = first.id;
        }
      } catch {
        return;
      }
    }
    if (!accountId) return;

    // Check if game is already running
    try {
      const running = await commands.isGameRunning();
      if (running) {
        setPendingLaunchId(accountId);
        setShowRelaunchConfirm(true);
        return;
      }
    } catch {
      /* ignore, proceed with launch */
    }

    await handleLaunch(
      accountId,
      latestOtpRef.current?.accountId === accountId ? latestOtpRef.current.otp : undefined,
    );
  }

  async function handleConfirmRelaunch() {
    setShowRelaunchConfirm(false);
    // Kill the running game first, then relaunch
    try {
      await commands.killGame();
      setGamePid(null);
      setGameRunning(false);
      await new Promise((r) => setTimeout(r, 500));
    } catch {
      /* proceed with launch anyway */
    }

    if (pendingLaunchId === "__direct__") {
      setLaunching(true);
      try {
        const processId = await commands.launchGameDirect();
        if (processId > 0) {
          setGamePid(processId);
          setGameRunning(true);
        }
      } finally {
        setLaunching(false);
      }
    } else if (pendingLaunchId) {
      await handleLaunch(
        pendingLaunchId,
        latestOtpRef.current?.accountId === pendingLaunchId ? latestOtpRef.current.otp : undefined,
      );
    }
    setPendingLaunchId(null);
  }

  return (
    <div className="flex h-full flex-col">
      <SessionTabs />
      <div className="flex flex-1 overflow-hidden">
        {/* Left: Focus Side (40%) */}
        <div className="relative flex w-[40%] shrink-0 flex-col items-center justify-center overflow-hidden p-6">
          {/* Ghost icon bg */}
          <div className="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 opacity-[0.04] blur-[2px]">
            <img src="/MapleStory.ico" alt="" className="h-[160px] w-[160px]" draggable={false} />
          </div>

          <div className="relative z-10 flex flex-col items-center gap-4">
            <div className="flex h-14 w-14 items-center justify-center overflow-hidden rounded-[14px] border border-border bg-[var(--surface-hover)]">
              <img
                src="/MapleStory.ico"
                alt="Game"
                className="h-10 w-10 object-contain"
                draggable={false}
              />
            </div>
            <div className="text-center text-base font-extrabold tracking-[0.5px] text-[var(--text)]">
              {t("launcher.game_info")}
            </div>
            <div className="text-[12px] tracking-[1px] text-text-dim">Gamania · MMORPG</div>

            {/* Circular PLAY button */}
            <button
              onClick={handlePlayClick}
              disabled={launching}
              className="relative mt-1 flex h-[72px] w-[72px] items-center justify-center rounded-full border-none bg-gradient-to-br from-[#c46a00] to-accent text-[12px] font-extrabold tracking-[3px] text-white uppercase shadow-[0_4px_24px_var(--accent-glow),0_0_0_3px_rgba(232,162,58,0.1)] transition-all hover:scale-[1.08] hover:shadow-[0_6px_32px_rgba(232,162,58,0.5)] active:scale-[0.93] disabled:transform-none disabled:opacity-40"
            >
              {launching ? "..." : t("launcher.play")}
            </button>

            {(gameRunning || gamePid !== null) && (
              <span className="text-[12px] text-accent">
                {t("launcher.running")}
                {gamePid !== null ? ` (PID: ${gamePid})` : ""}
              </span>
            )}

            {/* Nav links */}
            <div className="mt-2 flex gap-2.5">
              <button
                onClick={() => setShowLogoutConfirm(true)}
                disabled={logout.isPending}
                className="rounded-md bg-[var(--surface)] px-2.5 py-1 text-[12px] font-semibold tracking-[1px] text-text-dim uppercase transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-[0.93]"
              >
                {t("launcher.logout")}
              </button>
            </div>
          </div>

          {/* Bottom status */}
          <div className="absolute right-0 bottom-0 left-0">
            <StatusBar />
            <div className="shrink-0 pb-2 text-center font-mono text-[12px] text-text-faint">
              MapleLink v{appVersion}
            </div>
          </div>
        </div>

        {/* Right: Account Side (60%) */}
        <div className="flex flex-1 flex-col border-l border-border">
          {/* Top bar */}
          <div className="flex shrink-0 items-center gap-1.5 border-b border-border px-3 py-2">
            <div className="flex shrink-0 items-center gap-1.5 text-[12px] text-text-dim">
              <div className="flex h-[22px] w-[22px] items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[12px] font-bold text-white">
                {session?.accountName?.charAt(0)?.toUpperCase() ?? "?"}
              </div>
              <span className="max-w-[160px] truncate">{session?.accountName ?? ""}</span>
            </div>
            <div className="flex-1" />
            <div className="flex items-center gap-1.5">
              <div className="relative">
                <span
                  ref={beansRef}
                  onClick={() => setBeansMenuOpen(!beansMenuOpen)}
                  className="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-md border border-[rgba(232,162,58,0.15)] bg-[rgba(232,162,58,0.08)] px-2 py-0.5 text-[12px] whitespace-nowrap transition-all hover:bg-[rgba(232,162,58,0.14)]"
                >
                  <span className="font-semibold text-accent">
                    {t("launcher.beans")}: <b>{remainPoint}</b>
                  </span>
                  {remainPoint > 0 && (
                    <>
                      <span className="text-text-faint">·</span>
                      <span className="text-text-dim">
                        {t("launcher.game_points")}: <b>{Math.floor(remainPoint / 2.5)}</b>
                      </span>
                    </>
                  )}
                </span>
                {beansMenuOpen && (
                  <BeansPopupMenu
                    t={t}
                    region={region}
                    onRefresh={async () => {
                      const pts = await commands.getRemainPoint(activeSessionId ?? "");
                      setRemainPoint(pts);
                      setBeansMenuOpen(false);
                    }}
                    onClose={() => setBeansMenuOpen(false)}
                    sessionId={activeSessionId ?? ""}
                  />
                )}
              </div>
              <span
                onClick={() => commands.openMemberPopup(activeSessionId ?? "").catch(() => {})}
                className="cursor-pointer truncate text-[12px] text-text-dim transition-colors hover:text-accent"
              >
                {t("launcher.member_center")}
              </span>
              <span
                onClick={() => commands.openCustomerService().catch(() => {})}
                className="cursor-pointer truncate text-[12px] text-text-dim transition-colors hover:text-accent"
              >
                {t("launcher.support")}
              </span>
            </div>
          </div>

          {/* Account grid */}
          <div className="flex-1 overflow-y-auto p-4">
            <AccountGrid
              selectedAccountId={selectedAccountId}
              onSelectAccount={handleSelectAccount}
            />
          </div>

          {/* OTP Panel */}
          <OtpPanel
            selectedAccountId={selectedAccountId}
            onOtpFetched={(accountId: string, otp: string) => {
              latestOtpRef.current = { accountId, otp };
            }}
          />
        </div>
      </div>
      {/* close inner flex */}

      {/* Relaunch confirmation modal */}
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
              onClick={handleConfirmRelaunch}
              className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("launcher.relaunch_confirm")}
            </button>
          </div>
        </div>
      </Modal>

      {/* Logout confirmation modal */}
      <Modal
        isOpen={showLogoutConfirm}
        onClose={() => setShowLogoutConfirm(false)}
        title={t("launcher.logout")}
      >
        <div className="flex flex-col gap-4">
          <p className="text-xs text-text-dim">{t("launcher.logout_confirm_message")}</p>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowLogoutConfirm(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={() => {
                setShowLogoutConfirm(false);
                logout.mutate();
              }}
              className="rounded-lg bg-[var(--danger)] px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("launcher.logout")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

function BeansPopupMenu({
  t,
  region,
  onRefresh,
  onClose,
  sessionId,
}: {
  t: (key: string) => string;
  region: string;
  onRefresh: () => void;
  onClose: () => void;
  sessionId: string;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
    }, 16);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [onClose]);

  async function handleTopup() {
    try {
      await commands.openGashPopup(sessionId);
    } catch {
      /* ignore */
    }
    onClose();
  }

  async function handleExchange() {
    try {
      await commands.openAuthPopup(
        sessionId,
        "https://m.beanfun.com/Deposite",
        t("launcher.beans_exchange"),
      );
    } catch {
      /* ignore */
    }
    onClose();
  }

  return (
    <div
      ref={menuRef}
      className="absolute top-full left-0 z-50 mt-1 min-w-[160px] animate-[ctxIn_0.15s_ease] rounded-[10px] border border-border bg-[var(--surface)] py-1.5 shadow-[0_8px_32px_rgba(0,0,0,0.3)] backdrop-blur-[20px]"
    >
      <button
        onClick={onRefresh}
        className="flex w-full items-center gap-2.5 px-4 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent"
      >
        <span className="w-4 text-center text-xs">🔄</span>
        {t("launcher.beans_refresh")}
      </button>
      <button
        onClick={handleTopup}
        className="flex w-full items-center gap-2.5 px-4 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent"
      >
        <span className="w-4 text-center text-xs">💳</span>
        {t("launcher.beans_topup")}
      </button>
      {region === "TW" && (
        <button
          onClick={handleExchange}
          className="flex w-full items-center gap-2.5 px-4 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent"
        >
          <span className="w-4 text-center text-xs">🎁</span>
          {t("launcher.beans_exchange")}
        </button>
      )}
    </div>
  );
}
