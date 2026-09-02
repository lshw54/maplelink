/**
 * Current announcement identity + config.
 *
 * Bump `ANNOUNCEMENT_ID` whenever a NEW announcement is published — that resets
 * the "seen" state so every user is shown it once more. The body text lives in
 * i18n under the `announcement.*` keys.
 */
export const ANNOUNCEMENT_ID = "2026-09-download-source";

/**
 * How insistent an announcement is, from least to most:
 *
 * - `info` — closable straight away. For things worth saying once.
 * - `pinned` — must stay open for {@link ANNOUNCEMENT_FORCED_SECONDS} before it
 *   can be closed, and closing then counts as read. For things everyone really
 *   should see.
 * - `blocking` — same countdown, but only the acknowledge button counts as read;
 *   closing it any other way brings it back next launch. For the rare notice
 *   that must not be clicked past.
 */
export type AnnouncementLevel = "info" | "pinned" | "blocking";

/**
 * Level of the announcement identified by {@link ANNOUNCEMENT_ID}.
 *
 * `pinned`: people are being handed repackaged builds, and someone who never
 * reads this cannot tell one from ours. It holds for
 * {@link ANNOUNCEMENT_FORCED_SECONDS} and then closing it counts as read, so
 * it is asked of each person exactly once; the banner stays behind it, and the
 * toolbox keeps the text — see {@link ANNOUNCEMENT_ARCHIVE}.
 */
export const ANNOUNCEMENT_LEVEL: AnnouncementLevel = "pinned";

/** Seconds a `pinned` / `blocking` announcement stays locked open. */
export const ANNOUNCEMENT_FORCED_SECONDS = 10;

/** External links opened from the announcement. */
export const ANNOUNCEMENT_MORE_INFO_URL = "https://github.com/pungin/Beanfun/issues/323";
export const ANNOUNCEMENT_BEANFUN_URL = "https://github.com/pungin/Beanfun";

/**
 * The only place this app is published.
 *
 * Spelled out in the announcement rather than linked as bare text, so the
 * address someone compares their download against is the one the app itself
 * shows them.
 */
export const ANNOUNCEMENT_RELEASES_URL = "https://github.com/lshw54/maplelink/releases";

/**
 * The i18n key prefix an announcement's own strings live under.
 *
 * Titles used to read from a fixed `announcement.title`, which was right while
 * there was one announcement and wrong the moment there were two: the archive
 * would have shown the newest title above every older body.
 */
const KEY_PREFIX: Record<string, string> = {
  "2026-07-dual-track": "announcement.dual_track",
  "2026-09-download-source": "announcement.download_source",
};

/** i18n key for `leaf` of the announcement `id`. */
export function announcementKey(id: string, leaf: string): string {
  const prefix = KEY_PREFIX[id];
  return prefix === undefined ? `announcement.${leaf}` : `${prefix}.${leaf}`;
}

/**
 * Every announcement ever published, newest first — nothing is ever removed.
 * The toolbox lists these so closing a banner is never how someone loses the
 * text behind it.
 */
export interface ArchivedAnnouncement {
  id: string;
  /** Shown next to the title in the list. */
  date: string;
}

export const ANNOUNCEMENT_ARCHIVE: ArchivedAnnouncement[] = [
  { id: ANNOUNCEMENT_ID, date: "2026-09" },
  { id: "2026-07-dual-track", date: "2026-07" },
];
