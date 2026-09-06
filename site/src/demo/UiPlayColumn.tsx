import React, { forwardRef, useImperativeHandle, useState } from "react";
import clsx from "clsx";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useT } from "./i18n";
import { useLatestVersion } from "./release";
import styles from "./UiPlayColumn.module.css";

/**
 * The left-hand side of the main window: the live / classic switch, the game
 * badge, the round PLAY button, logout, and the status line at the bottom.
 * PLAY runs a short timer and then reports the game as running.
 */
export interface UiPlayColumnHandle {
  reset(): void;
}

const UiPlayColumn = forwardRef<UiPlayColumnHandle, { canClassic?: boolean; hint?: boolean; onLaunched?: () => void }>(
  function UiPlayColumn({ canClassic = true, hint = false, onLaunched }, ref) {
    const t = useT();
    const version = useLatestVersion();
    const game = useBaseUrl("/MapleStory.png");

    const [classic, setClassic] = useState(false);
    const [launching, setLaunching] = useState(false);
    const [running, setRunning] = useState(false);

    function play() {
      if (launching) return;
      setLaunching(true);
      setTimeout(() => {
        setLaunching(false);
        setRunning(true);
        onLaunched?.();
      }, 800);
    }
    useImperativeHandle(ref, () => ({
      reset() {
        setRunning(false);
      },
    }));

    return (
      <div className={styles.play}>
        <img className={styles.play__ghost} src={game} alt="" />
        <div className={styles.play__col}>
          {canClassic && (
            <div className={styles.play__pill}>
              <button className={clsx(!classic && ["ml-grad-deep", styles["play__pill--on"]])} onClick={() => setClassic(false)}>
                {t("正式服", "正式服", "Live")}
              </button>
              <button className={clsx(classic && ["ml-grad-deep", styles["play__pill--on"]])} onClick={() => setClassic(true)}>
                {t("懷舊服", "怀旧服", "Classic")}
              </button>
            </div>
          )}
          <div className={styles.play__game}>
            <img src={game} alt="" />
          </div>
          <div className={styles.play__name}>{classic ? t("新楓之谷：經典版", "新枫之谷：经典版", "MapleStory Classic") : "MapleStory"}</div>
          <div className={styles.play__sub}>Gamania · MMORPG</div>
          <button className={clsx(styles.play__btn, "ml-grad-deep", launching && styles["play__btn--busy"], hint && "ml-hint")} onClick={play}>
            {launching ? "..." : t("開始遊戲", "开始游戏", "PLAY")}
          </button>
          {running && (
            <span className={styles.play__running}>{t("執行中", "运行中", "Running")} (PID: 24816)</span>
          )}
          <div className={styles.play__logout}>{t("登出", "登出", "LOGOUT")}</div>
        </div>
        <div className={styles.play__status}>
          <span className={styles.play__online}>
            <i></i>ONLINE <small>42ms</small>
          </span>
          <span className={styles.play__ver}>MapleLink v{version ?? "…"}</span>
        </div>
      </div>
    );
  },
);

export default UiPlayColumn;
