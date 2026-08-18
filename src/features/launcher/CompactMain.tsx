import { useState } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { useOtp } from "../../lib/hooks/use-otp";
import { AccountGrid } from "./AccountGrid";
import { BeansPopupMenu, MorePopupMenu } from "./PopupMenus";
import { StatusBar } from "../shared/StatusBar";
import type { SessionDto, GameAccountDto, ClassicCheckDto } from "../../lib/types";

/**
 * The compact launcher — sized like the old beanfun launcher: header · game
 * switch · accounts · one OTP + Play row · status. Pure layout: every handler
 * and piece of state comes from MainPage, which owns the launch logic and the
 * confirmation modals for both layouts.
 */
export interface CompactMainProps {
  session: SessionDto | null;
  activeSessionId: string | null;
  region: string;
  nameMask: string;
  remainPoint: number;
  onRemainPoint: (pts: number) => void;
  // Regular ↔ Classic switch (HK sessions only)
  canClassic: boolean;
  classicGame: boolean;
  onClassicGame: (on: boolean) => void;
  showClassic: boolean;
  classicCheck: ClassicCheckDto | null;
  ngmReady: boolean;
  // Launch
  launching: boolean;
  gameRunning: boolean;
  gamePid: number | null;
  onPlay: () => void;
  onLogout: () => void;
  // Accounts / OTP
  selectedAccountId: string | null;
  onSelectAccount: (account: GameAccountDto) => void;
  onOtpFetched: (accountId: string, otp: string) => void;
}

