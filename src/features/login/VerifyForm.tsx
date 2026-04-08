import { useState, useEffect, useRef } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useLogin } from "../../lib/hooks/use-auth";
import { open } from "@tauri-apps/plugin-shell";
import type { AdvanceCheckState } from "../../lib/types";

interface VerifyFormProps {
  advanceCheckUrl?: string;
  onBack: () => void;
  onVerified: () => void;
  onAdvanceCheck: (url?: string) => void;
  onTotpRequired: () => void;
}

function extractErrorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (typeof err === "object" && err !== null) {
    const obj = err as Record<string, unknown>;
    if (typeof obj.message === "string" && obj.message) return obj.message;
    try {
      return JSON.stringify(err);
    } catch {
      /* fallback */
    }
  }
  return String(err);
}

export function VerifyForm({
  advanceCheckUrl,
  onBack,
  onVerified,
  onAdvanceCheck,
  onTotpRequired,
}: VerifyFormProps) {
  const { t } = useTranslation();
  const login = useLogin();
  const [checkState, setCheckState] = useState<AdvanceCheckState | null>(null);
  const [authInfo, setAuthInfo] = useState("");
  const [captchaCode, setCaptchaCode] = useState("");
  const [captchaImage, setCaptchaImage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [webVerifyUrl, setWebVerifyUrl] = useState<string | null>(null);
  const [reLogging, setReLogging] = useState(false);
  const loadedRef = useRef(false);

  // Load page ONCE only
  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;
    loadPage();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function loadPage() {
    setLoading(true);
    setError(null);
    setWebVerifyUrl(null);
    try {
      const state = await commands.getAdvanceCheck(advanceCheckUrl);
      setCheckState(state);
      setCaptchaImage(state.captchaImageBase64);
    } catch (err) {
      const msg = extractErrorMessage(err);
      if (msg.includes("advance_check_web:")) {
        const url = msg.replace("advance_check_web:", "").replace("Invalid credentials: ", "");
        setWebVerifyUrl(url);
      } else {
        setError(msg);
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleRefreshCaptcha() {
    if (!checkState) return;
    try {
      const newImage = await commands.refreshAdvanceCheckCaptcha(checkState.samplecaptcha);
      setCaptchaImage(newImage);
      setCaptchaCode("");
    } catch {
      /* ignore */
    }
  }

  async function handleSubmit() {
    if (!checkState || !authInfo.trim() || !captchaCode.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const success = await commands.submitAdvanceCheck({
        viewstate: checkState.viewstate,
        viewstateGenerator: checkState.viewstateGenerator,
        eventValidation: checkState.eventValidation,
        samplecaptcha: checkState.samplecaptcha,
        submitUrl: checkState.submitUrl,
        verifyCode: authInfo.trim(),
        captchaCode: captchaCode.trim(),
      });
      if (success) {
        // Auto re-login using pending credentials
        const pending = useAuthStore.getState().pendingCredentials;
        if (pending) {
          setReLogging(true);
          setError(null);
          login.mutate(
            {
              account: pending.account,
              password: pending.password,
              rememberPassword: pending.rememberPassword,
            },
            {
              onError: (err) => {
                if (err.message === "TOTP_REQUIRED" || err.name === "TotpRequired") {
                  onTotpRequired();
                } else if (err.message === "ADVANCE_CHECK" || err.name === "AdvanceCheck") {
                  onAdvanceCheck((err as { advanceUrl?: string }).advanceUrl);
                } else {
                  // Login failed after verify — go back to login page
                  onVerified();
                }
              },
            },
          );
        } else {
          onVerified();
        }
      }
    } catch (err) {
      setError(extractErrorMessage(err));
      await handleRefreshCaptcha();
    } finally {
      setSubmitting(false);
    }
  }

  if (loading) {
    return (
      <div className="flex w-full flex-col items-center gap-4 py-8">
        <span className="text-xs text-text-dim">{t("app.loading")}</span>
      </div>
    );
  }

  // Re-logging in after verification
  if (reLogging) {
    return (
      <div className="flex w-full flex-col items-center gap-4 py-12">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-text-faint border-t-accent" />
        <span className="text-[12px] uppercase tracking-[2px] text-text-dim">
          {t("login.logging_in")}
        </span>
      </div>
    );
  }

  // New-style web verification
  if (webVerifyUrl) {
    return (
      <div className="flex w-full flex-col items-center gap-4">
        <div className="text-lg">🔒</div>
        <div className="text-[12px] uppercase tracking-[3px] text-text-dim">
          {t("login.verify.title")}
        </div>
        <p className="text-center text-[12px] leading-relaxed text-text-dim">
          {t("login.verify.web_required")}
        </p>
        <button
          onClick={() => open(webVerifyUrl)}
          className="w-full rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-5 py-2.5 text-[12px] font-semibold uppercase tracking-[1.5px] text-white shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:shadow-[0_4px_20px_var(--accent-glow)] active:scale-95"
        >
          {t("login.verify.open_browser")}
        </button>
        <button
          onClick={onBack}
          className="w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
        >
          {t("login.back_normal")}
        </button>
      </div>
    );
  }

  // Parse auth hint into label + masked value
  const hintLines = checkState?.authHint?.split("\n") ?? [];
  const hintLabel = hintLines[0] ?? "";
  const hintValue = hintLines[1] ?? "";

  return (
    <div className="flex w-full flex-col">
      {/* Auth hint */}
      {(hintLabel || hintValue) && (
        <div className="mb-3 rounded-lg bg-[rgba(232,162,58,0.06)] px-3 py-2">
          {hintLabel && <div className="text-[12px] text-text-dim">{hintLabel}</div>}
          {hintValue && (
            <div className="mt-0.5 text-[12px] font-semibold text-[var(--text)]">{hintValue}</div>
          )}
        </div>
      )}

      {/* Auth info input */}
      <div className="mb-2">
        <input
          type="text"
          value={authInfo}
          onChange={(e) => setAuthInfo(e.target.value)}
          placeholder={hintLabel || t("login.verify.auth_info_placeholder")}
          autoComplete="off"
          data-form-type="other"
          autoFocus
          disabled={submitting}
          className="w-full rounded-lg border border-border bg-[var(--surface)] px-3.5 py-2.5 text-[13px] text-[var(--text)] placeholder:text-[12px] placeholder:text-text-dim focus:border-[rgba(232,162,58,0.4)] focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Captcha code input */}
      <div className="mb-2">
        <input
          type="text"
          value={captchaCode}
          onChange={(e) => setCaptchaCode(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleSubmit();
          }}
          placeholder={t("login.verify.captcha_placeholder")}
          autoComplete="off"
          data-form-type="other"
          disabled={submitting}
          className="w-full rounded-lg border border-border bg-[var(--surface)] px-3.5 py-2.5 text-[13px] text-[var(--text)] placeholder:text-[12px] placeholder:text-text-dim focus:border-[rgba(232,162,58,0.4)] focus:outline-none disabled:opacity-50"
        />
      </div>

      {/* Captcha image */}
      <div className="mb-3 flex justify-center">
        {captchaImage ? (
          <img
            src={captchaImage}
            alt="captcha"
            onClick={handleRefreshCaptcha}
            className="h-10 w-[200px] cursor-pointer rounded-lg border border-border object-contain transition-opacity hover:opacity-70"
            title={t("login.verify.refresh_captcha")}
          />
        ) : (
          <button
            onClick={handleRefreshCaptcha}
            className="flex h-10 w-[200px] items-center justify-center rounded-lg border border-border bg-[var(--surface)] text-[12px] text-text-dim"
          >
            {t("login.verify.refresh_captcha")}
          </button>
        )}
      </div>

      {error && <p className="mb-2 text-[12px] text-[var(--danger)]">{error}</p>}

      <button
        onClick={handleSubmit}
        disabled={submitting || !authInfo.trim() || !captchaCode.trim()}
        className="w-full rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-5 py-2.5 text-[12px] font-semibold uppercase tracking-[1.5px] text-white shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:shadow-[0_4px_20px_var(--accent-glow)] active:scale-95 disabled:opacity-40"
      >
        {submitting ? t("login.verify.submitting") : t("login.verify.submit")}
      </button>

      <button
        onClick={onBack}
        className="mt-2 w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
      >
        {t("login.back_normal")}
      </button>
    </div>
  );
}
