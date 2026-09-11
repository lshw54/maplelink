import React, { useCallback, useEffect, useRef, useState } from "react";
import clsx from "clsx";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiTitlebar from "./UiTitlebar";
import styles from "./DemoAnnouncement.module.css";

/**
 * The in-app announcement shown on first launch: a card over the dimmed
 * window whose only button counts down before it can be pressed. The real
 * app holds it for ten seconds; the demo uses five and says so.
 */
const HOLD = 5;

export default function DemoAnnouncement() {
  const t = useT();
  const logo = useBaseUrl("/logo.png");
  const [left, setLeft] = useState(HOLD);
  const [dismissed, setDismissed] = useState(false);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  const stop = useCallback(() => {
    if (timer.current) clearInterval(timer.current);
    timer.current = null;
  }, []);
  // Only the ticking, so the mount effect below sets no state of its own —
  // the countdown's initial value already comes from useState.
  const tick = useCallback(() => {
    stop();
    timer.current = setInterval(() => {
      setLeft((v) => {
        const next = v > 0 ? v - 1 : v;
        if (next === 0) stop();
        return next;
      });
    }, 1000);
  }, [stop]);

  /** "Try again": put the countdown back and run it, from a click. */
  const start = useCallback(() => {
    setLeft(HOLD);
    setDismissed(false);
    tick();
  }, [tick]);

  useEffect(() => {
    tick();
    return stop;
  }, [tick, stop]);

  const coach = dismissed
    ? t("關閉後不會再自動彈出。日後可在工具箱的「公告」分頁重看。", "关闭后不会再自动弹出。日后可在工具箱的「公告」标签重看。", "It will not pop up again. You can reread it later under Toolbox → Announcements.")
    : left > 0
      ? t("按鈕會鎖住幾秒，讓你先讀完（真實程式是 10 秒）。", "按钮会锁住几秒，让你先读完（真实程序是 10 秒）。", "The button stays locked for a few seconds so you read first (ten in the real app).")
      : t("可以按了。", "可以点了。", "You can press it now.");

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={640} height={440}>
        <div className={clsx("ml", styles.an)}>
          <div className="ml-glow"></div>
          <UiTitlebar page="login" region="HK" />
          <div className={styles.an__bg}>
            <img src={logo} alt="" />
            <span>MAPLELINK</span>
          </div>
          {!dismissed && (
            <div className={styles.an__overlay}>
              <div className={styles.an__card}>
                <div className={styles.an__head}>
                  <span>📢</span>
                  <b>{t("下載來源提醒", "下载来源提醒", "Where to download this")}</b>
                </div>
                <div className={styles.an__body}>
                  <p>{t("我們接獲回報，有非官方、疑似被重新打包的 Beanfun 啟動器在外流傳。", "我们接获回报，有非官方、疑似被重新打包的 Beanfun 启动器在外流传。", "We have had reports of unofficial, seemingly repackaged copies of the Beanfun launcher circulating.")}</p>
                  <p>{t("開發團隊每次發佈，只會提供 GitHub 上該版本 exe 的下載連結。我們給的永遠是連結，不是檔案。", "开发团队每次发布，只会提供 GitHub 上该版本 exe 的下载连结。我们给的永远是连结，不是档案。", "For every release, the development team gives out one thing: a link to that version's exe on GitHub. What we hand over is always a link, never a file.")}</p>
                  <p>{t("下載與使用前，請先確認來源安全可靠。不是從認可位置取得的，請立即刪除並重新下載。", "下载与使用前，请先确认来源安全可靠。不是从认可位置取得的，请立即删除并重新下载。", "Before you download it and before you run it, make sure the source is one you can trust. If it did not come from a recognised source, delete it and download it again.")}</p>
                </div>
                <div className={styles.an__foot}>
                  <button className={clsx(styles.an__btn, left > 0 && styles["an__btn--off"], left === 0 && "ml-hint")} disabled={left > 0} onClick={() => setDismissed(true)}>
                    {left > 0 ? t(`請先閱讀公告（${left} 秒）`, `请先阅读公告（${left} 秒）`, `Please read the notice (${left}s)`) : t("我已閱讀，下次不再顯示", "我已阅读，下次不再显示", "I've read it. Don't show again")}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </UiFrame>
      <UiCoach done={dismissed} action={dismissed ? <button onClick={start}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
