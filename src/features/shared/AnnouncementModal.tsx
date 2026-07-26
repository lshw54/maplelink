import { useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import {
  ANNOUNCEMENT_FORCED_SECONDS,
  ANNOUNCEMENT_ID,
  ANNOUNCEMENT_LEVEL,
} from "../../lib/announcement";
import { AnnouncementBody } from "./AnnouncementBody";

/**
 * Announcement overlay. Mounted fresh each open (parent renders it
 * conditionally), so the countdown initialises correctly.
 *
 * When `forced` (first launch, unread) the behaviour follows the announcement's
 * level — see `ANNOUNCEMENT_LEVEL`. `info` is closable at once; `pinned` and
 * `blocking` lock for a countdown first, and of those only `blocking` insists on
 * the acknowledge button, with any other close bringing it back next launch.
 * Reopened from the banner, it's plain content with a Close button.
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
  const counts = forced && ANNOUNCEMENT_LEVEL !== "info";
  const [secondsLeft, setSecondsLeft] = useState(counts ? ANNOUNCEMENT_FORCED_SECONDS : 0);

  useEffect(() => {
    if (!counts) return;
    const iv = setInterval(() => setSecondsLeft((s) => (s <= 1 ? 0 : s - 1)), 1000);
    return () => clearInterval(iv);
  }, [counts]);

  const locked = counts && secondsLeft > 0;
  // Closing by X or backdrop. Everything but `blocking` treats that as read —
  // only `blocking` reserves that for the acknowledge button.
  const dismiss = () => {
    if (locked) return;
    if (forced && ANNOUNCEMENT_LEVEL !== "blocking") onMarkSeen();
    else onClose();
  };

  return (
    <div
      className="fixed inset-0 z-[110] flex items-center justify-center bg-black/55 p-5 backdrop-blur-[6px]"
      onMouseDown={dismiss}
    >
      <div
        className="flex w-[540px] max-w-full flex-col overflow-hidden rounded-2xl border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_20px_60px_rgba(0,0,0,0.45)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center gap-2.5 border-b border-[var(--tb-border)] px-6 py-4">
          <span className="text-lg">📢</span>
          <span className="flex-1 text-base font-bold text-[var(--text)]">
            {t("announcement.title")}
          </span>
          {!locked && (
            <button
              onClick={dismiss}
              aria-label={t("announcement.close")}
              className="text-[16px] leading-none text-text-faint transition-colors hover:text-[var(--text)]"
            >
              ×
            </button>
          )}
        </div>

        {/* Body */}
        <div className="flex flex-col gap-4 px-6 py-5">
          <AnnouncementBody id={ANNOUNCEMENT_ID} />

          {/* Action */}
          {forced ? (
            <button
              disabled={locked}
              onClick={onMarkSeen}
              className="mt-1 w-full rounded-lg bg-accent py-2.5 text-[13px] font-semibold text-white transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {locked
                ? t("announcement.reading", { seconds: String(secondsLeft) })
                : t("announcement.dismiss")}
            </button>
          ) : (
            <button
              onClick={onClose}
              className="mt-1 w-full rounded-lg bg-accent py-2.5 text-[13px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("announcement.close")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
