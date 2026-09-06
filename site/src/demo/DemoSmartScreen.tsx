import React, { useState } from "react";
import clsx from "clsx";
import { useLocale, useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoSmartScreen.module.css";

/**
 * The Windows SmartScreen dialog, playable. As on a real PC, "Run anyway" is
 * hidden until "More info" is clicked. The dialog's own language follows the
 * visitor's Windows, not this site, so a switch lets them see both.
 */
export default function DemoSmartScreen() {
  const t = useT();
  const lang = useLocale();
  const [ui, setUi] = useState<"zh" | "en">(lang.startsWith("en") ? "en" : "zh");
  const [expanded, setExpanded] = useState(false);
  const [done, setDone] = useState<"run" | "stop" | null>(null);

  const s = (() => {
    const cn = lang === "zh-CN";
    if (ui === "en")
      return {
        title: "Windows protected your PC",
        body: "Microsoft Defender SmartScreen prevented an unrecognized app from starting. Running this app might put your PC at risk.",
        more: "More info",
        app: "App:",
        publisher: "Publisher:",
        unknown: "Unknown publisher",
        run: "Run anyway",
        stop: "Don't run",
      };
    return cn
      ? {
          title: "Windows 已保护你的电脑",
          body: "Microsoft Defender SmartScreen 已阻止启动一个未识别的应用。运行此应用可能会导致你的电脑存在风险。",
          more: "更多信息",
          app: "应用:",
          publisher: "发布者:",
          unknown: "未知发布者",
          run: "仍要运行",
          stop: "不运行",
        }
      : {
          title: "Windows 已保護您的電腦",
          body: "Microsoft Defender SmartScreen 已防止某個無法辨識的應用程式啟動。執行此應用程式可能會讓您的電腦暴露在風險中。",
          more: "其他資訊",
          app: "應用程式:",
          publisher: "發行者:",
          unknown: "未知的發行者",
          run: "仍要執行",
          stop: "不要執行",
        };
  })();

  const coach = (() => {
    if (done === "run") return t("程式就會開啟。這個提示只在第一次出現。", "程序就会打开。这个提示只在第一次出现。", "The app opens. This prompt appears only the first time.");
    if (done === "stop") return t("什麼都不會發生。核對過 SHA256 就可以放心按另一個。", "什么都不会发生。核对过 SHA256 就可以放心点另一个。", "Nothing happens. Once the SHA256 matches, the other button is safe.");
    if (!expanded) return t("一開始沒有「仍要執行」。先按「其他資訊」。", "一开始没有「仍要运行」。先点「更多信息」。", "There is no Run anyway at first. Click More info.");
    return t("現在按「仍要執行」。發行者顯示未知是正常的，程式沒有買商業憑證。", "现在点「仍要运行」。发布者显示未知是正常的，程序没有买商业证书。", "Now click Run anyway. Unknown publisher is expected; the app has no commercial certificate.");
  })();
  function reset() {
    setExpanded(false);
    setDone(null);
  }

  return (
    <div className={clsx("demo", styles.demo)}>
      <div className={styles["ss-lang"]}>
        <span>{t("對話框語言跟隨你的 Windows：", "对话框语言跟随你的 Windows：", "The dialog follows your Windows language:")}</span>
        <button
          className={clsx(ui === "zh" && styles.on)}
          onClick={() => {
            setUi("zh");
            reset();
          }}
        >
          {lang === "zh-CN" ? "简体中文" : "繁體中文"}
        </button>
        <button
          className={clsx(ui === "en" && styles.on)}
          onClick={() => {
            setUi("en");
            reset();
          }}
        >
          English
        </button>
      </div>
      <UiFrame width={520} height={400}>
        <div className={clsx(styles.ss, done && styles["ss--done"])}>
          <button className={styles.ss__close} aria-label="Close" onClick={() => setDone("stop")}>
            ✕
          </button>
          <h2 className={styles.ss__title}>{s.title}</h2>
          <p className={styles.ss__body}>{s.body}</p>
          {!expanded ? (
            <button className={clsx(styles.ss__more, !done && styles["ss-hint"])} onClick={() => setExpanded(true)}>
              {s.more}
            </button>
          ) : (
            <dl className={styles.ss__info}>
              <dt>{s.app}</dt>
              <dd>MapleLink.exe</dd>
              <dt>{s.publisher}</dt>
              <dd>{s.unknown}</dd>
            </dl>
          )}
          <div className={styles.ss__actions}>
            {expanded && (
              <button className={clsx(styles.ss__btn, !done && styles["ss-hint"])} onClick={() => setDone("run")}>
                {s.run}
              </button>
            )}
            <button className={styles.ss__btn} onClick={() => setDone("stop")}>
              {s.stop}
            </button>
          </div>
        </div>
      </UiFrame>
      <UiCoach done={done === "run"} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
