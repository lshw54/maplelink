import { useState } from "react";
import { commands } from "../../lib/tauri";
import { useTranslation } from "../../lib/i18n";
import { useOtp } from "../../lib/hooks/use-otp";
import { AccountGrid } from "./AccountGrid";
import { BeansPopupMenu, MorePopupMenu, OtpMoreMenu } from "./PopupMenus";
import { StatusBar } from "../shared/StatusBar";
import type { SessionDto, GameAccountDto, ClassicCheckDto } from "../../lib/types";

/**
 * The compact launcher, laid out like the old beanfun one: an action line
 * (beans · Play · more), the regular/classic switch, the account list, one
 * OTP row, status. Flat by design — few borders, small type. Pure layout:
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
  const [otpMenuOpen, setOtpMenuOpen] = useState(false);
  const otp = useOtp(p.selectedAccountId, p.onOtpFetched);
  const sessionRegion = p.session?.region ?? p.region;
  const running = !p.showClassic && (p.gameRunning || p.gamePid !== null);
  const gamePoints =
    sessionRegion === "HK" && p.remainPoint > 0 ? Math.floor(p.remainPoint / 2.5) : null;

  return (
    <div className="relative flex flex-1 flex-col overflow-hidden">
      {/* Line 1: beans · running · Play · more. The session strip above
          already names the account, so there is no identity line. */}
      <div className="flex shrink-0 items-center gap-2 px-3 pt-2 pb-1.5">
        <div className="relative min-w-0 flex-1">
          <button
            onClick={() => setBeansMenuOpen(!beansMenuOpen)}
            className="group flex max-w-full items-center gap-1 rounded-md py-1 pr-1.5 pl-1 text-[11.5px] whitespace-nowrap text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
            title={`${t("launcher.beans")}: ${p.remainPoint}${
              gamePoints !== null ? ` · ${t("launcher.game_points")}: ${gamePoints}` : ""
            }`}
          >
            <span className="truncate">
              {t("launcher.beans")} <b className="text-accent">{p.remainPoint}</b>
              {gamePoints !== null && (
                <span className="text-text-faint">
                  {" · "}
                  {t("launcher.game_points")} <b className="text-text-dim">{gamePoints}</b>
                </span>
              )}
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

        {running && (
          <span
            className="flex shrink-0 items-center gap-1 text-[10px] text-accent"
            title={`PID ${p.gamePid ?? ""}`}
          >
            <span className="h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_6px_var(--accent-glow)]" />
            {t("launcher.running")}
          </span>
        )}

        <button
          onClick={p.onPlay}
          disabled={p.launching}
          title={p.showClassic ? t("launcher.game_classic_title") : t("launcher.play")}
          className="relative flex h-[26px] shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-[7px] bg-gradient-to-br from-[#c46a00] to-accent px-3 text-[11px] font-extrabold tracking-[1px] text-white shadow-[0_2px_8px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_3px_12px_var(--accent-glow)] active:scale-[0.96] disabled:transform-none disabled:opacity-40"
        >
          <span className="pointer-events-none absolute inset-0 bg-gradient-to-b from-white/15 to-transparent" />
          <span className="relative text-[10px]">{p.showClassic ? "🍁" : "▶"}</span>
          <span className="relative">{p.launching ? "…" : t("launcher.play")}</span>
        </button>

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

      {/* Line 2 (HK only): which game Play opens — a full-width switch */}
      {p.canClassic && (
        <div className="shrink-0 px-3 pb-2">
          <div className="grid grid-cols-2 rounded-[7px] border border-border bg-[var(--surface)] p-[2px]">
            {[false, true].map((classic) => {
              const active = p.classicGame === classic;
              return (
                <button
                  key={String(classic)}
                  onClick={() => p.onClassicGame(classic)}
                  className={`flex h-[22px] items-center justify-center gap-1.5 rounded-[5px] text-[11px] font-bold transition-all ${
                    active
                      ? "bg-gradient-to-br from-[#c46a00] to-accent text-white shadow-[0_1px_6px_var(--accent-glow)]"
                      : "text-text-dim hover:text-[var(--text)]"
                  }`}
                >
                  <span className={`text-[11px] ${active ? "" : "opacity-50 grayscale"}`}>
                    {classic ? "🍁" : "🍄"}
                  </span>
                  {t(classic ? "launcher.game_classic" : "launcher.game_regular")}
                </button>
              );
            })}
          </div>
        </div>
      )}

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

      {/* Accounts */}
      <div className="scroll-quiet flex min-h-0 flex-1 flex-col overflow-y-auto border-t border-border px-2 pt-1.5 pb-1">
        <AccountGrid
          compact
          selectedAccountId={p.selectedAccountId}
          onSelectAccount={p.onSelectAccount}
        />
      </div>

      {/* OTP row: readout (copy) · Get OTP with a ▾ for the rarer copy-credentials · auto-input */}
      <div className="flex shrink-0 items-center gap-2 border-t border-border px-3 py-2">
        <button
          type="button"
          onClick={otp.copyOtp}
          disabled={!otp.credentials}
          title={t("launcher.context.copy_otp")}
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

        {/* Split button: Get OTP, and a ▾ with the less common actions */}
        <div className="relative flex shrink-0">
          <button
            onClick={otp.getOtp}
            disabled={!p.selectedAccountId || otp.busy}
            className="flex h-7 items-center gap-1 rounded-l-[7px] bg-[rgba(232,162,58,0.14)] px-2 text-[11px] font-semibold text-accent transition-all hover:bg-[rgba(232,162,58,0.22)] active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↻ {t("launcher.get_otp")}
          </button>
          <button
            onClick={() => setOtpMenuOpen(!otpMenuOpen)}
            disabled={!p.selectedAccountId}
            aria-label="More"
            className="flex h-7 w-5 items-center justify-center rounded-r-[7px] border-l border-[rgba(232,162,58,0.25)] bg-[rgba(232,162,58,0.14)] text-accent transition-all hover:bg-[rgba(232,162,58,0.22)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            <svg
              width="8"
              height="8"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
            >
              <path d="M2 4l3 3 3-3" />
            </svg>
          </button>
          {otpMenuOpen && (
            <OtpMoreMenu
              onClose={() => setOtpMenuOpen(false)}
              items={[
                {
                  icon: "🔑",
                  label: t("launcher.context.copy_credentials"),
                  onClick: () => void otp.copyCredentials(),
                  disabled: otp.busy,
                },
              ]}
            />
          )}
        </div>

        <label
          className="flex shrink-0 cursor-pointer items-center gap-1.5 text-[10.5px] text-text-faint"
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
      </div>

      <StatusBar />
    </div>
  );
}
