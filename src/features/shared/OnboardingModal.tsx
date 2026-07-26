import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { ONBOARDING_PAGES } from "../../lib/onboarding";

interface OnboardingModalProps {
  /** Finished or skipped — both close it and mark the guide as seen. */
  onClose: () => void;
}

/**
 * First-run guide: one screen per topic, stepped through with dots.
 *
 * Deliberately skippable — a guide nobody can dismiss is a guide people click
 * past without reading. The page copy lives in i18n; this only walks the list.
 */
export function OnboardingModal({ onClose }: OnboardingModalProps) {
  const { t } = useTranslation();
  const [index, setIndex] = useState(0);
  const total = ONBOARDING_PAGES.length;
  const page = ONBOARDING_PAGES[index];
  const last = index === total - 1;

  if (!page) return null;

  return (
    <div className="fixed inset-0 z-[80] flex flex-col bg-[var(--bg)]/97 backdrop-blur-sm">
      <div className="flex items-center justify-between px-5 pt-4">
        <span className="text-[11px] font-bold tracking-[2px] text-text-dim uppercase">
          {t("onboarding.title")}
        </span>
        <button
          type="button"
          onClick={onClose}
          className="text-[11px] text-text-faint transition-colors hover:text-accent"
        >
          {t("onboarding.skip")}
        </button>
      </div>

      <div className="flex flex-1 flex-col items-center justify-center gap-3 px-8 text-center">
        <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-[rgba(232,162,58,0.12)] text-[26px]">
          {page.icon}
        </div>
        <h2 className="max-w-[420px] text-[15px] font-bold text-[var(--text)]">
          {t(`onboarding.${page.key}_title`)}
        </h2>
        {/* One point per line — a solid paragraph is what people skip. */}
        <ul className="mx-auto flex w-fit max-w-full flex-col gap-1.5 text-left">
          {t(`onboarding.${page.key}_body`)
            .split("\n")
            .map((line) => (
              <li key={line} className="flex items-start gap-2 text-[12px] leading-snug">
                <span className="mt-[5px] h-1 w-1 shrink-0 rounded-full bg-accent" />
                <span className="text-text-dim">{line}</span>
              </li>
            ))}
        </ul>
      </div>

      <div className="flex items-center justify-between gap-3 px-5 pb-5">
        <button
          type="button"
          onClick={() => setIndex((i) => i - 1)}
          disabled={index === 0}
          className="rounded-lg border border-border px-3 py-1.5 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent disabled:cursor-not-allowed disabled:opacity-0"
        >
          {t("onboarding.back")}
        </button>

        <div className="flex items-center gap-1.5">
          {ONBOARDING_PAGES.map((p, i) => (
            <button
              key={p.key}
              type="button"
              onClick={() => setIndex(i)}
              aria-label={`${i + 1}`}
              className={`h-1.5 rounded-full transition-all ${
                i === index
                  ? "w-4 bg-accent"
                  : "w-1.5 bg-[var(--surface-hover)] hover:bg-text-faint"
              }`}
            />
          ))}
        </div>

        <button
          type="button"
          onClick={() => (last ? onClose() : setIndex((i) => i + 1))}
          className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-bold text-white shadow-[0_2px_10px_var(--accent-glow)] transition-all hover:translate-y-[-1px] active:scale-95"
        >
          {last ? t("onboarding.done") : t("onboarding.next")}
        </button>
      </div>
    </div>
  );
}
