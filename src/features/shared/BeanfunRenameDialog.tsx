import { useState } from "react";
import { useTranslation } from "../../lib/i18n";

/**
 * Offered on startup when the IP looks like mainland China and the exe isn't
 * already `Beanfun.exe`. Renaming lets game accelerators (网游加速器) match the
 * process by name so login / reCAPTCHA work behind the GFW. Confirming renames
 * the exe and relaunches (the app exits); "don't ask again" is persisted.
 */
export function BeanfunRenameDialog({
  currentName,
  targetName,
  onConfirm,
  onDismiss,
  onCancel,
}: {
  currentName: string;
  targetName: string;
  onConfirm: () => void;
  onDismiss: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [dontAsk, setDontAsk] = useState(false);
  const [working, setWorking] = useState(false);

  function handleClose() {
    if (dontAsk) onDismiss();
    else onCancel();
  }

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/55 p-6 backdrop-blur-[6px]"
      onMouseDown={handleClose}
    >
      <div
        className="w-[340px] max-w-full rounded-2xl border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_20px_60px_rgba(0,0,0,0.45)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-[var(--tb-border)] px-5 py-3.5">
          <span className="text-base">🚀</span>
          <span className="text-sm font-bold text-[var(--text)]">{t("beanfun_rename.title")}</span>
        </div>

        <div className="flex flex-col gap-3.5 px-5 py-4">
          <p className="text-[12px] leading-relaxed text-text-dim">{t("beanfun_rename.message")}</p>

          <div className="flex items-center justify-center gap-2 rounded-lg border border-[var(--tb-border)] bg-[var(--tb-input-bg)] px-3 py-2 font-mono text-[12px]">
            <span className="text-text-dim">{currentName || "MapleLink.exe"}</span>
            <span className="text-accent">→</span>
            <span className="font-semibold text-[var(--text)]">{targetName}</span>
          </div>

          <p className="text-[11px] leading-relaxed text-text-faint">{t("beanfun_rename.hint")}</p>

          <label className="flex cursor-pointer items-center gap-2 text-[12px] text-[var(--text)] select-none">
            <input
              type="checkbox"
              checked={dontAsk}
              onChange={(e) => setDontAsk(e.target.checked)}
              className="h-3.5 w-3.5 accent-[var(--accent)]"
            />
            {t("beanfun_rename.dont_ask")}
          </label>

          <div className="flex gap-2">
            <button
              onClick={handleClose}
              disabled={working}
              className="flex-1 rounded-lg border border-border py-2 text-[12px] font-semibold text-[var(--text)] transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {t("beanfun_rename.later")}
            </button>
            <button
              onClick={() => {
                setWorking(true);
                onConfirm();
              }}
              disabled={working}
              className="flex-1 rounded-lg bg-accent py-2 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              {working ? t("beanfun_rename.renaming") : t("beanfun_rename.confirm")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
