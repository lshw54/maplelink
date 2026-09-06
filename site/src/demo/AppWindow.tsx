import React, { useRef, useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiTitlebar from "./UiTitlebar";
import UiPlayColumn, { type UiPlayColumnHandle } from "./UiPlayColumn";
import UiAccountPanel from "./UiAccountPanel";
import styles from "./AppWindow.module.css";

/**
 * The launcher's main window (760×530), playable, for the landing page. It is
 * assembled from the same pieces the guide embeds one at a time. A coach bar
 * at the bottom walks a visitor through the three clicks the real app takes;
 * every control works in any order. Account names are examples.
 */
interface Session {
  id: string;
  name: string;
  region: "HK" | "TW";
  beans: number;
  accounts: { initial: string; name: string }[];
}

export default function AppWindow() {
  const t = useT();

  const sessions: Session[] = [
    {
      id: "hk",
      name: t("主帳", "主账", "Main"),
      region: "HK",
      beans: 120,
      accounts: [
        { initial: t("角", "角", "A"), name: t("角色一", "角色一", "Alpha") },
        { initial: t("角", "角", "B"), name: t("角色二", "角色二", "Bravo") },
        { initial: t("倉", "仓", "M"), name: t("倉庫號", "仓库号", "Mule") },
        { initial: t("小", "小", "S"), name: t("小號", "小号", "Spare") },
      ],
    },
    {
      id: "tw",
      name: t("台服", "台服", "TW alt"),
      region: "TW",
      beans: 35,
      accounts: [
        { initial: t("台", "台", "T"), name: t("台服主號", "台服主号", "TW main") },
        { initial: t("練", "练", "L"), name: t("練功號", "练功号", "Leveller") },
      ],
    },
  ];

  const [active, setActive] = useState(0);
  const session = sessions[active];
  /** 0 pick an account · 1 fetch an OTP · 2 launch · 3 done */
  const [step, setStep] = useState(0);
  const [key, setKey] = useState(0);
  const play = useRef<UiPlayColumnHandle | null>(null);

  function pickSession(i: number) {
    if (i === active) return;
    setActive(i);
    setKey((k) => k + 1);
    play.current?.reset();
  }
  function reset() {
    setActive(0);
    setStep(0);
    setKey((k) => k + 1);
    play.current?.reset();
  }
  const coach = [
    t("試試看：按一個帳號卡片選擇帳號", "试试看：点一个账号卡片选择账号", "Try it: click an account card to select it"),
    t("按 ↻ 取得一次性密碼", "点 ↻ 获取一次性密码", "Click ↻ to get a one-time password"),
    t("OTP 已貼進遊戲。按「開始遊戲」", "OTP 已贴进游戏。点「开始游戏」", "The OTP is in the game. Press PLAY"),
    t("就是這樣，三下就進遊戲。", "就是这样，三下就进游戏。", "That is it. Three clicks into the game."),
  ][step];

  return (
    <UiFrame width={760} height={530}>
      <div className={clsx("ml", styles.win)}>
        <div className="ml-glow"></div>
        <UiTitlebar page="main" region={session.region} />

        <div className={styles.win__tabs}>
          {sessions.map((s, i) => (
            <button key={s.id} className={clsx(styles.win__tab, i === active && styles["win__tab--on"])} onClick={() => pickSession(i)}>
              <i className={clsx(styles.win__dot, s.region === "TW" ? styles["win__dot--tw"] : styles["win__dot--hk"])}></i>
              {s.name}
              <small>{s.region}</small>
            </button>
          ))}
          <span className={clsx(styles.win__tab, styles["win__tab--add"])}>+</span>
        </div>

        <div className={styles.win__body}>
          <div className={styles.win__left}>
            <UiPlayColumn ref={play} canClassic={session.region === "HK"} hint={step === 2} onLaunched={() => setStep((s) => (s <= 2 ? 3 : s))} />
          </div>
          <div className={styles.win__right}>
            <UiAccountPanel
              key={key}
              accounts={session.accounts}
              user={session.name}
              beans={session.beans}
              region={session.region}
              hint={step === 0 ? "pick" : step === 1 ? "fetch" : null}
              onPick={() => setStep((s) => (s === 0 ? 1 : s))}
              onFetched={() => setStep((s) => (s <= 1 ? 2 : s))}
            />
          </div>
        </div>

        <div className={clsx(styles.win__coach, step === 3 && styles["win__coach--done"])}>
          <span>{coach}</span>
          {step === 3 && <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button>}
        </div>
      </div>
    </UiFrame>
  );
}
