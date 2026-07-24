import { useState, useEffect, useRef, useCallback, type SyntheticEvent } from "react";
import { useLogin } from "../../lib/hooks/use-auth";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useUiStore } from "../../lib/stores/ui-store";
import { useAuthStore } from "../../lib/stores/auth-store";
import { commands } from "../../lib/tauri";
import { PasswordInput } from "../../components/PasswordInput";
import { Modal } from "../../components/Modal";
import type { SavedAccountDto, ClassicCheckDto } from "../../lib/types";

const FORGOT_PWD_URLS: Record<string, string> = {
  TW: "https://tw.beanfun.com/member/forgot_pwd.aspx",
  HK: "https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx",
};

const NGM_DOWNLOAD_URL = "https://platform.nexon.com/NGM/Bin/Install_NGM.exe";

// Global flag — auto-login only fires once per app launch, not on re-mount.
// Uses window property to survive HMR in dev mode.
const AUTO_LOGIN_KEY = "__maplelink_auto_login_fired__";
function hasAutoLoginFired(): boolean {
  return (window as unknown as Record<string, unknown>)[AUTO_LOGIN_KEY] === true;
}
function markAutoLoginFired(): void {
  (window as unknown as Record<string, unknown>)[AUTO_LOGIN_KEY] = true;
}

interface NormalLoginFormProps {
  onShowQr: () => void;
  onTotpRequired: () => void;
  onAdvanceCheck: (url?: string) => void;
  onGamePass: () => void;
  onWebLaunch: () => void;
}

