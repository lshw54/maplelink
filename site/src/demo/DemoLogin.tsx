import React, { useState } from "react";
import clsx from "clsx";
import useBaseUrl from "@docusaurus/useBaseUrl";
import { useT } from "./i18n";
import { NATIVE_INPUT } from "./native";
import UiFrame from "./UiFrame";
import UiTitlebar from "./UiTitlebar";
import UiCoach from "./UiCoach";
import { useLatestVersion } from "./release";
import styles from "./DemoLogin.module.css";

/**
 * The sign-in window (350×620), playable. Flip the region flag to see the TW
 * form gain its QR / GamaPass buttons, open the QR view, tick the boxes, and
 * press sign in. Nothing is sent anywhere; the fields accept any text.
 */
export default function DemoLogin() {
  const t = useT();
  const version = useLatestVersion();
  const logo = useBaseUrl("/logo.png");
  const [region, setRegion] = useState<"HK" | "TW">("HK");
  const [classic, setClassic] = useState(false);
  const [view, setView] = useState<"form" | "qr">("form");
  const [account, setAccount] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(true);
  const [autoLogin, setAutoLogin] = useState(false);
  const [cafe, setCafe] = useState(false);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  function toggleRegion() {
    setRegion((r) => (r === "HK" ? "TW" : "HK"));
    setClassic(false);
    setView("form");
    setDone(false);
  }
  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (busy || !account.trim() || !password.trim()) return;
    setBusy(true);
    setTimeout(() => {
      setBusy(false);
      setDone(true);
    }, 900);
  }
  function reset() {
    setDone(false);
    setAccount("");
    setPassword("");
    setView("form");
  }
  const canSubmit = account.trim() !== "" && password.trim() !== "" && !busy;
  const coach = (() => {
    if (done) return t("登入成功後會進入主頁。", "登录成功后会进入主页。", "After signing in, the main window opens.");
    if (view === "qr") return t("真實程式會顯示 QR Code，用手機 beanfun! App 掃描。", "真实程序会显示 QR Code，用手机 beanfun! App 扫描。", "The real app shows a QR code here; scan it with the beanfun! app.");
    if (region === "HK")
      return t("右上角旗標是地區。填入帳號密碼，按「登入」試試。", "右上角旗标是地区。填入账号密码，点「登录」试试。", "The flag at the top right is the region. Fill in any account and password, then press sign in.");
    return t("TW 的登入鈕右邊多了 QR Code 和 GamaPass。按第一個圖示看看。", "TW 的登录钮右边多了 QR Code 和 GamaPass。点第一个图标看看。", "TW adds QR Code and GamaPass beside the sign-in button. Try the first icon.");
  })();

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={350} height={620}>
        <div className={clsx("ml", styles.login)}>
          <div className="ml-glow"></div>
          <UiTitlebar page="login" region={region} classic={classic} hint={done ? null : "region"} onToggleRegion={toggleRegion} onToggleClassic={() => setClassic((c) => !c)} />

          <div className={styles.login__body}>
            <div className={styles.login__logo}>
              <img src={logo} alt="" />
              <span>MAPLELINK</span>
            </div>

            {view === "form" ? (
              <form className={styles.login__form} onSubmit={submit}>
                <label className={styles.login__label}>{t("帳號", "账号", "Username")}</label>
                <input value={account} onChange={(e) => setAccount(e.target.value)} className={styles.login__input} type="text" name="demo-account" placeholder={t("輸入你的帳號", "输入你的账号", "Enter your username")} {...NATIVE_INPUT} />
                <label className={styles.login__label}>{t("密碼", "密码", "Password")}</label>
                <div className={styles.login__pw}>
                  <input value={password} onChange={(e) => setPassword(e.target.value)} className={clsx(styles.login__input, "ml-secret")} type="text" name="demo-secret" placeholder={t("輸入你的密碼", "输入你的密码", "Enter your password")} {...NATIVE_INPUT} />
                  <span className={styles.login__eye}>👁</span>
                </div>
                <div className={styles.login__opts}>
                  <label>
                    <input checked={remember} onChange={(e) => setRemember(e.target.checked)} type="checkbox" /> {t("記住密碼", "记住密码", "Remember password")}
                  </label>
                  <label>
                    <input checked={autoLogin} onChange={(e) => setAutoLogin(e.target.checked)} type="checkbox" /> {t("自動登入", "自动登录", "Auto login")}
                  </label>
                  <span className={styles.login__forgot}>{t("忘記密碼", "忘记密码", "Forgot password")}</span>
                </div>
                <label className={clsx(styles.login__cafe, cafe && styles["login__cafe--on"])}>
                  <input checked={cafe} onChange={(e) => setCafe(e.target.checked)} type="checkbox" />
                  <span>
                    <b>🖥️ {t("網咖模式", "网吧模式", "Café mode")}</b>
                    <small>{t("公用電腦適用，關閉時清除本機資料。", "公用电脑适用，关闭时清除本机数据。", "For shared PCs. Wipes local data on close.")}</small>
                  </span>
                </label>
                <div className={styles.login__row}>
                  <button type="submit" className={clsx(styles.login__submit, "ml-grad", !canSubmit && styles["login__submit--off"])} disabled={!canSubmit}>
                    {busy ? t("登入中...", "登录中...", "Signing in...") : t("登入", "登录", "Sign In")}
                  </button>
                  {region === "TW" && (
                    <>
                      <button type="button" className={clsx(styles.login__sq, !done && "ml-hint")} title="QR Code" onClick={() => setView("qr")}>
                        <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                          <rect x="1" y="1" width="6" height="6" rx="1" stroke="currentColor" strokeWidth="1.5"></rect>
                          <rect x="3" y="3" width="2" height="2" fill="currentColor"></rect>
                          <rect x="11" y="1" width="6" height="6" rx="1" stroke="currentColor" strokeWidth="1.5"></rect>
                          <rect x="13" y="3" width="2" height="2" fill="currentColor"></rect>
                          <rect x="1" y="11" width="6" height="6" rx="1" stroke="currentColor" strokeWidth="1.5"></rect>
                          <rect x="3" y="13" width="2" height="2" fill="currentColor"></rect>
                          <rect x="11" y="11" width="2" height="2" fill="currentColor"></rect>
                          <rect x="15" y="11" width="2" height="2" fill="currentColor"></rect>
                          <rect x="11" y="15" width="2" height="2" fill="currentColor"></rect>
                          <rect x="15" y="15" width="2" height="2" fill="currentColor"></rect>
                          <rect x="13" y="13" width="2" height="2" fill="currentColor"></rect>
                        </svg>
                      </button>
                      <button type="button" className={styles.login__sq} title={t("GamaPass 登入", "GamaPass 登录", "GamaPass login")}>
                        <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                          <path d="M9 1.5L2 5.5V12.5L9 16.5L16 12.5V5.5L9 1.5Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round"></path>
                          <path d="M9 8.5V16.5" stroke="currentColor" strokeWidth="1.5"></path>
                          <path d="M2 5.5L9 9.5L16 5.5" stroke="currentColor" strokeWidth="1.5"></path>
                        </svg>
                      </button>
                      <button type="button" className={styles.login__sq} title={t("網頁登入開機", "网页登录开机", "Web launch")}>
                        <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                          <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5"></circle>
                          <path d="M2 9H16M9 2C11 4 11.5 7 11.5 9C11.5 11 11 14 9 16C7 14 6.5 11 6.5 9C6.5 7 7 4 9 2Z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round"></path>
                        </svg>
                      </button>
                    </>
                  )}
                </div>
              </form>
            ) : (
              <div className={styles.login__qr}>
                <div className={styles["login__qr-title"]}>{t("QR CODE 登入", "QR CODE 登入", "QR CODE LOGIN")}</div>
                <div className={styles["login__qr-sub"]}>{t("請使用 Beanfun App 掃描 QR Code", "请使用 Beanfun App 扫描 QR Code", "Scan the QR code with the Beanfun app")}</div>
                <div className={styles["login__qr-box"]}>
                  <div className={styles["login__qr-img"]}>
                    <svg viewBox="0 0 21 21" width="150" height="150" shapeRendering="crispEdges">
                      <rect width="21" height="21" fill="#fff"></rect>
                      <path
                        fill="#111"
                        d="M0 0h7v7H0zM1 1v5h5V1zM2 2h3v3H2zM14 0h7v7h-7zM15 1v5h5V1zM16 2h3v3h-3zM0 14h7v7H0zM1 15v5h5v-5zM2 16h3v3H2zM8 0h1v1H8zM10 0h1v2h-1zM12 1h1v1h-1zM8 2h2v1H8zM11 3h2v1h-2zM8 4h1v2H8zM10 5h1v1h-1zM12 5h1v1h-1zM0 8h1v1H0zM2 8h2v1H2zM5 8h1v1H5zM7 8h1v2H7zM9 8h1v1H9zM11 8h2v1h-2zM14 8h1v1h-1zM16 8h1v2h-1zM18 8h1v1h-1zM20 8h1v1h-1zM1 10h1v1H1zM3 10h1v1H3zM5 10h1v1H5zM8 10h2v1H8zM12 10h1v1h-1zM14 10h1v1h-1zM17 10h1v1h-1zM19 10h2v1h-2zM0 12h1v1H0zM2 12h1v1H2zM4 12h2v1H4zM7 12h1v1H7zM9 12h1v1H9zM11 12h1v1h-1zM13 12h1v1h-1zM15 12h2v1h-2zM18 12h1v1h-1zM20 12h1v1h-1zM8 14h1v1H8zM10 14h2v1h-2zM13 14h1v1h-1zM15 14h1v1h-1zM17 14h3v1h-3zM9 15h1v1H9zM12 15h1v1h-1zM14 15h1v1h-1zM16 15h1v1h-1zM20 15h1v1h-1zM8 16h1v1H8zM10 16h1v1h-1zM12 16h2v1h-2zM15 16h1v1h-1zM17 16h2v1h-2zM9 17h1v1H9zM11 17h1v1h-1zM13 17h1v1h-1zM16 17h1v1h-1zM19 17h2v1h-2zM8 18h1v1H8zM10 18h2v1h-2zM14 18h1v1h-1zM17 18h1v1h-1zM9 19h1v1H9zM12 19h2v1h-2zM15 19h1v1h-1zM18 19h1v1h-1zM20 19h1v1h-1zM8 20h1v1H8zM11 20h1v1h-1zM13 20h1v1h-1zM16 20h2v1h-2zM19 20h1v1h-1z"
                      ></path>
                    </svg>
                  </div>
                  <div className={styles["login__qr-actions"]}>
                    <span>⧉ {t("複製 QR", "复制 QR", "Copy QR")}</span>
                    <span>⤢ {t("放大", "放大", "Enlarge")}</span>
                  </div>
                  <div className={styles["login__qr-wait"]}>
                    {t("等待掃描中...", "等待扫描中...", "Waiting for scan...")}
                    <small>{t("有效期", "有效期", "Valid for")}: 04:37</small>
                  </div>
                </div>
                <button type="button" className={styles.login__back} onClick={() => setView("form")}>
                  {t("← 返回一般登入", "← 返回普通登入", "← Back to login")}
                </button>
              </div>
            )}
          </div>

          <div className={styles.login__foot}>
            <span className={styles.login__online}>
              <i></i>ONLINE <small>42ms</small>
            </span>
            <span className={styles.login__direct}>▶ {t("免登入啟動遊戲", "免登录启动游戏", "Launch game without login")}</span>
            <span className={styles.login__ver}>MapleLink v{version ?? "…"}</span>
          </div>
        </div>
      </UiFrame>
      <UiCoach done={done} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
