import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { ANNOUNCEMENT_BEANFUN_URL, ANNOUNCEMENT_MORE_INFO_URL } from "../../lib/announcement";

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
 * The text of one announcement, without any chrome — so the overlay and the
 * toolbox's archive show exactly the same thing.
 *
 * Bodies are keyed by announcement id. A new announcement adds a case here and
 * an entry in `ANNOUNCEMENT_ARCHIVE`; nothing is ever removed, since the archive
 * is what makes closing a banner safe.
 */
export function AnnouncementBody({ id }: { id: string }) {
  const { t } = useTranslation();

  if (id !== "2026-07-dual-track") return null;

  return (
    <>
      <p className="text-[13px] leading-relaxed text-[var(--text)]">{t("announcement.intro")}</p>

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
    </>
  );
}
