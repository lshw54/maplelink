import React, { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import { NATIVE_INPUT } from "./native";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiToolbox from "./UiToolbox";
import styles from "./DemoGamaPass.module.css";

/**
 * Toolbox → GamaPass: pick Bitwarden as the passkey source (which installs
 * it), then open and sign in to the vault. From there GamaPass passkeys are
 * answered by Bitwarden inside the GamaPass window.
 */
export default function DemoGamaPass() {
  const t = useT();
  const [tab, setTab] = useState("account_manager");
  const [source, setSource] = useState<"windows" | "bitwarden">("windows");
  const [installing, setInstalling] = useState(false);
  const [installed, setInstalled] = useState(false);
  const [vault, setVault] = useState(false);
  const [pass, setPass] = useState("");
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  function choose(next: "windows" | "bitwarden") {
    if (next === "windows") {
      setSource("windows");
      return;
    }
    if (installed) {
      setSource("bitwarden");
      return;
    }
    setInstalling(true);
    timer.current = setTimeout(() => {
      setInstalling(false);
      setInstalled(true);
      setSource("bitwarden");
    }, 1400);
  }
  function signIn() {
    setVault(false);
    setPass("");
    setDone(true);
  }
  function reset() {
    setTab("account_manager");
    setSource("windows");
    setInstalling(false);
    setInstalled(false);
    setVault(false);
    setPass("");
    setDone(false);
  }
  const coach = (() => {
    if (done) return t("之後 GamaPass 的 Passkey 由 Bitwarden 在 GamaPass 視窗內回應；保管庫鎖定時會彈出它的解鎖視窗。", "之后 GamaPass 的 Passkey 由 Bitwarden 在 GamaPass 窗口内响应；保管库锁定时会弹出它的解锁窗口。", "From now on Bitwarden answers GamaPass passkeys inside the GamaPass window; a locked vault pops its own unlock window.");
    if (vault) return t("這是 Bitwarden 自己的視窗：輸入主密碼登入保管庫。", "这是 Bitwarden 自己的窗口：输入主密码登录保管库。", "This is Bitwarden's own window: enter the master password to sign in to the vault.");
    if (tab !== "gamapass") return t("按左邊的「GamaPass」。", "点左边的「GamaPass」。", "Click GamaPass on the left.");
    if (installing) return t("下載並檢查 Bitwarden 的擴充功能……", "下载并检查 Bitwarden 的扩展……", "Downloading and checking the Bitwarden extension…");
    if (!installed) return t("來源選「Bitwarden」，程式會自動下載它的擴充功能。", "来源选「Bitwarden」，程序会自动下载它的扩展。", "Pick Bitwarden as the source; the app downloads its extension.");
    return t("第一次先按「開啟並登入」登入保管庫。", "第一次先点「打开并登录」登录保管库。", "Press Open and sign in once to sign in to the vault.");
  })();

  const windowsLabel = t("Windows", "Windows", "Windows");

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={750} height={490}>
        <UiToolbox tab={tab} onTabChange={setTab}>
          {tab === "gamapass" ? (
            <div className={styles.gp}>
              <p className={styles.gp__intro}>{t("選密碼管理器後，GamaPass 的 Passkey 由它的擴充功能回應；Windows 則用系統內建提示。下次開啟 GamaPass 生效。", "选密码管理器后，GamaPass 的 Passkey 由它的扩展响应；Windows 则用系统内置提示。下次打开 GamaPass 生效。", "With a password manager, its extension answers GamaPass passkeys; Windows uses the built-in prompt. Applies to the next GamaPass window.")}</p>
              <div className={styles.gp__title}>GamaPass</div>
              <div className={styles.gp__card}>
                <div className={styles.gp__row}>
                  <div className={styles.gp__label}>
                    <div>{t("GamaPass Passkey 來源", "GamaPass Passkey 来源", "GamaPass passkey source")}</div>
                    <div className={styles.gp__hint}>{t("1Password、Proton Pass、NordPass 開發中。", "1Password、Proton Pass、NordPass 开发中。", "1Password, Proton Pass and NordPass are in development.")}</div>
                  </div>
                  <div className={styles.gp__seg}>
                    <button className={clsx(styles.gp__segbtn, source === "windows" && !installing && styles["gp__segbtn--on"])} onClick={() => choose("windows")}>
                      {windowsLabel}
                    </button>
                    <button
                      className={clsx(styles.gp__segbtn, (source === "bitwarden" || installing) && styles["gp__segbtn--on"], !installed && !installing && "ml-hint")}
                      onClick={() => choose("bitwarden")}
                    >
                      Bitwarden
                    </button>
                  </div>
                </div>
                <div className={styles.gp__row}>
                  <div className={styles.gp__label}>
                    <div>Bitwarden</div>
                    <div className={styles.gp__hint}>
                      {installed
                        ? `2026.9.2 · ${t("已安裝", "已安装", "installed")}`
                        : installing
                          ? t("下載中…", "下载中…", "downloading…")
                          : t("未安裝", "未安装", "not installed")}
                    </div>
                  </div>
                  {installed && (
                    <div className={styles.gp__actions}>
                      <button className={clsx(styles.gp__btn, !done && "ml-hint")} onClick={() => setVault(true)}>
                        {t("開啟並登入", "打开并登录", "Open and sign in")}
                      </button>
                      <button className={clsx(styles.gp__btn, styles["gp__btn--danger"])}>{t("移除", "移除", "Remove")}</button>
                    </div>
                  )}
                </div>
                {installed && (
                  <div className={styles.gp__note}>{t("首次使用請先「開啟並登入」。登入時沒有彈出確認視窗，多半是保管庫已鎖定。擴充功能更新後需重新登入保管庫。", "首次使用请先「打开并登录」。登录时没有弹出确认窗口，多半是保管库已锁定。扩展更新后需重新登录保管库。", "Press \"Open and sign in\" before the first login. No confirmation window usually means the vault is locked. Sign in again after the extension updates.")}</div>
                )}
              </div>
            </div>
          ) : (
            <div className={styles.gp__empty}>{t("此示範只包含「GamaPass」。", "此示范只包含「GamaPass」。", "This demo covers GamaPass only.")}</div>
          )}

          {vault && (
            <div className={styles.gp__overlay}>
              <div className={styles.gp__vault}>
                <div className={styles["gp__vault-head"]}>
                  <span>🛡 Bitwarden</span>
                  <button onClick={() => setVault(false)}>✕</button>
                </div>
                <div className={styles["gp__vault-title"]}>{t("登入保管庫", "登录保管库", "Log in to your vault")}</div>
                <div className={styles["gp__vault-mail"]}>player@example.com</div>
                <input
                  value={pass}
                  onChange={(e) => setPass(e.target.value)}
                  className={clsx(styles.gp__input, "ml-secret")}
                  type="text"
                  name="demo-secret"
                  placeholder={t("主密碼", "主密码", "Master password")}
                  {...NATIVE_INPUT}
                />
                <button className={clsx(styles.gp__ok, pass.length < 4 && styles["gp__ok--off"], pass.length >= 4 && "ml-hint")} disabled={pass.length < 4} onClick={signIn}>
                  {t("登入", "登录", "Log in")}
                </button>
              </div>
            </div>
          )}
        </UiToolbox>
      </UiFrame>
      <UiCoach done={done} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
