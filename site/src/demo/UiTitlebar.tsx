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
}: {
  page: "login" | "main";
  region: "HK" | "TW";
  classic?: boolean;
  hint?: "region" | null;
  onToggleRegion?: () => void;
  onToggleClassic?: () => void;
}) {
  const t = useT();
  return (
    <div className={styles.tb}>
      <span className={styles.brand}>MAPLELINK</span>
      {page === "login" ? (
        <>
          <button className={clsx(styles.btn, classic && styles.on)} title={t("懷舊服", "怀旧服", "Classic")} onClick={onToggleClassic}>
            🍁{classic && <i className={styles.under} />}
          </button>
          <button className={clsx(styles.btn, hint === "region" && "ml-hint")} title={t("切換地區", "切换地区", "Toggle region")} onClick={onToggleRegion}>
            {/* The app uses flag emoji here; Windows has no flag glyphs and draws
                the letters instead, which is what players actually see. */}
            {region}
            {!classic && <i className={styles.under} />}
          </button>
        </>
      ) : (
        <span className={clsx(styles.btn, styles.faint)}>{region}</span>
      )}
      <span className={styles.btn}>🛠</span>
      <span className={clsx(styles.btn, styles.lg)}>−</span>
      <span className={clsx(styles.btn, styles.lg)}>×</span>
    </div>
  );
}
