import React from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import styles from "./UiTitlebar.module.css";

/**
 * The 34px title bar. On the sign-in page it carries the classic toggle and
 * the region flag (both clickable); everywhere else just the region code.
 */
export default function UiTitlebar({
  page,
  region,
  classic = false,
  hint = null,
  onToggleRegion,
  onToggleClassic,
  onClient,
}: {
  page: "login" | "main";
  region: "HK" | "TW";
  classic?: boolean;
  hint?: "region" | "client" | null;
  onToggleRegion?: () => void;
  onToggleClassic?: () => void;
  /** Given only where the demo is about the client manager shortcut. */
  onClient?: () => void;
}) {
  const t = useT();
  return (
    <div className={styles.tb}>
      <span className={styles.brand}>MAPLELINK</span>
      {page === "login" ? (
        <>
          <button
            className={clsx(styles.btn, classic && styles.on)}
            title={t("懷舊服", "怀旧服", "Classic")}
            onClick={onToggleClassic}
          >
            🍁{classic && <i className={styles.under} />}
          </button>
          <button
            className={clsx(styles.btn, hint === "region" && "ml-hint")}
            title={t("切換地區", "切换地区", "Toggle region")}
            onClick={onToggleRegion}
          >
            {/* The app uses flag emoji here; Windows has no flag glyphs and draws
                the letters instead, which is what players actually see. */}
            {region}
            {!classic && <i className={styles.under} />}
          </button>
        </>
      ) : (
        <span className={clsx(styles.btn, styles.faint)}>{region}</span>
      )}
      {/* The client manager shortcut, drawn rather than set as an emoji —
          the same reason the app draws it. */}
      <button
        className={clsx(styles.btn, hint === "client" && "ml-hint")}
        title={t("遊戲客戶端管理", "游戏客户端管理", "Game client manager")}
        onClick={onClient}
      >
        <svg
          viewBox="0 0 16 16"
          width="13"
          height="13"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M8 2v7" />
          <path d="M5 6.5 8 9.5l3-3" />
          <path d="M2.5 10.5v1.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5v-1.5" />
        </svg>
      </button>
      <span className={styles.btn}>🛠</span>
      <span className={clsx(styles.btn, styles.lg)}>−</span>
      <span className={clsx(styles.btn, styles.lg)}>×</span>
    </div>
  );
}
