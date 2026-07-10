import { useState } from "react";
import { useTranslation } from "../../lib/i18n";

/**
 * Asked when the user closes the window and the close behaviour is still "ask".
 * Picking an option optionally remembers it (persisted to config.ini via the
 * caller) and then quits or minimizes to the system tray.
 */
export function CloseDialog({
  onCancel,
  onChoose,
}: {
  onCancel: () => void;
  onChoose: (action: "quit" | "tray", remember: boolean) => void;
}) {
  const { t } = useTranslation();
  const [remember, setRemember] = useState(false);

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/55 p-6 backdrop-blur-[6px]"
      onMouseDown={onCancel}
    >
      <div
        className="w-[320px] max-w-full rounded-2xl border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_20px_60px_rgba(0,0,0,0.45)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="border-b border-[var(--tb-border)] px-5 py-3.5">
          <span className="text-sm font-bold text-[var(--text)]">{t("close_dialog.title")}</span>
        </div>

        <div className="flex flex-col gap-4 px-5 py-4">
          <p className="text-[12px] leading-relaxed text-text-dim">{t("close_dialog.message")}</p>

          <label className="flex cursor-pointer items-center gap-2 text-[12px] text-[var(--text)] select-none">
            <input
              type="checkbox"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
              className="h-3.5 w-3.5 accent-[var(--accent)]"
            />
            {t("close_dialog.remember")}
          </label>

          <div className="flex gap-2">
            <button
              onClick={() => onChoose("tray", remember)}
              className="flex-1 rounded-lg border border-border py-2 text-[12px] font-semibold text-[var(--text)] transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("close_dialog.tray")}
            </button>
            <button
              onClick={() => onChoose("quit", remember)}
              className="flex-1 rounded-lg bg-accent py-2 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("close_dialog.quit")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
