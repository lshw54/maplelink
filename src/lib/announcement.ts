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

/** Level of the announcement identified by {@link ANNOUNCEMENT_ID}. */
export const ANNOUNCEMENT_LEVEL: AnnouncementLevel = "pinned";

/** Seconds a `pinned` / `blocking` announcement stays locked open. */
export const ANNOUNCEMENT_FORCED_SECONDS = 10;

/** External links opened from the announcement. */
export const ANNOUNCEMENT_MORE_INFO_URL = "https://github.com/pungin/Beanfun/issues/323";
export const ANNOUNCEMENT_BEANFUN_URL = "https://github.com/pungin/Beanfun";
