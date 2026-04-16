import { useState, useRef, useCallback, type KeyboardEvent, type ClipboardEvent } from "react";
import { useTotpVerify } from "../../lib/hooks/use-auth";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useTranslation } from "../../lib/i18n";

interface TotpFormProps {
  onBack: () => void;
}

export function TotpForm({ onBack }: TotpFormProps) {
  const { t } = useTranslation();
  const totp = useTotpVerify();
  const [digits, setDigits] = useState<string[]>(["", "", "", "", "", ""]);
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);

  const setRef = useCallback((el: HTMLInputElement | null, idx: number) => {
    inputRefs.current[idx] = el;
  }, []);

  function handleInput(idx: number, value: string) {
    const cleaned = value.replace(/[^0-9]/g, "");
    if (!cleaned) return;
    const next = [...digits];
    next[idx] = cleaned[0] ?? "";
    setDigits(next);
    if (idx < 5) inputRefs.current[idx + 1]?.focus();
  }

  function handleKeyDown(idx: number, e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Backspace") {
      e.preventDefault();
      const next = [...digits];
      if (digits[idx]) {
        // Clear current digit
        next[idx] = "";
        setDigits(next);
      } else if (idx > 0) {
        // Move to previous and clear it
        next[idx - 1] = "";
        setDigits(next);
        inputRefs.current[idx - 1]?.focus();
      }
      return;
    }
    if (e.key === "Enter") handleSubmit();
  }

  function handlePaste(e: ClipboardEvent<HTMLInputElement>) {
    e.preventDefault();
    const text = e.clipboardData.getData("text").replace(/[^0-9]/g, "");
    const next = [...digits];
    for (let i = 0; i < 6 && i < text.length; i++) {
      next[i] = text[i] ?? "";
    }
    setDigits(next);
    const focusIdx = Math.min(text.length, 5);
    inputRefs.current[focusIdx]?.focus();
  }

  function handleSubmit() {
    const code = digits.join("");
    if (code.length < 6) return;
    // Get sessionId from pending credentials (stored during login that triggered TOTP)
    const pending = useAuthStore.getState().pendingCredentials;
    const sessionId = pending?.sessionId ?? "";
    totp.mutate(
      { sessionId, code },
      {
        onError: () => {
          // Reset digits on error so user can re-enter
          setDigits(["", "", "", "", "", ""]);
          inputRefs.current[0]?.focus();
        },
      },
    );
  }

  return (
    <div className="flex w-full flex-col items-center">
      {/* Branding */}
      <div className="mb-6 flex flex-col items-center">
        <div className="mb-2.5 flex h-10 w-10 items-center justify-center rounded-[10px] text-lg">
          🔐
        </div>
        <div className="text-[12px] uppercase tracking-[4px] text-text-dim">
          {t("login.totp.title")}
        </div>
        <div className="mt-1.5 text-[12px] tracking-[0.5px] text-text-faint">
          {t("login.totp.instruction")}
        </div>
      </div>

      {/* 6-digit boxes */}
      <div className="mb-5 flex items-center justify-center gap-2">
        {digits.map((d, i) => (
          <span key={i} className="contents">
            {i === 3 && <div className="mx-1 h-0.5 w-2 rounded-sm bg-text-faint" />}
            <input
              ref={(el) => setRef(el, i)}
              type="text"
              inputMode="numeric"
              maxLength={1}
              value={d}
              onChange={(e) => handleInput(i, e.target.value)}
              onKeyDown={(e) => handleKeyDown(i, e)}
              onPaste={i === 0 ? handlePaste : undefined}
              disabled={totp.isPending}
              className="h-12 w-10 rounded-[10px] border-[1.5px] border-border bg-[var(--surface)] text-center font-mono text-[22px] font-extrabold text-accent caret-accent outline-none transition-all focus:border-accent focus:bg-[var(--surface-hover)] focus:shadow-[0_0_0_3px_var(--input-focus-ring),0_0_12px_rgba(232,162,58,0.1)] disabled:opacity-50"
            />
          </span>
        ))}
      </div>

      {totp.error && <p className="mb-3 text-[12px] text-[var(--danger)]">{totp.error.message}</p>}

      {/* Verify button */}
      <button
        type="button"
        onClick={handleSubmit}
        disabled={totp.isPending || digits.join("").length < 6}
        className="w-full rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-5 py-2.5 text-[12px] font-semibold uppercase tracking-[1.5px] text-white shadow-[0_2px_12px_var(--accent-glow)] transition-all hover:shadow-[0_4px_20px_var(--accent-glow)] active:scale-95 disabled:opacity-40"
      >
        {totp.isPending ? t("login.totp.verifying") : t("login.totp.submit")}
      </button>

      {/* Back button */}
      <button
        type="button"
        onClick={onBack}
        className="mt-2.5 w-full rounded-lg border border-border bg-transparent px-3.5 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
      >
        {t("login.back_normal")}
      </button>
    </div>
  );
}
