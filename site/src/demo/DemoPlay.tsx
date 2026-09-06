import clsx from "clsx";
import React, { useRef, useState } from "react";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiPlayColumn, { type UiPlayColumnHandle } from "./UiPlayColumn";
import styles from "./DemoPlay.module.css";

/** The PLAY column on its own, for the guide's launch step. */
export default function DemoPlay() {
  const t = useT();
  const [launched, setLaunched] = useState(false);
  const col = useRef<UiPlayColumnHandle | null>(null);
  const coach = launched
    ? t("遊戲已啟動。上方的正式服／懷舊服切換只在 HK 帳號出現。", "游戏已启动。上方的正式服／怀旧服切换只在 HK 账号出现。", "The game is running. The Live / Classic switch above appears for HK accounts only.")
    : t("按圓形的「開始遊戲」。", "点圆形的「开始游戏」。", "Press the round PLAY button.");
  function reset() {
    setLaunched(false);
    col.current?.reset();
  }

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={304} height={440}>
        <div className={`ml ${styles.panel}`}>
          <div className="ml-glow"></div>
          <UiPlayColumn ref={col} hint={!launched} onLaunched={() => setLaunched(true)} />
        </div>
      </UiFrame>
      <UiCoach done={launched} action={launched ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
