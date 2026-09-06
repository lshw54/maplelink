import React, { useRef, useState, type FormEvent } from "react";
import clsx from "clsx";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useT } from "./i18n";
import { NATIVE_INPUT } from "./native";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiTitlebar from "./UiTitlebar";
import UiPlayColumn from "./UiPlayColumn";
import UiAccountPanel from "./UiAccountPanel";
import styles from "./DemoSessions.module.css";

/**
 * Several accounts open at once. "+" on the tab strip brings back the
 * sign-in page; signing in adds a tab. Each tab keeps its own accounts and
 * OTP panel; hovering a tab shows its close button.
 */
interface Session {
  id: number;
  name: string;
  region: "HK" | "TW";
  beans: number;
  accounts: { initial: string; name: string }[];
}

export default function DemoSessions() {
  const t = useT();
  const logo = useBaseUrl("/logo.png");
  const first: Session = {
    id: 1,
    name: t("主帳", "主账", "Main"),
    region: "HK",
    beans: 120,
    accounts: [
      { initial: t("角", "角", "A"), name: t("角色一", "角色一", "Alpha") },
      { initial: t("角", "角", "B"), name: t("角色二", "角色二", "Bravo") },
    ],
  };
  const [sessions, setSessions] = useState<Session[]>(() => [first]);
  const [active, setActive] = useState(0);
  const [adding, setAdding] = useState(false);
  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [region, setRegion] = useState<"HK" | "TW">("TW");
  const [busy, setBusy] = useState(false);
  const nextId = useRef(2);

  const session = sessions[active];
  const canSubmit = account.trim() !== "" && password.trim() !== "" && !busy;

  function submit(e: FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setTimeout(() => {
      const name = account.trim();
      const next: Session = {
        id: nextId.current++,
        name,
        region,
        beans: region === "TW" ? 35 : 80,
        accounts: [
          { initial: name.charAt(0).toUpperCase(), name: t("角色一", "角色一", "Alpha") },
          { initial: name.charAt(0).toUpperCase(), name: t("練功號", "练功号", "Leveller") },
        ],
      };
      setSessions((list) => {
        const updated = [...list, next];
        setActive(updated.length - 1);
        return updated;
      });
      setAdding(false);
      setBusy(false);
      setAccount("");
      setPassword("");
    }, 800);
  }
  function close(i: number) {
    const updated = sessions.filter((_, j) => j !== i);
    setSessions(updated);
    if (active >= updated.length) setActive(updated.length - 1);
  }
  function reset() {
    setSessions([first]);
    setActive(0);
    setAdding(false);
  }
  const coach = adding
    ? t("這就是登入頁，只多了「返回帳號列表」。填好帳密按「登入」。", "这就是登录页，只多了「返回账号列表」。填好账密点「登录」。", "This is the sign-in page with one extra button, Back to Accounts. Fill in and sign in.")
    : sessions.length >= 2
      ? t("每個分頁各有自己的帳號列表和 OTP。點分頁切換；滑到分頁上按 × 可登出該帳號。", "每个标签各有自己的账号列表和 OTP。点标签切换；移到标签上点 × 可登出该账号。", "Each tab has its own accounts and OTP. Click a tab to switch; hover one and press × to sign that account out.")
      : t("按分頁列右邊的「+」加入第二個帳號。", "点标签栏右边的「+」添加第二个账号。", "Click + at the end of the tab strip to add a second account.");
  const done = sessions.length >= 2 && !adding;

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={760} height={530}>
        <div className={clsx("ml", styles.se)}>
          <div className="ml-glow"></div>
          <UiTitlebar
            page={adding ? "login" : "main"}
            region={adding ? region : session.region}
            onToggleRegion={() => setRegion((r) => (r === "HK" ? "TW" : "HK"))}
          />

          <div className={styles.se__tabs}>
            {sessions.map((s, i) => (
              <button
                key={s.id}
                className={clsx(styles.se__tab, i === active && !adding && styles["se__tab--on"])}
                onClick={() => {
                  setActive(i);
                  setAdding(false);
                }}
              >
                <i className={clsx(styles.se__dot, s.region === "TW" ? styles["se__dot--tw"] : styles["se__dot--hk"])}></i>
                {s.name}
                <small>{s.region}</small>
                {sessions.length > 1 && (
                  <span
                    className={styles.se__close}
                    title={t("登出", "登出", "Close")}
                    onClick={(e) => {
                      e.stopPropagation();
                      close(i);
                    }}
                  >
                    ×
                  </span>
                )}
              </button>
            ))}
            <button
              className={clsx(styles.se__tab, styles["se__tab--add"], sessions.length < 2 && !adding && "ml-hint")}
              title={t("新增帳號", "添加账号", "Add Account")}
              onClick={() => setAdding(true)}
            >
              +
            </button>
          </div>

          {!adding ? (
            <div className={styles.se__body}>
              <div className={styles.se__left}>
                <UiPlayColumn key={session.id} canClassic={session.region === "HK"} />
              </div>
              <div className={styles.se__right}>
                <UiAccountPanel key={session.id} accounts={session.accounts} user={session.name} beans={session.beans} region={session.region} />
              </div>
            </div>
          ) : (
            <form className={styles.se__login} onSubmit={submit}>
              <img src={logo} alt="" />
              <span className={styles.se__brand}>MAPLELINK</span>
              <label>{t("帳號", "账号", "Username")}</label>
              <input
                value={account}
                onChange={(e) => setAccount(e.target.value)}
                type="text"
                name="demo-account"
                placeholder={t("輸入你的帳號", "输入你的账号", "Enter your username")}
                {...NATIVE_INPUT}
              />
              <label>{t("密碼", "密码", "Password")}</label>
              <input
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="ml-secret"
                type="text"
                name="demo-secret"
                placeholder={t("輸入你的密碼", "输入你的密码", "Enter your password")}
                {...NATIVE_INPUT}
              />
              <button type="submit" className={clsx(styles.se__submit, "ml-grad", !canSubmit && styles["se__submit--off"])} disabled={!canSubmit}>
                {busy ? t("登入中...", "登录中...", "Signing in...") : t("登入", "登录", "Sign In")}
              </button>
              <button type="button" className={styles.se__back} onClick={() => setAdding(false)}>
                {t("← 返回帳號列表", "← 返回账号列表", "← Back to Accounts")}
              </button>
            </form>
          )}
        </div>
      </UiFrame>
      <UiCoach done={done} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
