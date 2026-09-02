import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import {
  ANNOUNCEMENT_BEANFUN_URL,
  ANNOUNCEMENT_MORE_INFO_URL,
  ANNOUNCEMENT_RELEASES_URL,
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

/** A project's releases page: its name, and the address in full, clickable. */
function ReleaseLink({ name, url }: { name: string; url: string }) {
  return (
    <button onClick={() => openExternal(url)} className="group text-left">
      <span className="block text-[11px] text-text-faint">{name}</span>
      <span className="block text-[12px] font-semibold break-all text-accent underline decoration-transparent underline-offset-2 transition-colors group-hover:decoration-current">
        {url}
      </span>
    </button>
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

  if (id === "2026-09-download-source") {
    return (
      <>
        <p className="text-[13px] leading-relaxed text-[var(--text)]">
          {t("announcement.download_source.intro")}
        </p>

        <div className="rounded-xl border border-[var(--tb-border)] bg-[var(--surface)] px-4 py-3">
          <p className="text-[12px] leading-relaxed text-[var(--text)]">
            {t("announcement.download_source.rule")}
          </p>
          {/* The addresses are the thing being checked, so they are both
              readable in full and clickable — a bare label like "downloads
              page" gives the reader nothing to compare their own copy to. */}
          <div className="mt-3 flex flex-col gap-2">
            <ReleaseLink name="MapleLink" url={ANNOUNCEMENT_RELEASES_URL} />
            <ReleaseLink name="Beanfun" url={`${ANNOUNCEMENT_BEANFUN_URL}/releases`} />
          </div>
        </div>

        <p className="text-[12px] leading-relaxed text-text-dim">
          {t("announcement.download_source.tell")}
        </p>
        <p className="text-[12px] leading-relaxed font-semibold text-[var(--text)]">
          {t("announcement.download_source.act")}
        </p>
      </>
    );
  }

  if (id !== "2026-07-dual-track") return null;

  return (
    <>
      <p className="text-[13px] leading-relaxed text-[var(--text)]">
        {t("announcement.dual_track.intro")}
      </p>

      <div className="flex flex-col gap-2.5">
        <ProjectRow
          dot="bg-accent"
          name="MapleLink"
          nameClass="text-accent"
          tag={t("announcement.dual_track.this_project")}
          desc={t("announcement.dual_track.maplelink")}
        />
        <ProjectRow
          dot="bg-blue-400"
          name="Beanfun"
          nameClass="text-[var(--text)]"
          desc={t("announcement.dual_track.beanfun")}
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
          {t("announcement.dual_track.more_info_link")} ↗
        </button>
      </div>
    </>
  );
}
