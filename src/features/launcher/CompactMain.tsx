import { useState } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { AccountGrid } from "./AccountGrid";
import { OtpPanel } from "./OtpPanel";
import { BeansPopupMenu, MorePopupMenu } from "./PopupMenus";
import { StatusBar } from "../shared/StatusBar";
import type { SessionDto, GameAccountDto, ClassicCheckDto } from "../../lib/types";

/**
 * The compact launcher: one narrow column (header · accounts · OTP + Play ·
 * status) so the window can sit beside the game. Pure layout — every handler
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
  const sessionRegion = p.session?.region ?? p.region;

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header: who's signed in · classic switch · beans · more */}
      <div className="flex shrink-0 items-center gap-1.5 border-b border-border px-2.5 py-1.5">
        <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[11px] font-bold text-white">
          {p.session?.accountName?.charAt(0)?.toUpperCase() ?? "?"}
        </div>
        <span className={`min-w-0 flex-1 truncate text-[12px] text-text-dim ${p.nameMask}`}>
          {p.session?.accountName ?? ""}
        </span>

        {p.canClassic && (
          <button
            onClick={() => p.onClassicGame(!p.classicGame)}
            title={t(p.classicGame ? "launcher.game_classic" : "launcher.game_regular")}
            className={`relative flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-[13px] transition-all hover:bg-[var(--surface-hover)] active:scale-[0.92] ${
              p.classicGame ? "text-accent" : "opacity-50 grayscale hover:opacity-100"
            }`}
          >
            🍁
            {p.classicGame && (
              <span className="absolute bottom-0.5 left-1/2 h-0.5 w-2.5 -translate-x-1/2 rounded-sm bg-accent" />
            )}
          </button>
        )}

        <div className="relative">
          <span
            onClick={() => setBeansMenuOpen(!beansMenuOpen)}
            className="inline-flex cursor-pointer items-center gap-1 rounded-md border border-[rgba(232,162,58,0.15)] bg-[rgba(232,162,58,0.08)] px-1.5 py-0.5 text-[11px] whitespace-nowrap transition-all hover:bg-[rgba(232,162,58,0.14)]"
            title={`${t("launcher.beans")}: ${p.remainPoint}${
              sessionRegion === "HK" && p.remainPoint > 0
                ? ` · ${t("launcher.game_points")}: ${Math.floor(p.remainPoint / 2.5)}`
                : ""
            }`}
          >
            <span className="font-semibold text-accent">
              {t("launcher.beans")} <b>{p.remainPoint}</b>
            </span>
          </span>
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
            className="flex h-6 w-6 items-center justify-center rounded-md text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
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

      {/* Classic readiness — one slim line, only while Classic is selected */}
      {p.showClassic && (
        <div className="flex shrink-0 items-center justify-center border-b border-border bg-[rgba(232,162,58,0.04)] px-2 py-1 text-[10px]">
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
              className="font-semibold text-[var(--danger)] hover:underline"
            >
              ⚠️ {t("login.classic_ngm_missing_short")} · {t("login.classic_download")}
            </button>
          )}
        </div>
      )}

      {/* Accounts */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pt-2 pb-1.5">
        <AccountGrid
          compact
          selectedAccountId={p.selectedAccountId}
          onSelectAccount={p.onSelectAccount}
        />
      </div>

      {/* OTP + Play on one row */}
      <OtpPanel
        compact
        selectedAccountId={p.selectedAccountId}
        onOtpFetched={p.onOtpFetched}
        actions={
          <button
            onClick={p.onPlay}
            disabled={p.launching}
            title={p.showClassic ? t("launcher.game_classic") : t("launcher.play")}
            className="flex h-9 shrink-0 items-center justify-center gap-1 rounded-[10px] bg-gradient-to-br from-[#c46a00] to-accent px-3 text-[11px] font-extrabold tracking-[1.5px] text-white uppercase shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_18px_var(--accent-glow)] active:scale-[0.94] disabled:transform-none disabled:opacity-40"
          >
            {p.showClassic && <span className="text-[12px]">🍁</span>}
            {p.launching ? "…" : t("launcher.play")}
          </button>
        }
      />

      {/* Footer: running state · connectivity */}
      {!p.showClassic && (p.gameRunning || p.gamePid !== null) && (
        <div className="shrink-0 text-center text-[10px] text-accent">
          ● {t("launcher.running")}
          {p.gamePid !== null ? ` (PID: ${p.gamePid})` : ""}
        </div>
      )}
      <StatusBar />
    </div>
  );
}
