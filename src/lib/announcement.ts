/**
 * Current announcement identity + config.
 *
 * Bump `ANNOUNCEMENT_ID` whenever a NEW announcement is published — that resets
 * the "seen" state so every user gets the mandatory forced-read once more. The
 * body text lives in i18n under the `announcement.*` keys.
 */
export const ANNOUNCEMENT_ID = "2026-07-dual-track";

/** Seconds the user must keep the announcement open before they can dismiss it. */
export const ANNOUNCEMENT_FORCED_SECONDS = 30;

/** External link opened from the announcement. */
export const ANNOUNCEMENT_MORE_INFO_URL = "https://github.com/pungin/Beanfun/issues/323";