export function CompactMain(p: CompactMainProps) {
  const { t } = useTranslation();
  const [beansMenuOpen, setBeansMenuOpen] = useState(false);
  const [moreMenuOpen, setMoreMenuOpen] = useState(false);
  const otp = useOtp(p.selectedAccountId, p.onOtpFetched);
  const sessionRegion = p.session?.region ?? p.region;
  const running = !p.showClassic && (p.gameRunning || p.gamePid !== null);

  return (
    <div className="relative flex flex-1 flex-col overflow-hidden">
      {/* Header: who's signed in · beans · more */}
      <div className="flex shrink-0 items-center gap-1.5 px-2.5 py-1.5">
        <div className="flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[11px] font-bold text-white shadow-[0_1px_6px_var(--accent-glow)]">
          {p.session?.accountName?.charAt(0)?.toUpperCase() ?? "?"}
        </div>
        <span className={`min-w-0 flex-1 truncate text-[12px] font-semibold ${p.nameMask}`}>
          {p.session?.accountName ?? ""}
        </span>

        <div className="relative">
          <button
            onClick={() => setBeansMenuOpen(!beansMenuOpen)}
            className="inline-flex items-center gap-1 rounded-full border border-[rgba(232,162,58,0.18)] bg-[rgba(232,162,58,0.08)] px-2 py-[2px] text-[11px] whitespace-nowrap transition-all hover:bg-[rgba(232,162,58,0.16)]"
            title={`${t("launcher.beans")}: ${p.remainPoint}${
              sessionRegion === "HK" && p.remainPoint > 0
                ? ` · ${t("launcher.game_points")}: ${Math.floor(p.remainPoint / 2.5)}`
                : ""
            }`}
          >
            <span className="text-text-dim">{t("launcher.beans")}</span>
            <b className="text-accent">{p.remainPoint}</b>
          </button>
          {beansMenuOpen && (
            <BeansPopupMenu
              t={t}
              region={sessionRegion}
              onRefresh={async () => {
                const pts = await commands.getRemainPoint(p.activeSessionId ?? "");
                p.onRemainPoint(pts);
                setBeansMenuOpen(false);
              }}
              onClose={() => setBeansMenuOpen(false)}
              sessionId={p.activeSessionId ?? ""}
            />
          )}
        </div>

        <div className="relative">
          <button
            onClick={() => setMoreMenuOpen(!moreMenuOpen)}
            className="flex h-6 w-6 items-center justify-center rounded-full text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
            title="More"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <circle cx="3" cy="8" r="1.5" />
              <circle cx="8" cy="8" r="1.5" />
              <circle cx="13" cy="8" r="1.5" />
            </svg>
          </button>
          {moreMenuOpen && (
            <MorePopupMenu
              t={t}
              sessionId={p.activeSessionId ?? ""}
              onClose={() => setMoreMenuOpen(false)}
              onLogout={p.onLogout}
            />
          )}
        </div>
      </div>

      {/* Regular ↔ Classic — a segmented switch so what Play launches is
          unmistakable. Readiness only shows when something needs attention. */}
      {p.canClassic && (
        <div className="shrink-0 px-2.5 pb-1.5">
          <div className="grid grid-cols-2 rounded-[8px] border border-border bg-[var(--surface)] p-[2px] shadow-[inset_0_1px_2px_rgba(0,0,0,0.15)]">
            {[false, true].map((classic) => {
              const active = p.classicGame === classic;
              return (
                <button
                  key={String(classic)}
                  onClick={() => p.onClassicGame(classic)}
                  className={`flex items-center justify-center gap-1 rounded-[6px] py-[3px] text-[11px] font-bold transition-all ${
                    active
                      ? "bg-gradient-to-br from-[#c46a00] to-accent text-white shadow-[0_1px_8px_var(--accent-glow)]"
                      : "text-text-dim hover:text-[var(--text)]"
                  }`}
                >
                  <span className={`text-[12px] ${active ? "" : "opacity-60 grayscale"}`}>
                    {classic ? "🍁" : "🍄"}
                  </span>
                  {t(classic ? "launcher.game_classic" : "launcher.game_regular")}
                </button>
              );
            })}
          </div>
          {p.showClassic && !p.ngmReady && (
            <div className="mt-1 flex items-center justify-center text-[10px]">
              {p.classicCheck === null ? (
                <span className="text-text-faint">{t("login.classic_checking")}</span>
              ) : (
                <button
                  onClick={() =>
                    commands
                      .openExternal("https://platform.nexon.com/NGM/Bin/Install_NGM.exe")
                      .catch(() => {})
                  }
                  className="rounded-md border border-[var(--danger)] bg-[rgba(239,68,68,0.1)] px-2 py-0.5 font-semibold text-[var(--danger)] hover:opacity-90"
                >
                  ⚠️ {t("login.classic_ngm_missing_short")} · {t("login.classic_download")}
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {/* Accounts (auto-input toggle lives in its header line) */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto border-t border-border px-2.5 pt-1.5 pb-1">
        <AccountGrid
          compact
          selectedAccountId={p.selectedAccountId}
          onSelectAccount={p.onSelectAccount}
          headerExtra={
            <label
              className="flex cursor-pointer items-center gap-1 text-[10px] text-text-faint"
              title={t("launcher.auto_input")}
            >
              {t("launcher.auto_input")}
              <button
                type="button"
                role="switch"
                aria-checked={otp.autoInput}
                onClick={() => otp.setAutoInput(!otp.autoInput)}
                className={`relative h-[14px] w-[26px] shrink-0 rounded-full transition-colors ${
                  otp.autoInput ? "bg-[rgba(232,162,58,0.35)]" : "bg-[var(--surface-hover)]"
                }`}
              >
                <span
                  className={`absolute top-[2px] h-[10px] w-[10px] rounded-full transition-all ${
                    otp.autoInput ? "left-[14px] bg-accent" : "left-[2px] bg-text-dim"
                  }`}
                />
              </button>
            </label>
          }
        />
      </div>

      {/* One row: OTP readout · fetch · Play */}
      <div className="flex shrink-0 items-center gap-1.5 border-t border-border px-2.5 py-2">
        <button
          type="button"
          onClick={otp.copyOtp}
          disabled={!otp.credentials}
          title={t("launcher.otp")}
          className={`relative flex h-8 min-w-0 flex-1 items-center justify-center rounded-[8px] border pr-6 pl-2 font-mono text-[14px] font-bold tracking-[2px] transition-all ${
            otp.copied
              ? "border-[rgba(74,222,128,0.4)] bg-[rgba(74,222,128,0.04)] text-green-400"
              : otp.credentials
                ? "border-[rgba(232,162,58,0.15)] bg-[rgba(232,162,58,0.05)] text-accent hover:bg-[rgba(232,162,58,0.09)]"
                : "cursor-default border-border bg-[var(--surface)] text-text-faint"
          }`}
        >
          <span className="truncate">{otp.credentials?.otp ?? "••••••••"}</span>
          <span
            className={`absolute top-1/2 right-2 -translate-y-1/2 ${otp.copied ? "text-green-400" : "text-text-faint"}`}
          >
            {otp.copied ? (
              <svg
                width="11"
                height="11"
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
                width="11"
                height="11"
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
          onClick={otp.getOtp}
          disabled={!p.selectedAccountId || otp.busy}
          title={t("launcher.get_otp")}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] border border-[rgba(232,162,58,0.25)] bg-[rgba(232,162,58,0.1)] text-[14px] text-accent transition-all hover:bg-[rgba(232,162,58,0.18)] active:scale-[0.92] disabled:cursor-not-allowed disabled:opacity-40"
        >
          ↻
        </button>
        <button
          onClick={p.onPlay}
          disabled={p.launching}
          className="relative flex h-8 shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-[8px] bg-gradient-to-br from-[#c46a00] to-accent px-3 text-[11.5px] font-extrabold tracking-[1.5px] text-white shadow-[0_2px_10px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_16px_var(--accent-glow)] active:scale-[0.96] disabled:transform-none disabled:opacity-40"
        >
          <span className="pointer-events-none absolute inset-0 bg-gradient-to-b from-white/15 to-transparent" />
          <span className="relative text-[11px]">{p.showClassic ? "🍁" : "▶"}</span>
          <span className="relative">{p.launching ? "…" : t("launcher.play")}</span>
        </button>
      </div>
      {running && (
        <div className="-mt-1 flex shrink-0 items-center justify-center gap-1.5 pb-1 text-[10px] text-accent">
          <span className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_6px_var(--accent-glow)]" />
          {t("launcher.running")}
          {p.gamePid !== null ? ` · PID ${p.gamePid}` : ""}
        </div>
      )}

      <StatusBar />
    </div>
  );
}
