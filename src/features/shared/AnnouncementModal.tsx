import { useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import {
  ANNOUNCEMENT_BEANFUN_URL,
  ANNOUNCEMENT_FORCED_SECONDS,
  ANNOUNCEMENT_MORE_INFO_URL,
} from "../../lib/announcement";

function openExternal(url: string) {
  commands.openExternal(url).catch(() => {});
}

/** One project row: coloured dot + bold name (+ optional tag), description below. */
function ProjectRow({
  dot,
  name,
  nameClass,
  tag,
  desc,
}: {
  dot: string;
  name: string;
  nameClass: string;
  tag?: string;
  desc: string;
}) {
  return (
    <div className="rounded-xl border border-[var(--tb-border)] bg-[var(--surface)] px-4 py-3">
      <div className="flex items-center gap-2">
        <span className={`h-2 w-2 shrink-0 rounded-full ${dot}`} />
        <span className={`text-[13px] font-semibold ${nameClass}`}>{name}</span>
        {tag && <span className="text-[11px] text-text-dim">（{tag}）</span>}
      </div>
      <p className="mt-1.5 text-[12px] leading-relaxed text-text-dim">{desc}</p>
    </div>
  );
}

/**
 * Announcement overlay. Mounted fresh each open (parent renders it
 * conditionally), so the countdown initialises correctly.
 *
 * - `forced` (first launch, unread): a mandatory countdown runs; the overlay
 *   can't be closed until it hits zero, then the only action is "read & don't
 *   show again", which persists the seen-state.
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

  const locked = forced && secondsLeft > 0;
  const backdropClose = () => {
    if (!forced) onClose();
  };

  return (
    <div
      className="fixed inset-0 z-[110] flex items-center justify-center bg-black/55 p-5 backdrop-blur-[6px]"
      onMouseDown={backdropClose}
    >
      <div
        className="flex w-[540px] max-w-full flex-col overflow-hidden rounded-2xl border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_20px_60px_rgba(0,0,0,0.45)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center gap-2.5 border-b border-[var(--tb-border)] px-6 py-4">
          <span className="text-lg">📢</span>
          <span className="text-base font-bold text-[var(--text)]">{t("announcement.title")}</span>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-4 px-6 py-5">
          <p className="text-[13px] leading-relaxed text-[var(--text)]">
            {t("announcement.intro")}
          </p>

          <div className="flex flex-col gap-2.5">
            <ProjectRow
              dot="bg-accent"
              name="MapleLink"
              nameClass="text-accent"
              tag={t("announcement.this_project")}
              desc={t("announcement.maplelink")}
            />
            <ProjectRow
              dot="bg-blue-400"
              name="Beanfun"
              nameClass="text-[var(--text)]"
              desc={t("announcement.beanfun")}
            />
          </div>

          {/* Links */}
          <div className="flex flex-wrap items-center gap-x-5 gap-y-1.5">
            <button
              onClick={() => openExternal(ANNOUNCEMENT_BEANFUN_URL)}
              className="text-[12px] font-semibold text-accent transition-opacity hover:opacity-80"
            >
              Beanfun ↗
            </button>
            <button
              onClick={() => openExternal(ANNOUNCEMENT_MORE_INFO_URL)}
              className="text-[12px] font-semibold text-accent transition-opacity hover:opacity-80"
            >
              {t("announcement.more_info_link")} ↗
            </button>
          </div>

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
