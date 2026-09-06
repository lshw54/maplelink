import React, { useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiToolbox from "./UiToolbox";
import styles from "./DemoSettings.module.css";

/**
 * Toolbox → Settings (game path) and Toolbox → Advanced (launch toggles).
 * `focus` picks which control the coach points at and which tab opens first.
 */
export default function DemoSettings({ focus = "path" }: { focus?: "path" | "patcher" }) {
  const t = useT();
  const [tab, setTab] = useState(focus === "patcher" ? "advanced" : "settings");
  const [gamePath, setGamePath] = useState<string | null>(null);
  const [killPatcher, setKillPatcher] = useState(false);
  const [skipPlay, setSkipPlay] = useState(true);
  const [autoLaunch, setAutoLaunch] = useState(false);
  const [done, setDone] = useState(false);

  function browse() {
    setGamePath("C:\\Games\\MapleStory");
    if (focus === "path") setDone(true);
  }
  function togglePatcher() {
    setKillPatcher(!killPatcher);
    if (focus === "patcher") setDone(true);
  }
  function reset() {
    setDone(false);
    setGamePath(null);
    setKillPatcher(false);
    setTab(focus === "patcher" ? "advanced" : "settings");
  }
  const coach = (() => {
    if (focus === "path") {
      if (done) return t("路徑會存入 config.ini。之後啟動遊戲就用這裏的位置。", "路径会存入 config.ini。之后启动游戏就用这里的位置。", "The path is saved to config.ini and used for every launch.");
      if (tab !== "settings") return t("按左邊的「設定」。", "点左边的「设置」。", "Click Settings on the left.");
      return t("遊戲路徑顯示「—」代表未偵測到。按「瀏覽」，選 MapleStory.exe 所在的資料夾。", "游戏路径显示「—」代表未检测到。点「浏览」，选 MapleStory.exe 所在的文件夹。", "A dash means no path was detected. Click Browse and pick the folder that holds MapleStory.exe.");
    }
    if (done)
      return killPatcher
        ? t("已開啟。啟動遊戲時會自動關閉 Patcher.exe。平時建議關閉。", "已开启。启动游戏时会自动关闭 Patcher.exe。平时建议关闭。", "On. Patcher.exe is closed as the game starts. Leave it off normally.")
        : t("已關閉，恢復正常更新。", "已关闭，恢复正常更新。", "Off. Updates run as normal.");
    if (tab !== "advanced") return t("按左邊的「進階」。", "点左边的「高级」。", "Click Advanced on the left.");
    return t("按「阻止遊戲自動更新」右邊的開關。", "点「阻止游戏自动更新」右边的开关。", "Flip the switch beside Block Game Auto-Update.");
  })();

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={750} height={490}>
        <UiToolbox tab={tab} onTabChange={setTab}>
          {tab === "settings" ? (
            <div className={styles.st}>
              <section className={styles.st__sec}>
                <h3>{t("遊戲", "游戏", "Game")}</h3>
                <div className={styles.st__card}>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("遊戲路徑", "游戏路径", "Game Path")}</div>
                    <span className={clsx(styles.st__val, styles.st__mono)}>{gamePath ?? "—"}</span>
                    <button className={clsx(styles.st__btn, focus === "path" && !done && "ml-hint")} onClick={browse}>
                      {t("瀏覽", "浏览", "Browse")}
                    </button>
                  </div>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("經典版 NGM 路徑", "经典版 NGM 路径", "Classic NGM path")}</div>
                    <span className={clsx(styles.st__val, styles.st__mono)}>{t("自動偵測", "自动检测", "Auto-detect")}</span>
                    <button className={styles.st__btn}>{t("瀏覽", "浏览", "Browse")}</button>
                  </div>
                </div>
              </section>
              <section className={styles.st__sec}>
                <h3>{t("外觀", "外观", "Appearance")}</h3>
                <div className={styles.st__card}>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("主題", "主题", "Theme")}</div>
                    <span className={styles.st__seg}>
                      <b>{t("系統", "系统", "System")}</b>
                      <span>{t("深色", "深色", "Dark")}</span>
                      <span>{t("淺色", "浅色", "Light")}</span>
                    </span>
                  </div>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("語言", "语言", "Language")}</div>
                    <span className={styles.st__dd}>{t("繁體中文", "简体中文", "English")} ▾</span>
                  </div>
                </div>
              </section>
            </div>
          ) : tab === "advanced" ? (
            <div className={styles.st}>
              <section className={styles.st__sec}>
                <h3>{t("啟動", "启动", "Launch")}</h3>
                <div className={styles.st__card}>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("跳過 Play 視窗", "跳过 Play 窗口", "Skip Play Window")}</div>
                    <button className={clsx(styles.st__toggle, skipPlay && styles["st__toggle--on"])} onClick={() => setSkipPlay(!skipPlay)}>
                      <i></i>
                    </button>
                  </div>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>{t("啟動後自動開遊戲", "启动后自动开游戏", "Auto-launch game")}</div>
                    <button className={clsx(styles.st__toggle, autoLaunch && styles["st__toggle--on"])} onClick={() => setAutoLaunch(!autoLaunch)}>
                      <i></i>
                    </button>
                  </div>
                  <div className={styles.st__row}>
                    <div className={styles.st__label}>
                      {t("阻止遊戲自動更新", "阻止游戏自动更新", "Block Game Auto-Update")}
                      <small>{t("啟動遊戲時自動關閉 Patcher.exe，防止遊戲強制更新。", "启动游戏时自动关闭 Patcher.exe，防止游戏强制更新。", "Kill Patcher.exe when launching to prevent forced updates.")}</small>
                    </div>
                    <button
                      className={clsx(styles.st__toggle, killPatcher && styles["st__toggle--on"], focus === "patcher" && !done && "ml-hint")}
                      onClick={togglePatcher}
                    >
                      <i></i>
                    </button>
                  </div>
                </div>
              </section>
            </div>
          ) : (
            <div className={clsx(styles.st, styles.st__empty)}>{t("此示範只包含「設定」與「進階」。", "此示范只包含「设置」与「高级」。", "This demo covers Settings and Advanced only.")}</div>
          )}
        </UiToolbox>
      </UiFrame>
      <UiCoach done={done} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