export function NormalLoginForm({
  onShowQr,
  onTotpRequired,
  onAdvanceCheck,
  onGamePass,
  onWebLaunch,
}: NormalLoginFormProps) {
  const { t } = useTranslation();
  const login = useLogin();
  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(true);
  const [savedAccounts, setSavedAccounts] = useState<SavedAccountDto[]>([]);
  const [showDropdown, setShowDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const accountInputRef = useRef<HTMLInputElement>(null);
  const [highlightIdx, setHighlightIdx] = useState(-1);
  const [cafeConfirm, setCafeConfirm] = useState(false);
  const [classicCheck, setClassicCheck] = useState<ClassicCheckDto | null>(null);
  const [showCheckDetail, setShowCheckDetail] = useState(false);

  async function runClassicCheck() {
    try {
      setClassicCheck(await commands.classicSelfCheck());
    } catch {
      /* ignore */
    }
  }

  const isLoading = login.isPending;
  const region = useConfigStore((s) => s.config?.region ?? "HK");
  const autoLogin = useConfigStore((s) => s.config?.autoLogin ?? false);
  const cafeMode = useConfigStore((s) => s.config?.cafeMode ?? false);
  const classicMode = useUiStore((s) => s.classicMode);
  const showQr = region === "TW" && !classicMode;

  // Run the classic readiness check as soon as classic mode is entered, so its
  // status shows inline without the user having to ask.
  useEffect(() => {
    if (!classicMode || classicCheck !== null) return;
    let alive = true;
    commands
      .classicSelfCheck()
      .then((c) => {
        if (alive) setClassicCheck(c);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [classicMode]);

  function setCafeMode(on: boolean) {
    commands.setConfig("cafe_mode", String(on)).catch(() => {});
    const cfg = useConfigStore.getState().config;
    if (cfg) useConfigStore.setState({ config: { ...cfg, cafeMode: on } });
  }

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

  // Auto-login: only fires once per app launch when enabled and credentials exist.
  // Does NOT fire on logout, session switch, or re-mount.
  // Delayed slightly to ensure WebView2 is fully rendered before navigating.
  useEffect(() => {
    // Never auto-login in café mode — it must not sign in a previous user.
    if (
      autoLogin &&
      !cafeMode &&
      !hasAutoLoginFired() &&
      account.trim() &&
      password.trim() &&
      !isLoading
    ) {
      markAutoLoginFired();
      const timer = setTimeout(() => {
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
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [autoLogin, cafeMode, account, password]); // eslint-disable-line react-hooks/exhaustive-deps

  // Close dropdown when clicking outside.
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
        setHighlightIdx(-1);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleSelectAccount = useCallback((saved: SavedAccountDto) => {
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
    closeDropdown();
  }, []);

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

  function closeDropdown() {
    setShowDropdown(false);
    setHighlightIdx(-1);
  }

  function handleAccountKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (savedAccounts.length === 0) return;

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      // Directly cycle through saved accounts like mouse wheel
      const currentIdx = savedAccounts.findIndex((s) => s.account === account);
      const dir = e.key === "ArrowDown" ? 1 : -1;
      let nextIdx = currentIdx + dir;
      if (nextIdx < 0) nextIdx = savedAccounts.length - 1;
      if (nextIdx >= savedAccounts.length) nextIdx = 0;
      const next = savedAccounts[nextIdx];
      if (next) handleSelectAccount(next);
    } else if (e.key === "Escape" && showDropdown) {
      e.preventDefault();
      closeDropdown();
    }
  }

  // Native wheel listener with { passive: false } so preventDefault works.
  useEffect(() => {
    const el = accountInputRef.current;
    if (!el) return;
    function onWheel(e: WheelEvent) {
      if (savedAccounts.length === 0) return;
      e.preventDefault();
      const currentIdx = savedAccounts.findIndex((s) => s.account === account);
      const dir = e.deltaY > 0 ? 1 : -1;
      let nextIdx = currentIdx + dir;
      if (nextIdx < 0) nextIdx = savedAccounts.length - 1;
      if (nextIdx >= savedAccounts.length) nextIdx = 0;
      const next = savedAccounts[nextIdx];
      if (next) handleSelectAccount(next);
    }
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [savedAccounts, account, handleSelectAccount]);

  function handleSubmit(e: SyntheticEvent) {
    e.preventDefault();
    if (!account.trim() || !password.trim()) return;

    // Classic: if this account is already logged in, reuse that session's cookies
    // to open the portal instead of logging in again. beanfun allows one session
    // per account, so a second login would kick the existing (regular) session.
    if (classicMode) {
      const acct = account.trim();
      const existing = [...useAuthStore.getState().sessions.values()].find(
        (s) => s.session.accountName === acct,
      );
      if (existing) {
        useUiStore.setState({ classicStatus: "launching" });
        commands.openClassicLogin(existing.sessionId).catch(() => {
          useUiStore.setState({ classicStatus: "failed" });
        });
        if (useAuthStore.getState().isAuthenticated) useUiStore.getState().setPage("main");
        return;
      }
    }

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
      {classicMode &&
        (classicCheck && !(classicCheck.ngmRegistered && classicCheck.ngmExeExists) ? (
          // NGM missing — prominent warning with a direct download.
          <div className="mb-3 flex items-center gap-2 rounded-lg border border-[var(--danger)] bg-[rgba(239,68,68,0.1)] px-3 py-2 text-[11px]">
            <span className="text-[13px]">⚠️</span>
            <span className="flex-1 font-semibold text-[var(--danger)]">
              {t("login.classic_ngm_missing_short")}
            </span>
            <button
              type="button"
              onClick={() => commands.openExternal(NGM_DOWNLOAD_URL).catch(() => {})}
              className="shrink-0 rounded-md bg-[var(--danger)] px-2 py-1 font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("login.classic_download")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setShowCheckDetail(true)}
            className="mb-3 flex w-full items-center gap-2 rounded-lg bg-[rgba(232,162,58,0.08)] px-3 py-2 text-left text-[11px] transition-colors hover:bg-[rgba(232,162,58,0.12)]"
          >
            {classicCheck === null ? (
              <span className="text-text-dim">{t("login.classic_checking")}</span>
            ) : (
              <>
                <span className="text-green-500">✓</span>
                <span className="flex-1 text-text-dim">{t("login.classic_ready")}</span>
              </>
            )}
            <span className="shrink-0 text-text-faint">›</span>
          </button>
        ))}

      {/* Account field with dropdown */}
      <div className="mb-3">
        <label
          htmlFor="login-account"
          className="mb-1 block text-[11px] font-semibold tracking-[2px] text-text-dim uppercase"
        >
          {t("login.username")}
        </label>
        <div className="relative" ref={dropdownRef}>
          <input
            id="login-account"
            ref={accountInputRef}
            type="text"
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            onKeyDown={handleAccountKeyDown}
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
              {savedAccounts.map((saved, idx) => (
                <div
                  key={saved.account}
                  ref={(el) => {
                    if (idx === highlightIdx && el) {
                      el.scrollIntoView({ block: "nearest" });
                    }
                  }}
                  className={`group flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-[var(--text)] transition-colors ${
                    idx === highlightIdx
                      ? "bg-[rgba(232,162,58,0.12)]"
                      : "hover:bg-[rgba(232,162,58,0.08)]"
                  }`}
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
          className="mb-1 block text-[11px] font-semibold tracking-[2px] text-text-dim uppercase"
        >
          {t("login.password")}
        </label>
        <PasswordInput
          id="login-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={isLoading}
          placeholder={t("login.password_placeholder")}
          autoComplete="new-password"
          data-form-type="other"
          className="w-full rounded-lg border border-border bg-[var(--surface)] py-2.5 pr-9 pl-3.5 text-[13px] text-[var(--text)] placeholder:text-[12px] placeholder:text-text-dim focus:border-[rgba(232,162,58,0.4)] focus:bg-[var(--surface-hover)] focus:shadow-[0_0_0_3px_var(--input-focus-ring)] focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Options row */}
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-text-dim transition-colors hover:text-[var(--text)]">
          <input
            type="checkbox"
            name="remember-password"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
            className="h-3.5 w-3.5 accent-accent"
          />
          {t("login.remember")}
        </label>
        <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-text-dim transition-colors hover:text-[var(--text)]">
          <input
            type="checkbox"
            name="auto-login"
            checked={useConfigStore.getState().config?.autoLogin ?? false}
            onChange={(e) => {
              commands.setConfig("auto_login", String(e.target.checked)).catch(() => {});
              const cfg = useConfigStore.getState().config;
              if (cfg) {
                useConfigStore.setState({ config: { ...cfg, autoLogin: e.target.checked } });
              }
            }}
            className="h-3.5 w-3.5 accent-accent"
          />
          {t("login.auto_login")}
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

      {/* Café / shared-PC mode — wipes all local data on close */}
      <label
        className={`mb-3 flex cursor-pointer items-start gap-2 rounded-lg border px-3 py-2 transition-colors ${
          cafeMode
            ? "border-[rgba(232,162,58,0.45)] bg-[rgba(232,162,58,0.08)]"
            : "border-border bg-[var(--surface)] hover:border-[rgba(232,162,58,0.3)]"
        }`}
      >
        <input
          type="checkbox"
          name="cafe-mode"
          checked={cafeMode}
          onChange={(e) => (e.target.checked ? setCafeConfirm(true) : setCafeMode(false))}
          className="mt-0.5 h-3.5 w-3.5 shrink-0 accent-accent"
        />
        <span className="min-w-0">
          <span className="flex items-center gap-1 text-[12px] font-semibold text-[var(--text)]">
            🖥️ {t("login.cafe_mode")}
          </span>
          <span className="mt-0.5 block text-[11px] leading-snug text-text-dim">
            {t("login.cafe_mode_hint")}
          </span>
        </span>
      </label>

      {login.error && (
        <p className="mb-2 text-[12px] text-[var(--danger)]">{login.error.message}</p>
      )}

      {/* Actions row: Sign In + QR button (QR only for TW) */}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={isLoading || !account.trim() || !password.trim()}
          className="flex-1 rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-5 py-2.5 text-[11px] font-bold tracking-[2px] text-white uppercase shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:translate-y-[-1px] hover:shadow-[0_4px_20px_var(--accent-glow)] active:scale-95 disabled:transform-none disabled:cursor-not-allowed disabled:opacity-40"
        >
          {isLoading
            ? t("login.logging_in")
            : classicMode
              ? t("login.classic_hk_login")
              : t("login.submit")}
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
        {showQr && (
          <button
            type="button"
            onClick={onWebLaunch}
            title={t("web_launch.title")}
            className="flex h-[42px] w-[42px] shrink-0 items-center justify-center rounded-lg border border-border bg-[var(--surface)] text-text-dim transition-all hover:border-accent hover:bg-[var(--surface-hover)] hover:text-accent active:scale-95"
          >
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
              <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />
              <path
                d="M2 9H16M9 2C11 4 11.5 7 11.5 9C11.5 11 11 14 9 16C7 14 6.5 11 6.5 9C6.5 7 7 4 9 2Z"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        )}
      </div>

      {/* Classic also accepts GamePass (TW) sign-in, reusing the regular flow */}
      {classicMode && (
        <button
          type="button"
          onClick={() => {
            // GamePass OAuth is gated to TW; classic reuses it, so switch the
            // region to TW for the flow. The portal keys off the session region.
            commands.setConfig("region", "TW").catch(() => {});
            const cfg = useConfigStore.getState().config;
            if (cfg) useConfigStore.setState({ config: { ...cfg, region: "TW" } });
            onGamePass();
          }}
          disabled={isLoading}
          className="mt-2 flex items-center justify-center gap-2 rounded-lg border border-border bg-[var(--surface)] px-5 py-2.5 text-[12px] font-semibold text-text-dim transition-all hover:border-accent hover:text-accent active:scale-95 disabled:opacity-40"
        >
          <svg width="16" height="16" viewBox="0 0 18 18" fill="none">
            <path
              d="M9 1.5L2 5.5V12.5L9 16.5L16 12.5V5.5L9 1.5Z"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinejoin="round"
            />
            <path d="M9 8.5V16.5" stroke="currentColor" strokeWidth="1.5" />
            <path d="M2 5.5L9 9.5L16 5.5" stroke="currentColor" strokeWidth="1.5" />
          </svg>
          {t("login.classic_gamepass")}
        </button>
      )}

      {/* Café-mode confirmation — enabling is destructive on every close */}
      <Modal
        isOpen={cafeConfirm}
        onClose={() => setCafeConfirm(false)}
        title={t("login.cafe_confirm_title")}
      >
        <div className="flex flex-col gap-4">
          <p className="text-xs leading-relaxed text-text-dim">{t("login.cafe_confirm_body")}</p>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => setCafeConfirm(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              onClick={() => {
                setCafeMode(true);
                setCafeConfirm(false);
              }}
              className="rounded-lg bg-[var(--danger)] px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("login.cafe_confirm_enable")}
            </button>
          </div>
        </div>
      </Modal>

      {/* Classic readiness self-check */}
      <Modal
        isOpen={showCheckDetail}
        onClose={() => setShowCheckDetail(false)}
        title={t("login.classic_check_title")}
      >
        <div className="flex flex-col gap-2.5 text-[12px]">
          <div className="flex items-center gap-2">
            <span
              className={
                classicCheck?.ngmRegistered && classicCheck?.ngmExeExists
                  ? "text-green-500"
                  : "text-red-500"
              }
            >
              {classicCheck?.ngmRegistered && classicCheck?.ngmExeExists ? "✓" : "✗"}
            </span>
            <span className="flex-1 text-[var(--text)]">{t("login.check_ngm")}</span>
          </div>
          {classicCheck?.ngmExe && (
            <p className="pl-6 font-mono text-[10px] break-all text-text-faint">
              {classicCheck.ngmExe}
            </p>
          )}
          <div className="flex items-center gap-2">
            <span className={classicCheck?.webview2Version ? "text-green-500" : "text-yellow-500"}>
              {classicCheck?.webview2Version ? "✓" : "?"}
            </span>
            <span className="flex-1 text-[var(--text)]">WebView2</span>
            <span className="font-mono text-[11px] text-text-dim">
              {classicCheck?.webview2Version ?? t("login.check_unknown")}
            </span>
          </div>
          {!classicCheck?.ngmRegistered && (
            <div className="mt-1 flex flex-col gap-2 rounded-md bg-[rgba(234,179,8,0.08)] px-2.5 py-2">
              <p className="text-[11px] leading-relaxed text-yellow-500">
                {t("login.check_ngm_missing")}
              </p>
              <button
                type="button"
                onClick={() => commands.openExternal(NGM_DOWNLOAD_URL).catch(() => {})}
                className="self-start rounded-md bg-accent px-3 py-1 text-[11px] font-semibold text-white transition-opacity hover:opacity-90"
              >
                {t("login.classic_download")}
              </button>
            </div>
          )}
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={runClassicCheck}
              className="rounded-lg border border-border px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("login.classic_check")}
            </button>
            <button
              type="button"
              onClick={() => setShowCheckDetail(false)}
              className="rounded-lg bg-accent px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("common.ok")}
            </button>
          </div>
        </div>
      </Modal>
    </form>
  );
}
