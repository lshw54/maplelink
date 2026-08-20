import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { useOtp } from "../../lib/hooks/use-otp";
import { OtpMoreMenu } from "./PopupMenus";

interface OtpPanelProps {
  selectedAccountId: string | null;
  onOtpFetched?: (accountId: string, otp: string) => void;
}

export function OtpPanel({ selectedAccountId, onOtpFetched }: OtpPanelProps) {
  const { t } = useTranslation();
  const { credentials, copied, autoInput, setAutoInput, busy, getOtp, copyOtp, copyCredentials } =
    useOtp(selectedAccountId, onOtpFetched);
  const [moreOpen, setMoreOpen] = useState(false);

  return (
    <div className="mx-3 mb-3 shrink-0 rounded-xl border border-border bg-[var(--surface)] p-3.5 shadow-[0_-4px_20px_rgba(0,0,0,0.1),0_0_0_1px_var(--border)] backdrop-blur-sm">
      {/* Header */}
      <div className="mb-2.5 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-bold tracking-[2.5px] text-text-dim uppercase">
            🔐 {t("launcher.otp")}
          </span>
        </div>
        <label className="flex cursor-pointer items-center gap-1.5">
          <span className="text-[12px] tracking-[0.5px] text-text-faint">
            {t("launcher.auto_input")}
          </span>
          <button
            type="button"
            onClick={() => setAutoInput(!autoInput)}
            className={`relative h-[18px] w-8 shrink-0 rounded-[9px] transition-colors ${
              autoInput ? "bg-[rgba(232,162,58,0.3)]" : "bg-[var(--surface-hover)]"
            }`}
          >
            <span
              className={`absolute top-0.5 h-3.5 w-3.5 rounded-full transition-all ${
                autoInput ? "left-4 bg-accent" : "left-0.5 bg-text-dim"
              }`}
            />
          </button>
        </label>
      </div>

      {/* OTP display row */}
      <div className="flex items-center gap-2.5">
        <button
          type="button"
          onClick={copyOtp}
          disabled={!credentials}
          className={`relative flex flex-1 items-center justify-center rounded-[10px] border px-4 py-2.5 font-mono text-[22px] font-bold tracking-[4px] transition-all ${
            copied
              ? "border-[rgba(74,222,128,0.4)] bg-[rgba(74,222,128,0.04)] text-green-400"
              : credentials
                ? "border-[rgba(232,162,58,0.08)] bg-[rgba(232,162,58,0.04)] text-accent shadow-[0_0_20px_rgba(232,162,58,0.06)_inset,0_2px_8px_rgba(0,0,0,0.3)_inset] hover:border-[rgba(232,162,58,0.2)] hover:bg-[rgba(232,162,58,0.06)]"
                : "cursor-default border-[rgba(232,162,58,0.08)] bg-[rgba(232,162,58,0.04)] text-text-faint"
          }`}
        >
          {credentials?.otp ?? "••••••••••"}
          {/* Copy / Check icon — always visible */}
          <span
            className={`absolute top-1/2 right-2.5 -translate-y-1/2 transition-colors ${
              copied ? "text-green-400" : "text-text-faint"
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
          </span>
        </button>

        {/* Split button: fetch, and a ▾ with copy one-time credentials */}
        <div className="relative flex shrink-0 shadow-[0_2px_10px_var(--accent-glow)] transition-shadow hover:shadow-[0_4px_16px_var(--accent-glow)]">
          <button
            onClick={getOtp}
            disabled={!selectedAccountId || busy}
            title={t("launcher.get_otp")}
            className="flex h-10 w-10 items-center justify-center rounded-l-[10px] bg-gradient-to-br from-accent to-[#c47a1a] text-base text-white transition-all active:scale-[0.95] disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↻
          </button>
          <button
            onClick={() => setMoreOpen(!moreOpen)}
            disabled={!selectedAccountId}
            aria-label="More"
            className="flex h-10 w-5 items-center justify-center rounded-r-[10px] border-l border-white/25 bg-gradient-to-br from-accent to-[#c47a1a] text-white transition-all hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <svg
              width="9"
              height="9"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
            >
              <path d="M2 4l3 3 3-3" />
            </svg>
          </button>
          {moreOpen && (
            <OtpMoreMenu
              onClose={() => setMoreOpen(false)}
              items={[
                {
                  icon: "🔑",
                  label: t("launcher.context.copy_credentials"),
                  onClick: () => void copyCredentials(),
                  disabled: busy,
                },
              ]}
            />
          )}
        </div>
      </div>
    </div>
  );
}
