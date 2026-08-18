import { useState } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { AccountGrid } from "./AccountGrid";
import { OtpPanel } from "./OtpPanel";
import { BeansPopupMenu, MorePopupMenu } from "./PopupMenus";
import { StatusBar } from "../shared/StatusBar";
import type { SessionDto, GameAccountDto, ClassicCheckDto } from "../../lib/types";

/**
 * The compact launcher: one narrow column — header · game switch · accounts ·
 * OTP · Play · status — so the window can sit beside the game. Pure layout:
 * every handler and piece of state comes from MainPage, which owns the launch
 * logic and the confirmation modals for both layouts.
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
  const sessionRegion = p.session?.region ?? p.region;
  const running = !p.showClassic && (p.gameRunning || p.gamePid !== null);

  return (
    <div className="relative flex flex-1 flex-col overflow-hidden">
      {/* Soft accent wash behind the header so the column has some depth */}
      <div className="pointer-events-none absolute inset-x-0 top-0 h-24 bg-gradient-to-b from-[rgba(232,162,58,0.07)] to-transparent" />

      {/* Header: who's signed in · beans · more */}
      <div className="relative flex shrink-0 items-center gap-2 px-3 pt-2.5 pb-2">
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[12px] font-bold text-white shadow-[0_2px_8px_var(--accent-glow)]">
          {p.session?.accountName?.charAt(0)?.toUpperCase() ?? "?"}
        </div>
        <div className="flex min-w-0 flex-1 flex-col leading-tight">
          <span className={`truncate text-[12px] font-semibold text-[var(--text)] ${p.nameMask}`}>
            {p.session?.accountName ?? ""}
          </span>
          <span className="text-[9.5px] font-medium tracking-[1.5px] text-text-faint uppercase">
            beanfun · {sessionRegion}
          </span>
        </div>

        <div className="relative">
          <button
            onClick={() => setBeansMenuOpen(!beansMenuOpen)}
            className="inline-flex items-center gap-1 rounded-full border border-[rgba(232,162,58,0.18)] bg-[rgba(232,162,58,0.08)] px-2.5 py-1 text-[11px] whitespace-nowrap transition-all hover:bg-[rgba(232,162,58,0.16)]"
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
            className="flex h-7 w-7 items-center justify-center rounded-full text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
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

      {/* Regular ↔ Classic — a full-width segmented switch so the choice is
          unmistakable (it decides what Play launches). */}
      {p.canClassic && (
        <div className="relative shrink-0 px-3 pb-2">
          <div className="grid grid-cols-2 rounded-[10px] border border-border bg-[var(--surface)] p-[3px] shadow-[inset_0_1px_2px_rgba(0,0,0,0.15)]">
            {[false, true].map((classic) => {
              const active = p.classicGame === classic;
              return (
                <button
                  key={String(classic)}
                  onClick={() => p.onClassicGame(classic)}
                  className={`flex items-center justify-center gap-1.5 rounded-[8px] py-1.5 text-[11.5px] font-bold tracking-[0.5px] transition-all ${
                    active
                      ? "bg-gradient-to-br from-[#c46a00] to-accent text-white shadow-[0_2px_10px_var(--accent-glow)]"
                      : "text-text-dim hover:text-[var(--text)]"
                  }`}
                >
                  <span className={active ? "" : "opacity-60 grayscale"}>
                    {classic ? "🍁" : "🍄"}
                  </span>
                  {t(classic ? "launcher.game_classic" : "launcher.game_regular")}
                </button>
              );
            })}
          </div>
          {/* Classic readiness — one slim line, only while Classic is selected */}
          {p.showClassic && (
            <div className="mt-1.5 flex items-center justify-center text-[10px]">
              {p.classicCheck === null ? (
                <span className="text-text-faint">{t("login.classic_checking")}</span>
              ) : p.ngmReady ? (
                <span className="text-green-500">✓ {t("login.classic_ready")}</span>
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

      {/* Accounts */}
      <div className="relative flex min-h-0 flex-1 flex-col overflow-y-auto border-t border-border px-3 pt-2 pb-1.5">
        <AccountGrid
          compact
          selectedAccountId={p.selectedAccountId}
          onSelectAccount={p.onSelectAccount}
        />
      </div>

      {/* OTP card, then the hero Play bar */}
      <div className="shrink-0 px-3 pb-2">
        <OtpPanel compact selectedAccountId={p.selectedAccountId} onOtpFetched={p.onOtpFetched} />
        <button
          onClick={p.onPlay}
          disabled={p.launching}
          className="group relative mt-2 flex h-10 w-full items-center justify-center gap-2 overflow-hidden rounded-[12px] bg-gradient-to-br from-[#c46a00] to-accent text-[12px] font-extrabold tracking-[3px] text-white uppercase shadow-[0_4px_18px_var(--accent-glow),0_0_0_3px_rgba(232,162,58,0.08)] transition-all hover:translate-y-[-1px] hover:shadow-[0_6px_24px_rgba(232,162,58,0.5)] active:scale-[0.98] disabled:transform-none disabled:opacity-40"
        >
          {/* sheen */}
          <span className="pointer-events-none absolute inset-0 bg-gradient-to-b from-white/15 to-transparent opacity-70" />
          <span className="relative text-[13px]">{p.showClassic ? "🍁" : "▶"}</span>
          <span className="relative">{p.launching ? "…" : t("launcher.play")}</span>
          {p.showClassic && (
            <span className="relative rounded-md bg-white/20 px-1.5 py-0.5 text-[10px] tracking-[1px] normal-case">
              {t("launcher.game_classic")}
            </span>
          )}
        </button>
        {running && (
          <div className="mt-1.5 flex items-center justify-center gap-1.5 text-[10px] text-accent">
            <span className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_6px_var(--accent-glow)]" />
            {t("launcher.running")}
            {p.gamePid !== null ? ` · PID ${p.gamePid}` : ""}
          </div>
        )}
      </div>

      <StatusBar />
    </div>
  );
}
