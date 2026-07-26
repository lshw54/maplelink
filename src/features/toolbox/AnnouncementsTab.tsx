import { useTranslation } from "../../lib/i18n";
import { ANNOUNCEMENT_ARCHIVE } from "../../lib/announcement";
import { AnnouncementBody } from "../shared/AnnouncementBody";

/**
 * Every announcement ever published, newest first, each laid out in full.
 *
 * This is what makes the banner's × safe: closing it is a way to stop being
 * reminded, never a way to lose the text.
 */
export function AnnouncementsTab() {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-3">
      {ANNOUNCEMENT_ARCHIVE.map((entry) => (
        <article
          key={entry.id}
          className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]"
        >
          <header className="flex items-center gap-2 border-b border-[var(--tb-border)] bg-[rgba(232,162,58,0.06)] px-4 py-2.5">
            <span className="text-[13px]">📢</span>
            <span className="flex-1 text-[13px] font-bold text-[var(--text)]">
              {t("announcement.title")}
            </span>
            <time className="font-mono text-[11px] text-text-dim">{entry.date}</time>
          </header>
          <div className="flex flex-col gap-3 px-4 py-4">
            <AnnouncementBody id={entry.id} />
          </div>
        </article>
      ))}

      {ANNOUNCEMENT_ARCHIVE.length === 0 && (
        <p className="py-8 text-center text-[12px] text-text-dim">
          {t("toolbox.announcements.empty")}
        </p>
      )}
    </div>
  );
}
