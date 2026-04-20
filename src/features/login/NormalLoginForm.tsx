import { useState, useEffect, useRef, type FormEvent } from "react";
import { useLogin } from "../../lib/hooks/use-auth";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { commands } from "../../lib/tauri";
import type { SavedAccountDto } from "../../lib/types";

const FORGOT_PWD_URLS: Record<string, string> = {
  TW: "https://tw.beanfun.com/member/forgot_pwd.aspx",
  HK: "https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx",
};

interface NormalLoginFormProps {
  onShowQr: () => void;
  onTotpRequired: () => void;
  onAdvanceCheck: (url?: string) => void;
  onGamePass: () => void;
}

export function NormalLoginForm({
  onShowQr,
  onTotpRequired,
  onAdvanceCheck,
  onGamePass,
}: NormalLoginFormProps) {
  const { t } = useTranslation();
  const login = useLogin();
  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(true);
  const [savedAccounts, setSavedAccounts] = useState<SavedAccountDto[]>([]);
  const [showDropdown, setShowDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const isLoading = login.isPending;
  const region = useConfigStore((s) => s.config?.region ?? "HK");
  const showQr = region === "TW";

  // Auto-fill from last saved account on mount and when region changes.
  const prevRegionRef = useRef(region);
  useEffect(() => {
    const regionChanged = prevRegionRef.current !== region;
    prevRegionRef.current = region;

    // Clear fields when switching region so stale cross-region data is never shown
    if (regionChanged) {
      setAccount("");
      setPassword("");
      setRemember(true);
      setSavedAccounts([]);
    }

    let cancelled = false;
    async function loadSaved() {
      try {
        const [accounts, last] = await Promise.all([
          commands.getSavedAccounts(),
          commands.getLastSavedAccount(),
        ]);
        if (cancelled) return;
        setSavedAccounts(accounts);
        if (last) {
          setAccount(last.account);
          if (last.password) {
            setPassword(last.password);
          }
          setRemember(last.rememberPassword);
        }
      } catch {
        /* non-critical */
      }
    }
    loadSaved();
    return () => {
      cancelled = true;
    };
  }, [region]);

  // Close dropdown when clicking outside.
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  function handleSelectAccount(saved: SavedAccountDto) {
    setAccount(saved.account);
    setRemember(saved.rememberPassword);
    // Fetch the specific account's saved password from the backend.
    if (saved.hasPassword) {
      commands
        .getSavedAccountDetail(saved.account)
        .then((detail) => {
          if (detail && detail.password) {
            setPassword(detail.password);
            setRemember(detail.rememberPassword);
          } else {
            setPassword("");
          }
        })
        .catch(() => {
          setPassword("");
        });
    } else {
      setPassword("");
    }
    setShowDropdown(false);
  }

  async function handleDeleteAccount(acct: SavedAccountDto) {
    try {
      await commands.deleteSavedAccount(acct.account, acct.region);
      setSavedAccounts((prev) => prev.filter((a) => a.account !== acct.account));
      if (account === acct.account) {
        setAccount("");
        setPassword("");
      }
    } catch {
      /* non-critical */
    }
  }

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!account.trim() || !password.trim()) return;
    login.mutate(
      { account: account.trim(), password, rememberPassword: remember },
      {
        onError: (err) => {
          if (err.message === "TOTP_REQUIRED" || err.name === "TotpRequired") {
            onTotpRequired();
          } else if (err.message === "ADVANCE_CHECK" || err.name === "AdvanceCheck") {
            onAdvanceCheck((err as { advanceUrl?: string }).advanceUrl);
          }
        },
      },
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex w-full flex-col">
      {/* Account field with dropdown */}
      <div className="mb-3">
        <label
          htmlFor="login-account"
          className="mb-1 block text-[12px] tracking-[1.5px] text-text-dim uppercase"
        >
          {t("login.username")}
        </label>
        <div className="relative" ref={dropdownRef}>
          <input
            id="login-account"
            type="text"
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            disabled={isLoading}
            placeholder={t("login.username_placeholder")}
            autoComplete="off"
            autoCorrect="off"
            data-form-type="other"
            spellCheck={false}
            className="w-full rounded-lg border border-border bg-[var(--surface)] px-3.5 py-2.5 pr-8 text-[13px] text-[var(--text)] placeholder:text-[12px] placeholder:text-text-dim focus:border-[rgba(232,162,58,0.4)] focus:bg-[var(--surface-hover)] focus:shadow-[0_0_0_3px_var(--input-focus-ring)] focus:outline-none disabled:opacity-50"
          />
          {savedAccounts.length > 0 && (
            <button
              type="button"
              onClick={() => setShowDropdown((v) => !v)}
              tabIndex={-1}
              className="absolute top-1/2 right-2 -translate-y-1/2 text-text-dim transition-colors hover:text-[var(--text)]"
              aria-label="Show saved accounts"
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path
                  d="M3 5L6 8L9 5"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </button>
          )}
          {showDropdown && savedAccounts.length > 0 && (
            <div className="absolute top-full right-0 left-0 z-50 mt-1 max-h-40 overflow-y-auto rounded-[10px] border border-border bg-[var(--bg)] py-1 shadow-[0_8px_32px_rgba(0,0,0,0.25)]">
              {savedAccounts.map((saved) => (
                <div
                  key={saved.account}
                  className="group flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)]"
                >
                  <button
                    type="button"
                    onClick={() => handleSelectAccount(saved)}
                    className="flex min-w-0 flex-1 items-center gap-2"
                  >
                    <span className="truncate">{saved.account}</span>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteAccount(saved);
                    }}
                    className="shrink-0 rounded p-0.5 text-text-faint opacity-0 transition-all group-hover:opacity-100 hover:bg-[rgba(239,68,68,0.1)] hover:text-red-400"
                    title="Delete"
                  >
                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                      <path
                        d="M3 3L9 9M9 3L3 9"
                        stroke="currentColor"
                        strokeWidth="1.5"
                        strokeLinecap="round"
                      />
                    </svg>
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Password field */}
      <div className="mb-3">
        <label
          htmlFor="login-password"
          className="mb-1 block text-[12px] tracking-[1.5px] text-text-dim uppercase"
        >
          {t("login.password")}
        </label>
        <input
          id="login-password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={isLoading}
          placeholder={t("login.password_placeholder")}
          autoComplete="new-password"
          data-form-type="other"
          className="w-full rounded-lg border border-border bg-[var(--surface)] px-3.5 py-2.5 text-[13px] text-[var(--text)] placeholder:text-[12px] placeholder:text-text-dim focus:border-[rgba(232,162,58,0.4)] focus:bg-[var(--surface-hover)] focus:shadow-[0_0_0_3px_var(--input-focus-ring)] focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Options row */}
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-text-dim transition-colors hover:text-[var(--text)]">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
            className="h-3.5 w-3.5 accent-accent"
          />
          {t("login.remember")}
        </label>
        <button
          type="button"
          onClick={() => {
            const url =
              FORGOT_PWD_URLS[region] ??
              FORGOT_PWD_URLS.HK ??
              "https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx";
            commands.openWebPopup(url, String(t("login.forgot"))).catch(() => {});
          }}
          className="ml-auto text-[12px] text-accent transition-opacity hover:opacity-70"
        >
          {t("login.forgot")}
        </button>
      </div>

      {login.error && (
        <p className="mb-2 text-[12px] text-[var(--danger)]">{login.error.message}</p>
      )}

      {/* Actions row: Sign In + QR button (QR only for TW) */}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={isLoading || !account.trim() || !password.trim()}
          className="flex-1 rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-5 py-2.5 text-[12px] font-semibold tracking-[1.5px] text-white uppercase shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_20px_var(--accent-glow)] active:scale-95 disabled:transform-none disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isLoading ? t("login.logging_in") : t("login.submit")}
        </button>
        {showQr && (
          <button
            type="button"
            onClick={onShowQr}
            title="QR Code"
            className="flex h-[42px] w-[42px] shrink-0 items-center justify-center rounded-lg border border-border bg-[var(--surface)] text-text-dim transition-all hover:border-accent hover:bg-[var(--surface-hover)] hover:text-accent active:scale-95"
          >
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
              <rect
                x="1"
                y="1"
                width="6"
                height="6"
                rx="1"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <rect x="3" y="3" width="2" height="2" fill="currentColor" />
              <rect
                x="11"
                y="1"
                width="6"
                height="6"
                rx="1"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <rect x="13" y="3" width="2" height="2" fill="currentColor" />
              <rect
                x="1"
                y="11"
                width="6"
                height="6"
                rx="1"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <rect x="3" y="13" width="2" height="2" fill="currentColor" />
              <rect x="11" y="11" width="2" height="2" fill="currentColor" />
              <rect x="15" y="11" width="2" height="2" fill="currentColor" />
              <rect x="11" y="15" width="2" height="2" fill="currentColor" />
              <rect x="15" y="15" width="2" height="2" fill="currentColor" />
              <rect x="13" y="13" width="2" height="2" fill="currentColor" />
            </svg>
          </button>
        )}
        {showQr && (
          <button
            type="button"
            onClick={onGamePass}
            disabled={isLoading}
            title={t("login.gamepass")}
            className="flex h-[42px] w-[42px] shrink-0 items-center justify-center rounded-lg border border-border bg-[var(--surface)] text-text-dim transition-all hover:border-accent hover:bg-[var(--surface-hover)] hover:text-accent active:scale-95 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
              <path
                d="M9 1.5L2 5.5V12.5L9 16.5L16 12.5V5.5L9 1.5Z"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinejoin="round"
              />
              <path d="M9 8.5V16.5" stroke="currentColor" strokeWidth="1.5" />
              <path d="M2 5.5L9 9.5L16 5.5" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </button>
        )}
      </div>
    </form>
  );
}
