import React, { useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiToolbox from "./UiToolbox";
import UiCoach from "./UiCoach";
import styles from "./DemoClientEntry.module.css";

/**
 * Where the client manager opens from.
 *
 * Two routes lead to the same window, and the page says so in a sentence; this
 * shows both so nobody has to hunt for a 13-pixel icon. Clicking either one
 * lands on the same "it opened" state rather than pretending to draw the
 * window — the window itself has its own demo further down the page.
 */
export default function DemoClientEntry() {
  const t = useT();
  const [tab, setTab] = useState("tools");
  const [opened, setOpened] = useState<"titlebar" | "toolbox" | null>(null);

  const coach = (() => {
    if (opened === "titlebar")
      return t(
        "就是這個圖示。它一直在標題列上，不用先進工具箱。",
        "就是这个图标。它一直在标题栏上，不用先进工具箱。",
        "That is the icon. It sits in the title bar, so you never have to open the toolbox first.",
      );
    if (opened === "toolbox")
      return t(
        "工具箱「工具」分頁的第一項就是它，說明也寫在旁邊。",
        "工具箱「工具」分页的第一项就是它，说明也写在旁边。",
        "It is the first row on the toolbox's Tools tab, with its description beside it.",
      );
    if (tab !== "tools")
      return t("按左邊的「工具」。", "点左边的「工具」。", "Click Tools on the left.");
    return t(
      "兩條路都通到同一個視窗：標題列右上角的下載圖示，或者下面這一項。兩個都可以按。",
      "两条路都通到同一个窗口：标题栏右上角的下载图标，或者下面这一项。两个都可以点。",
      "Two ways in, one window: the download icon at the top right, or the row below. Try either.",
    );
  })();

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={750} height={490}>
        <UiToolbox
          tab={tab}
          onTabChange={setTab}
          titlebarHint={opened === null ? "client" : null}
          onTitlebarClient={() => setOpened("titlebar")}
        >
          {tab === "tools" ? (
            <div className={styles.tools}>
              <span className={styles.tools__head}>
                {t("遊戲客戶端", "游戏客户端", "Game Client")}
              </span>
              <button
                className={clsx(styles.row, opened === null && "ml-hint")}
                onClick={() => setOpened("toolbox")}
              >
                <span className={clsx(styles.row__icon, styles.row__icon_green)}>
                  <svg
                    viewBox="0 0 16 16"
                    width="16"
                    height="16"
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
                </span>
                <span className={styles.row__text}>
                  <strong>{t("遊戲客戶端管理", "游戏客户端管理", "Game client manager")}</strong>
                  <em>
                    {t(
                      "檢查並修復現有客戶端，或取得官方完整下載",
                      "检查并修复现有客户端，或获取官方完整下载",
                      "Check and repair the installed client, or get the official download",
                    )}
                  </em>
                </span>
                <span className={styles.row__chev}>›</span>
              </button>

              <button className={styles.row} disabled>
                <span className={clsx(styles.row__icon, styles.row__icon_indigo)}>📂</span>
                <span className={styles.row__text}>
                  <strong>{t("資料夾", "文件夹", "Data folder")}</strong>
                  <em>
                    {t(
                      "存放設定與帳號的位置",
                      "存放设置与账号的位置",
                      "Where settings and accounts live",
                    )}
                  </em>
                </span>
                <span className={styles.row__chev}>›</span>
              </button>

              <span className={styles.tools__head}>
                {t("系統工具", "系统工具", "System Tools")}
              </span>
              <button className={styles.row} disabled>
                <span className={clsx(styles.row__icon, styles.row__icon_red)}>🗑</span>
                <span className={styles.row__text}>
                  <strong>{t("清理暫存", "清理缓存", "Clean up")}</strong>
                  <em>{t("釋出磁碟空間", "释放磁盘空间", "Free some disk space")}</em>
                </span>
                <span className={styles.row__chev}>›</span>
              </button>
            </div>
          ) : (
            <p className={styles.other}>
              {t(
                "這個示範只用到「工具」分頁。",
                "这个示范只用到「工具」分页。",
                "This walk-through only uses the Tools tab.",
              )}
            </p>
          )}
        </UiToolbox>
      </UiFrame>

      <UiCoach
        done={opened !== null}
        action={
          opened !== null ? (
            <button
              className={styles.reset}
              onClick={() => {
                setOpened(null);
                setTab("tools");
              }}
            >
              {t("再試一次", "再试一次", "Try again")}
            </button>
          ) : undefined
        }
      >
        {coach}
      </UiCoach>
    </div>
  );
}
