import { useMemo, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import {
  ANNOUNCEMENT_ARCHIVE,
  ANNOUNCEMENT_KINDS,
  announcementKey,
  announcementKind,
  type AnnouncementKind,
} from "../../lib/announcement";
import { AnnouncementBody } from "../shared/AnnouncementBody";

/**
 * Every announcement ever published, as a list you open one at a time.
 *
 * This is what makes the banner's × safe: closing it stops the reminder, it
 * never loses the text. Laid out like a mailbox rather than a wall of stacked
 * notices — the list stays scannable however many pile up.
 *
 * The two kinds are kept apart and the list opens on notices: a release note
 * arrives with every version, so without the split they would bury the handful
 * of things that were worth interrupting someone for.
 */
export function AnnouncementsTab() {
  const { t } = useTranslation();
  const [openId, setOpenId] = useState<string | null>(null);
  const [kind, setKind] = useState<AnnouncementKind>("notice");
  const open = ANNOUNCEMENT_ARCHIVE.find((a) => a.id === openId);

  const counts = useMemo(() => {
    const by: Record<AnnouncementKind, number> = { notice: 0, update: 0 };
    for (const entry of ANNOUNCEMENT_ARCHIVE) by[announcementKind(entry)] += 1;
    return by;
  }, []);

  const shown = useMemo(
    () => ANNOUNCEMENT_ARCHIVE.filter((entry) => announcementKind(entry) === kind),
    [kind],
  );

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
              {t(announcementKey(open.id, "title"))}
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

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-1.5">
        {ANNOUNCEMENT_KINDS.map((id) => (
          <button
            key={id}
            onClick={() => setKind(id)}
            className={`rounded-lg border px-3 py-1 text-[12px] font-semibold transition-colors ${
              kind === id
                ? "border-accent bg-[var(--surface-hover)] text-[var(--text)]"
                : "border-[var(--tb-border)] text-text-dim hover:bg-[var(--surface-hover)]"
            }`}
          >
            {t(`toolbox.announcements.kind_${id}`)}{" "}
            <span className="font-mono text-[11px] text-text-faint">{counts[id]}</span>
          </button>
        ))}
      </div>

      {shown.length === 0 ? (
        <p className="py-8 text-center text-[12px] text-text-dim">
          {t(`toolbox.announcements.empty_${kind}`)}
        </p>
      ) : (
        <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]">
          {shown.map((entry, i) => (
            <button
              key={entry.id}
              onClick={() => setOpenId(entry.id)}
              className={`flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--surface-hover)] ${
                i > 0 ? "border-t border-[var(--tb-border)]" : ""
              }`}
            >
              <span className="shrink-0 text-[13px]">📢</span>
              <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-[var(--text)]">
                {t(announcementKey(entry.id, "title"))}
              </span>
              <time className="shrink-0 font-mono text-[11px] text-text-dim">{entry.date}</time>
              <span className="shrink-0 text-[12px] text-text-faint">›</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
