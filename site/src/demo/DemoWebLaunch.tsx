import React, { useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiTitlebar from "./UiTitlebar";
import styles from "./DemoWebLaunch.module.css";

/**
 * The web-launch page (560×640), reached from the globe button on the sign-in
 * page. One big switch, two behaviour toggles, a self-check list.
 */
export default function DemoWebLaunch() {
  const t = useT();
  const [enabled, setEnabled] = useState(false);
  const [autoLaunch, setAutoLaunch] = useState(true);
  const [autoPaste, setAutoPaste] = useState(true);
  const [msg, setMsg] = useState(false);

  function toggle() {
    const next = !enabled;
    setEnabled(next);
    setMsg(next);
  }
  const coach = enabled
    ? t("之後在 beanfun 官網登入並按「開始遊戲」，會改由 MapleLink 開遊戲並貼入 OTP。關掉開關就還原官方。", "之后在 beanfun 官网登录并点「开始游戏」，会改由 MapleLink 开游戏并贴入 OTP。关掉开关就还原官方。", "From now on, Start Game on the beanfun website launches through MapleLink and pastes the OTP. Turn the switch off to restore the original.")
    : t("入口在登入頁登入鈕右邊的地球圖示。下方四項自我檢查全綠後，開啟最上面的開關。", "入口在登录页登录钮右边的地球图标。下方四项自我检查全绿后，开启最上面的开关。", "Reached from the globe icon beside the sign-in button. Once the four checks below are green, turn on the top switch.");

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={560} height={640}>
        <div className={clsx("ml", styles.wl)}>
          <div className="ml-glow"></div>
          <UiTitlebar page="main" region="HK" />
          <div className={styles.wl__head}>
            <span>🌐</span>
            <b>{t("網頁登入開機（一鍵）", "网页登录开机（一键）", "Web-login launch (one-click)")}</b>
            <span className={styles.wl__back}>{t("返回", "返回", "Back")}</span>
          </div>
          <div className={styles.wl__body}>
            <p className={styles.wl__intro}>{t("只能用網頁登入（UU／VPN）時開啟。啟用後，在官網按「開始遊戲」會改由 MapleLink 開機，並自動填入帳號／OTP。", "只能用网页登录（UU／VPN）时开启。启用后，在官网点「开始游戏」会改由 MapleLink 开机，并自动填入账号／OTP。", "For when you can only log in via the website (UU / VPN). Once enabled, Start Game on the official site launches through MapleLink and auto-fills the account / OTP.")}</p>

            <div className={clsx(styles.wl__status, enabled ? styles["wl__status--ok"] : styles["wl__status--info"])}>
              <span>{enabled ? "✅" : "🔹"}</span>
              {enabled ? t("全部就緒，網頁開機已開啟", "全部就绪，网页开机已开启", "All set. Web launch is enabled") : t("環境已就緒，開啟開關即可使用", "环境已就绪，开启开关即可使用", "Environment ready. Turn on the switch to use it")}
            </div>

            <div className={clsx(styles.wl__enable, enabled && styles["wl__enable--on"])}>
              <div>
                <b>{t("啟用網頁開機攔截", "启用网页开机拦截", "Enable web-launch interception")}</b>
                <small>{enabled ? t("已開啟：官網「開始遊戲」會由 MapleLink 開機並自動填帳號／OTP。", "已开启：官网「开始游戏」会由 MapleLink 开机并自动填账号／OTP。", "On: the website's Start Game now launches through MapleLink and auto-fills the account / OTP.") : t("開啟後，官網「開始遊戲」會改由 MapleLink 開機並自動填帳號／OTP；關閉會還原官方。", "开启后，官网「开始游戏」会改由 MapleLink 开机并自动填账号／OTP；关闭会还原官方。", "When on, the website's Start Game launches through MapleLink and auto-fills the account / OTP; off restores the original.")}</small>
              </div>
              <button className={clsx(styles.wl__toggle, styles["wl__toggle--lg"], enabled && styles["wl__toggle--on"], !enabled && "ml-hint")} onClick={toggle}>
                <i></i>
              </button>
            </div>
            {msg && <p className={styles.wl__msg}>{t("已啟用：之後在官網按「開始遊戲」就會由 MapleLink 開機。", "已启用：之后在官网点「开始游戏」就会由 MapleLink 开机。", "Enabled: clicking Start Game on the website will now launch through MapleLink.")}</p>}

            <div className={styles.wl__sec}>{t("啟動行為", "启动行为", "Launch behaviour")}</div>
            <div className={styles.wl__row}>
              <div>
                <b>{t("自動開啟遊戲", "自动打开游戏", "Auto-open the game")}</b>
                <small>{t("在官網按「開始遊戲」時自動開啟 MapleStory。", "在官网点「开始游戏」时自动打开 MapleStory。", "Open MapleStory automatically when you click Start Game on the website.")}</small>
              </div>
              <button className={clsx(styles.wl__toggle, autoLaunch && styles["wl__toggle--on"])} onClick={() => setAutoLaunch(!autoLaunch)}>
                <i></i>
              </button>
            </div>
            <div className={styles.wl__row}>
              <div>
                <b>{t("自動填入帳號／OTP", "自动填入账号／OTP", "Auto-fill account / OTP")}</b>
                <small>{t("遊戲登入視窗出現後，自動填入帳號與 OTP。", "游戏登录窗口出现后，自动填入账号与 OTP。", "Fill the account and OTP once the game login window appears.")}</small>
              </div>
              <button className={clsx(styles.wl__toggle, autoPaste && styles["wl__toggle--on"])} onClick={() => setAutoPaste(!autoPaste)}>
                <i></i>
              </button>
            </div>

            <div className={styles.wl__sec}>{t("自我檢查", "自我检查", "Self-check")}</div>
            <div className={styles.wl__check}>
              <i>✓</i>
              <div>
                <b>{t("程式名稱", "程序名称", "App executable")}</b>
                <small>MapleLink.exe</small>
              </div>
            </div>
            <div className={styles.wl__check}>
              <i>✓</i>
              <div>
                <b>{t("遊戲路徑", "游戏路径", "Game path")}</b>
                <small>C:\Games\MapleStory</small>
              </div>
            </div>
            <div className={styles.wl__check}>
              <i>✓</i>
              <div>
                <b>{t("語系轉換器（LR）", "语系转换器（LR）", "Locale Remulator (LR)")}</b>
                <small>{t("已就緒，可用 LR 啟動 MapleStory", "已就绪，可用 LR 启动 MapleStory", "Ready. LR can launch MapleStory")}</small>
              </div>
            </div>
            <div className={styles.wl__check}>
              <i>✓</i>
              <div>
                <b>{t("官方啟動器（Gamania）", "官方启动器（Gamania）", "Official launcher (Gamania)")}</b>
                <small>{t("已偵測到 gamania Games Manager", "已检测到 gamania Games Manager", "Detected gamania Games Manager")}</small>
              </div>
            </div>
          </div>
        </div>
      </UiFrame>
      <UiCoach
        done={enabled}
        action={
          enabled ? (
            <button
              onClick={() => {
                setEnabled(false);
                setMsg(false);
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
