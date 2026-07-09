import { useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { ANNOUNCEMENT_FORCED_SECONDS, ANNOUNCEMENT_MORE_INFO_URL } from "../../lib/announcement";

function openExternal(url: string) {
  import("@tauri-apps/plugin-shell").then(({ open }) => open(url));
}

/**
 * Announcement overlay. Mounted fresh each time it opens (parent renders it
 * conditionally), so the countdown state initialises correctly.
 *
 * - `forced` (first launch, not yet read): a mandatory countdown runs; the modal
 *   cannot be closed until it hits zero, then the only action is "read &
 *   don't show again" which persists the seen-state.
 * - not forced (reopened from the banner): plain content with a Close button.
 */
export function AnnouncementModal({
  forced,
  onClose,
  onMarkSeen,
}: {
  forced: boolean;
  onClose: () => void;
  onMarkSeen: () => void;
}) {
  const { t } = useTranslation();
  const [secondsLeft, setSecondsLeft] = useState(forced ? ANNOUNCEMENT_FORCED_SECONDS : 0);

  useEffect(() => {
    if (!forced) return;
    const iv = setInterval(() => setSecondsLeft((s) => (s <= 1 ? 0 : s - 1)), 1000);
    return () => clearInterval(iv);
  }, [forced]);

  // During the mandatory countdown the overlay is not dismissable.
  const locked = forced && secondsLeft > 0;
  const backdropClose = () => {
    if (!locked && !forced) onClose();
  };

  return (
    <div
      className="fixed inset-0 z-[110] flex items-center justify-center bg-black/55 backdrop-blur-[6px]"
      onMouseDown={backdropClose}
    >
      <div
        className="w-[380px] max-w-[92vw] rounded-[14px] border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_12px_40px_rgba(0,0,0,0.35)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-[var(--tb-border)] px-5 py-3">
          <span className="flex-1 text-sm font-bold text-[var(--text)]">
            {t("announcement.title")}
          </span>
        </div>

        <div className="flex flex-col gap-3 px-5 py-4">
          <p className="text-[12px] leading-relaxed text-text-dim">{t("announcement.intro")}</p>

          <ul className="flex flex-col gap-2">
            <li className="rounded-[10px] border border-[var(--tb-border)] bg-[var(--surface)] px-3 py-2 text-[12px] leading-relaxed text-[var(--text)]">
              <span className="font-semibold text-accent">MapleLink</span>
              <span className="text-text-dim">（{t("announcement.this_project")}）</span>：
              {t("announcement.maplelink")}
            </li>
            <li className="rounded-[10px] border border-[var(--tb-border)] bg-[var(--surface)] px-3 py-2 text-[12px] leading-relaxed text-[var(--text)]">
              <span className="font-semibold">Beanfun</span>：{t("announcement.beanfun")}
            </li>
          </ul>

          <button
            onClick={() => openExternal(ANNOUNCEMENT_MORE_INFO_URL)}
            className="self-start text-[12px] text-accent underline-offset-2 transition-opacity hover:underline hover:opacity-80"
          >
            {t("announcement.more_info_link")} ↗
          </button>

          <div className="mt-1 flex justify-end">
            {forced ? (
              <button
                disabled={locked}
                onClick={onMarkSeen}
                className="rounded-lg bg-accent px-4 py-2 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {locked
                  ? t("announcement.reading", { seconds: String(secondsLeft) })
                  : t("announcement.dismiss")}
              </button>
            ) : (
              <button
                onClick={onClose}
                className="rounded-lg bg-accent px-4 py-2 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
              >
                {t("announcement.close")}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
