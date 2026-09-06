import clsx from "clsx";
import React from "react";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoBlank.module.css";

/**
 * What a PC without WebView2 shows: the window opens, but there is nothing to
 * draw the interface with, so it stays plain white (or closes at once).
 */
export default function DemoBlank() {
  const t = useT();
  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={350} height={300}>
        <div className={styles.bl}>
          <div className={styles.bl__bar}>
            <span>MapleLink</span>
            <span className={styles.bl__ctl}>−  □  ✕</span>
          </div>
          <div className={styles.bl__page}></div>
        </div>
      </UiFrame>
      <UiCoach>
        {t("整個視窗一片白、沒有任何文字，或一開啟就自己關掉，就是缺少 WebView2。", "整个窗口一片白、没有任何文字，或一打开就自己关掉，就是缺少 WebView2。", "An all-white window with no text, or one that closes by itself, means WebView2 is missing.")}
      </UiCoach>
    </div>
  );
}
