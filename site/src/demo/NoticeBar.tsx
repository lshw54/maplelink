import React, { type CSSProperties } from "react";
import Link from "@docusaurus/Link";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useLocale } from "./i18n";
import { NOTICES } from "./notices";
import styles from "./NoticeBar.module.css";

/**
 * A one-line ticker above the nav. Headlines scroll past slowly; hovering
 * pauses them, and each one links to its section on the announcements page.
 * With reduced motion the newest headline is shown still.
 */
export default function NoticeBar() {
  const locale = useLocale();
  // useBaseUrl already includes the current locale's path segment.
  const announcements = useBaseUrl("/announcements");

  const items = NOTICES.map((n) => ({
    id: n.id,
    date: n.date,
    title: n.title[locale],
    href: `${announcements}#${n.id}`,
  }));
  const label = locale === "zh-CN" ? "公告" : locale === "en" ? "Notice" : "公告";
  const more = locale === "zh-CN" ? "全部公告" : locale === "en" ? "All announcements" : "全部公告";
  /* Roughly 12 s per headline keeps a sentence readable at a glance. */
  const duration = `${Math.max(20, items.length * 12)}s`;

  if (!items.length) return null;
  return (
    <div className={styles.notice}>
      <span className={styles.notice__label}>{label}</span>
      <div className={styles.notice__track} style={{ "--duration": duration } as CSSProperties}>
        <div className={styles.notice__reel}>
          {[1, 2].map((pass) =>
            items.map((n) => (
              <Link
                key={`${pass}-${n.id}`}
                className={styles.notice__item}
                to={n.href}
                aria-hidden={pass === 2 || undefined}
                tabIndex={pass === 2 ? -1 : undefined}
              >
                <time className={styles.notice__date}>{n.date}</time>
                <span>{n.title}</span>
              </Link>
            )),
          )}
        </div>
      </div>
      <Link className={styles.notice__more} to={announcements}>
        {more}
      </Link>
    </div>
  );
}
