import { useState } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { useOtp } from "../../lib/hooks/use-otp";
import { AccountGrid } from "./AccountGrid";
import { BeansPopupMenu, MorePopupMenu } from "./PopupMenus";
import { StatusBar } from "../shared/StatusBar";
import type { SessionDto, GameAccountDto, ClassicCheckDto } from "../../lib/types";

/**
 * The compact launcher, laid out like the old beanfun one: identity line ·
 * an action line (beans · regular/classic · Play) · the account list · one
 * OTP row · status. Flat by design — few borders, small type. Pure layout:
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
  const otp = useOtp(p.selectedAccountId, p.onOtpFetched);
  const sessionRegion = p.session?.region ?? p.region;
  const running = !p.showClassic && (p.gameRunning || p.gamePid !== null);
  const gamePoints =
    sessionRegion === "HK" && p.remainPoint > 0 ? Math.floor(p.remainPoint / 2.5) : null;

  return (
    <div className="relative flex flex-1 flex-col overflow-hidden">
      {/* Identity line */}
      <div className="flex shrink-0 items-center gap-2 px-3 pt-2 pb-1">
        <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[10px] font-bold text-white">
          {p.session?.accountName?.charAt(0)?.toUpperCase() ?? "?"}
        </div>
        <span className={`min-w-0 flex-1 truncate text-[12px] font-semibold ${p.nameMask}`}>
          {p.session?.accountName ?? ""}
        </span>
        {running && (
          <span
            className="flex items-center gap-1 text-[10px] text-accent"
            title={`PID ${p.gamePid ?? ""}`}
          >
            <span className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_6px_var(--accent-glow)]" />
            {t("launcher.running")}
          </span>
        )}
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

      {/* Action line: beans · which game · Play */}
      <div className="flex shrink-0 items-center gap-1.5 px-3 pb-2">
        <div className="relative min-w-0 flex-1">
          <button
            onClick={() => setBeansMenuOpen(!beansMenuOpen)}
            className="group flex max-w-full items-center gap-1 rounded-md py-1 pr-1.5 pl-1 text-[11px] whitespace-nowrap text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
            title={`${t("launcher.beans")}: ${p.remainPoint}${
              gamePoints !== null ? ` · ${t("launcher.game_points")}: ${gamePoints}` : ""
            }`}
          >
            <span className="truncate">
              {t("launcher.beans")} <b className="text-accent">{p.remainPoint}</b>
            </span>
            <svg
              width="8"
              height="8"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              className="shrink-0 text-text-faint group-hover:text-text-dim"
            >
              <path d="M2 4l3 3 3-3" />
            </svg>
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
              alignLeft
            />
          )}
        </div>

        {/* Regular ↔ Classic, right where Play is — the two decide together
            what gets launched. */}
        {p.canClassic && (
          <div className="flex shrink-0 rounded-[7px] border border-border bg-[var(--surface)] p-[2px]">
            {[false, true].map((classic) => {
              const active = p.classicGame === classic;
              return (
                <button
                  key={String(classic)}
                  onClick={() => p.onClassicGame(classic)}
                  title={t(classic ? "launcher.game_classic" : "launcher.game_regular")}
                  className={`flex h-[22px] items-center gap-1 rounded-[5px] px-2 text-[10.5px] font-bold transition-all ${
                    active
                      ? "bg-gradient-to-br from-[#c46a00] to-accent text-white shadow-[0_1px_6px_var(--accent-glow)]"
                      : "text-text-dim hover:text-[var(--text)]"
                  }`}
                >
                  <span className={`text-[11px] ${active ? "" : "opacity-50 grayscale"}`}>
                    {classic ? "🍁" : "🍄"}
                  </span>
                  {t(classic ? "launcher.game_classic_short" : "launcher.game_regular_short")}
                </button>
              );
            })}
          </div>
        )}

        <button
          onClick={p.onPlay}
          disabled={p.launching}
          title={p.showClassic ? t("launcher.game_classic_title") : t("launcher.play")}
          className="relative flex h-[26px] shrink-0 items-center justify-center gap-1 overflow-hidden rounded-[7px] bg-gradient-to-br from-[#c46a00] to-accent px-2.5 text-[11px] font-extrabold tracking-[1px] text-white shadow-[0_2px_8px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_3px_12px_var(--accent-glow)] active:scale-[0.96] disabled:transform-none disabled:opacity-40"
        >
          <span className="pointer-events-none absolute inset-0 bg-gradient-to-b from-white/15 to-transparent" />
          <span className="relative text-[10px]">{p.showClassic ? "🍁" : "▶"}</span>
          <span className="relative">{p.launching ? "…" : t("launcher.play")}</span>
        </button>
      </div>

      {/* Classic needs attention (NGM missing / still checking) */}
      {p.showClassic && !p.ngmReady && (
        <div className="flex shrink-0 items-center justify-center px-3 pb-1.5 text-[10px]">
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

      {/* Accounts (auto-input toggle lives in the header line) */}
      <div className="scroll-quiet flex min-h-0 flex-1 flex-col overflow-y-auto border-t border-border px-2 pt-1.5 pb-1">
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

      {/* OTP row */}
      <div className="flex shrink-0 items-center gap-1.5 border-t border-border px-3 py-1.5">
        <span className="shrink-0 text-[10px] font-semibold tracking-[1px] text-text-faint uppercase">
          OTP
        </span>
        <button
          type="button"
          onClick={otp.copyOtp}
          disabled={!otp.credentials}
          title={t("launcher.otp")}
          className={`relative flex h-7 min-w-0 flex-1 items-center justify-center rounded-[7px] pr-6 pl-2 font-mono text-[13px] font-bold tracking-[2px] transition-all ${
            otp.copied
              ? "bg-[rgba(74,222,128,0.08)] text-green-400"
              : otp.credentials
                ? "bg-[rgba(232,162,58,0.08)] text-accent hover:bg-[rgba(232,162,58,0.13)]"
                : "cursor-default bg-[var(--surface)] text-text-faint"
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
          className="flex h-7 shrink-0 items-center justify-center gap-1 rounded-[7px] bg-[rgba(232,162,58,0.12)] px-2 text-[11px] font-semibold text-accent transition-all hover:bg-[rgba(232,162,58,0.2)] active:scale-[0.95] disabled:cursor-not-allowed disabled:opacity-40"
        >
          ↻ {t("launcher.get_otp")}
        </button>
      </div>

      <StatusBar />
    </div>
  );
}
