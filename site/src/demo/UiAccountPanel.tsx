import React, { useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import styles from "./UiAccountPanel.module.css";

/**
 * The right-hand side of the launcher's main window: session header, account
 * cards and the one-time-password panel, drawn to the app's own sizes. Fully
 * playable: pick a card, fetch an OTP (random digits, no network), toggle
 * auto-type. Reports what happened so a parent can narrate.
 */
export interface DemoAccount {
  initial: string;
  name: string;
}

export default function UiAccountPanel({
  accounts,
  user,
  beans,
  region = "HK",
  hint = null,
  onPick,
  onFetched,
}: {
  accounts: DemoAccount[];
  user: string;
  beans: number;
  /** HK shows the beans → game points conversion; TW does not. */
  region?: "HK" | "TW";
  /** Which control to nudge: a card, the fetch button, or nothing. */
  hint?: "pick" | "fetch" | null;
  onPick?: (index: number) => void;
  onFetched?: (otp: string) => void;
}) {
  const t = useT();

  const [selected, setSelected] = useState(0);
  const [autoInput, setAutoInput] = useState(true);
  const [otp, setOtp] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [pasted, setPasted] = useState(false);

  function pick(i: number) {
    setSelected(i);
    setOtp(null);
    onPick?.(i);
  }
  function flashCopied() {
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }
  function copy() {
    if (!otp) return;
    flashCopied();
  }
  function fetchOtp() {
    if (busy) return;
    setBusy(true);
    setOtp(null);
    setTimeout(() => {
      const code = String(Math.floor(100000 + Math.random() * 900000));
      setOtp(code);
      setBusy(false);
      // The real app copies the code as it arrives (green flash) and, with
      // auto-input on, also types it into the game window.
      flashCopied();
      if (autoInput) {
        setPasted(true);
        setTimeout(() => setPasted(false), 1600);
      }
      onFetched?.(code);
    }, 650);
  }

  return (
    <div className={styles.acc}>
      <div className={styles.acc__bar}>
        <span className={clsx(styles.acc__avatar, "ml-grad")}>{user.charAt(0).toUpperCase()}</span>
        <span>{user}</span>
        <span className={styles.acc__spacer}></span>
        <span className={styles.acc__beans}>
          <b className={styles["acc__beans-n"]}>
            {t("樂豆", "乐豆", "Beans")}: <b>{beans}</b>
          </b>
          {region === "HK" && (
            <>
              <i className={styles.acc__sep}>·</i>
              <span className={styles.acc__pts}>
                {t("遊戲點數", "游戏点数", "Game points")}: {Math.floor(beans / 2.5)}
              </span>
            </>
          )}
        </span>
        <span className={styles.acc__more}>⋯</span>
      </div>

      <div className={styles.acc__list}>
        <div className={styles.acc__head}>
          <span className={styles.acc__label}>
            {t("帳號列表", "账号列表", "ACCOUNTS")}
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
              <line x1="2" y1="4" x2="14" y2="4"></line>
              <line x1="2" y1="8" x2="14" y2="8"></line>
              <line x1="2" y1="12" x2="14" y2="12"></line>
            </svg>
          </span>
          <span className={styles.acc__refresh}>{t("重新整理", "刷新", "Refresh")}</span>
        </div>
        <div className={styles.acc__grid}>
          {accounts.map((a, i) => (
            <button
              key={a.name}
              className={clsx(styles.acc__card, i === selected && styles["acc__card--on"], hint === "pick" && i === (selected === 1 ? 0 : 1) && "ml-hint")}
              onClick={() => pick(i)}
            >
              <span className={styles.acc__init}>{a.initial}</span>
              <span>{a.name}</span>
            </button>
          ))}
        </div>
      </div>

      <div className={styles.acc__otp}>
        <div className={styles["acc__otp-head"]}>
          <span>🔐 {t("一次性密碼", "一次性密码", "ONE-TIME PASSWORD")}</span>
          <button className={styles.acc__auto} onClick={() => setAutoInput((v) => !v)}>
            {t("自動輸入", "自动输入", "Auto-type")}
            <i className={clsx(styles.acc__toggle, !autoInput && styles["acc__toggle--off"])}>
              <i></i>
            </i>
          </button>
        </div>
        <div className={styles["acc__otp-row"]}>
          <button className={clsx(styles.acc__code, !otp && styles["acc__code--empty"], copied && styles["acc__code--copied"])} onClick={copy}>
            {otp ?? "••••••••••"}
            <span className={clsx(styles.acc__paste, pasted && styles["acc__paste--show"])}>{t("已貼入遊戲", "已贴入游戏", "Pasted into game")}</span>
            <span className={styles.acc__copyicon}>{copied ? "✓" : "⧉"}</span>
          </button>
          <div className={clsx(styles.acc__fetch, "ml-grad", hint === "fetch" && "ml-hint")}>
            <button className={clsx(busy && styles.acc__spin)} onClick={fetchOtp}>
              ↻
            </button>
            <span className={styles.acc__caret}>▾</span>
          </div>
        </div>
      </div>
    </div>
  );
}
