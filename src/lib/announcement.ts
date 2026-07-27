/**
 * Current announcement identity + config.
 *
 * Bump `ANNOUNCEMENT_ID` whenever a NEW announcement is published — that resets
 * the "seen" state so every user is shown it once more. The body text lives in
 * i18n under the `announcement.*` keys.
 */
export const ANNOUNCEMENT_ID = "2026-07-dual-track";

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
 * Downgraded to `info`: it has been shown for a while and is now reference
 * material rather than news. Its banner can be closed for good, and the toolbox
 * keeps it readable — see {@link ANNOUNCEMENT_ARCHIVE}.
 */
export const ANNOUNCEMENT_LEVEL: AnnouncementLevel = "info";

/** Seconds a `pinned` / `blocking` announcement stays locked open. */
export const ANNOUNCEMENT_FORCED_SECONDS = 10;

/** External links opened from the announcement. */
export const ANNOUNCEMENT_MORE_INFO_URL = "https://github.com/pungin/Beanfun/issues/323";
export const ANNOUNCEMENT_BEANFUN_URL = "https://github.com/pungin/Beanfun";

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
  { id: ANNOUNCEMENT_ID, date: "2026-07" },
];
