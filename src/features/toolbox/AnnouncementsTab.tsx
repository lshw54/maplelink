import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { ANNOUNCEMENT_ARCHIVE } from "../../lib/announcement";
import { AnnouncementBody } from "../shared/AnnouncementBody";

/**
 * Every announcement ever published, as a list you open one at a time.
 *
 * This is what makes the banner's × safe: closing it stops the reminder, it
 * never loses the text. Laid out like a mailbox rather than a wall of stacked
 * notices — the list stays scannable however many pile up.
 */
export function AnnouncementsTab() {
  const { t } = useTranslation();
  const [openId, setOpenId] = useState<string | null>(null);
  const open = ANNOUNCEMENT_ARCHIVE.find((a) => a.id === openId);

  if (open) {
    return (
      <div className="flex flex-col gap-3">
        <button
          onClick={() => setOpenId(null)}
          className="w-fit text-[12px] font-semibold text-text-dim transition-colors hover:text-accent"
        >
          ← {t("shared.titlebar.back")}
        </button>
        <article className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]">
          <header className="flex items-center gap-2 border-b border-[var(--tb-border)] bg-[rgba(var(--accent-rgb),0.06)] px-4 py-2.5">
            <span className="text-[13px]">📢</span>
            <span className="flex-1 text-[13px] font-bold text-[var(--text)]">
              {t("announcement.title")}
            </span>
            <time className="font-mono text-[11px] text-text-dim">{open.date}</time>
          </header>
          <div className="flex flex-col gap-3 px-4 py-4">
            <AnnouncementBody id={open.id} />
          </div>
        </article>
      </div>
    );
  }

  if (ANNOUNCEMENT_ARCHIVE.length === 0) {
    return (
      <p className="py-8 text-center text-[12px] text-text-dim">
        {t("toolbox.announcements.empty")}
      </p>
    );
  }

  return (
    <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]">
      {ANNOUNCEMENT_ARCHIVE.map((entry, i) => (
        <button
          key={entry.id}
          onClick={() => setOpenId(entry.id)}
          className={`flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--surface-hover)] ${
            i > 0 ? "border-t border-[var(--tb-border)]" : ""
          }`}
        >
          <span className="shrink-0 text-[13px]">📢</span>
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-[var(--text)]">
            {t("announcement.title")}
          </span>
          <time className="shrink-0 font-mono text-[11px] text-text-dim">{entry.date}</time>
          <span className="shrink-0 text-[12px] text-text-faint">›</span>
        </button>
      ))}
    </div>
  );
}
