import React, { useState } from "react";
import clsx from "clsx";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiTitlebar from "./UiTitlebar";
import styles from "./DemoRename.module.css";

/** The "rename to Beanfun.exe" prompt some users see on first launch. */
export default function DemoRename() {
  const t = useT();
  const logo = useBaseUrl("/logo.png");
  const [dontAsk, setDontAsk] = useState(false);
  const [working, setWorking] = useState(false);
  const [done, setDone] = useState<"renamed" | "later" | null>(null);

  function confirm() {
    setWorking(true);
    setTimeout(() => {
      setWorking(false);
      setDone("renamed");
    }, 900);
  }
  function reset() {
    setDone(null);
    setDontAsk(false);
  }
  const coach = (() => {
    if (done === "renamed") return t("程式改名為 Beanfun.exe 並重新啟動。功能完全一樣，自動更新也會保留新名稱。", "程序改名为 Beanfun.exe 并重新启动。功能完全一样，自动更新也会保留新名称。", "The app renames itself to Beanfun.exe and restarts. Nothing else changes; auto-update keeps the new name.");
    if (done === "later") return t("不改名也可以照常使用，只是部分加速器認不到。", "不改名也可以照常使用，只是部分加速器认不到。", "You can carry on without renaming; some accelerators just will not see the app.");
    return t("用網遊加速器就按「改名並重啟」；不用就按「稍後」。", "用网游加速器就点「改名并重启」；不用就点「稍后」。", "Using a game accelerator? Click Rename. Otherwise click Later.");
  })();

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={520} height={400}>
        <div className={clsx("ml", styles.rn)}>
          <div className="ml-glow"></div>
          <UiTitlebar page="login" region="HK" />
          <div className={styles.rn__bg}>
            <img src={logo} alt="" />
            <span>MAPLELINK</span>
          </div>
          {!done ? (
            <div className={styles.rn__overlay}>
              <div className={styles.rn__card}>
                <div className={styles.rn__head}>
                  <span>🚀</span>
                  <b>{t("改名以配合加速器？", "改名以配合加速器？", "Rename for accelerator?")}</b>
                </div>
                <div className={styles.rn__body}>
                  <p>{t("偵測到你的連線來自中國大陸。網遊加速器是按程式名稱來加速的。將本程式改名為 Beanfun.exe，加速器就能對應，登入與 reCAPTCHA 才會穩定。", "检测到你的连接来自中国大陆。网游加速器是按进程名称来加速的。将本程序改名为 Beanfun.exe，加速器就能对应，登录与 reCAPTCHA 才会稳定。", "Your connection looks like it is from mainland China. Game accelerators route traffic by process name. Renaming this app to Beanfun.exe lets the accelerator match it, so login and reCAPTCHA work reliably.")}</p>
                  <div className={styles.rn__names}>
                    <span>MapleLink.exe</span>
                    <i>→</i>
                    <b>Beanfun.exe</b>
                  </div>
                  <small>{t("程式會自行改名並重新啟動。之後自動更新會保留新名稱。", "程序会自行改名并重新启动。之后自动更新会保留新名称。", "The app will rename itself and restart. Auto-update keeps the new name.")}</small>
                  <label className={styles.rn__check}>
                    <input checked={dontAsk} onChange={(e) => setDontAsk(e.target.checked)} type="checkbox" /> {t("不再提示", "不再提示", "Don't ask again")}
                  </label>
                  <div className={styles.rn__actions}>
                    <button className={styles.rn__later} onClick={() => setDone("later")}>
                      {t("稍後", "稍后", "Later")}
                    </button>
                    <button className={clsx(styles.rn__ok, working && styles["rn__ok--busy"])} disabled={working} onClick={confirm}>
                      {working ? t("改名中…", "改名中…", "Renaming…") : t("改名並重啟", "改名并重启", "Rename & restart")}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ) : done === "renamed" ? (
            <div className={styles.rn__restart}>
              <span className={styles.rn__brand}>BEANFUN</span>
              <small>{t("重新啟動中…", "重新启动中…", "Restarting…")}</small>
            </div>
          ) : null}
        </div>
      </UiFrame>
      <UiCoach done={done === "renamed"} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
