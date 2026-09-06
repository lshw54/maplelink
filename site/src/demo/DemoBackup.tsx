import React, { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import { NATIVE_INPUT } from "./native";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import UiToolbox from "./UiToolbox";
import styles from "./DemoBackup.module.css";

/**
 * Toolbox → Account Manager, with the Export / Import pair. Export opens the
 * real dialog's layout: a warning, an encrypt tick box, a password field.
 */
export default function DemoBackup() {
  const t = useT();
  const [tab, setTab] = useState("account_manager");
  const [dialog, setDialog] = useState<"none" | "export">("none");
  const [encrypt, setEncrypt] = useState(true);
  const [pass, setPass] = useState("");
  const [toast, setToast] = useState(false);
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  function doExport() {
    setDialog("none");
    setToast(true);
    setDone(true);
    timer.current = setTimeout(() => setToast(false), 1800);
  }
  function reset() {
    setDone(false);
    setPass("");
    setDialog("none");
    setTab("account_manager");
  }
  const coach = (() => {
    if (done) return t("備份檔存到你選的位置。在新電腦按「匯入資料」選這個檔就可以。", "备份文件存到你选的位置。在新电脑点「导入数据」选这个文件就可以。", "The backup is saved where you chose. On the new PC, click Import data and pick that file.");
    if (dialog === "export") return t("建議勾選加密並設密碼，明文檔任何人都讀得到密碼。然後按「匯出」。", "建议勾选加密并设密码，明文文件任何人都能读到密码。然后点「导出」。", "Tick encrypt and set a password; a plain file exposes every password. Then click Export.");
    if (tab !== "account_manager") return t("按左邊的「帳號管理」。", "点左边的「账号管理」。", "Click Account Manager on the left.");
    return t("右上角按「匯出資料」。", "右上角点「导出数据」。", "Click Export data at the top right.");
  })();

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={750} height={490}>
        <UiToolbox tab={tab} onTabChange={setTab}>
          {tab === "account_manager" ? (
            <div className={styles.am}>
              <div className={styles.am__head}>
                <span className={styles.am__title}>{t("已儲存帳號", "已保存账号", "Saved Accounts")}</span>
                <span className={styles.am__spacer}></span>
                <button className={clsx(styles.am__io, !done && dialog === "none" && "ml-hint")} onClick={() => setDialog("export")}>
                  ⤓ {t("匯出資料", "导出数据", "Export data")}
                </button>
                <button className={styles.am__io}>⤒ {t("匯入資料", "导入数据", "Import data")}</button>
              </div>
              <div className={styles.am__list}>
                <div className={styles.am__row}>
                  <span className={clsx(styles.am__avatar, "ml-grad")}>{t("主", "主", "M")}</span>
                  <span className={styles.am__name}>{t("主帳", "主账", "Main")}</span>
                  <span className={styles.am__pill}>HK</span>
                  <span className={clsx(styles.am__pill, styles["am__pill--ok"])}>{t("已儲存密碼", "已保存密码", "Password Saved")}</span>
                  <span className={styles.am__chev}>▾</span>
                </div>
                <div className={styles.am__row}>
                  <span className={clsx(styles.am__avatar, "ml-grad")}>{t("台", "台", "T")}</span>
                  <span className={styles.am__name}>{t("台服", "台服", "TW alt")}</span>
                  <span className={styles.am__pill}>TW</span>
                  <span className={clsx(styles.am__pill, styles["am__pill--ok"])}>{t("已儲存密碼", "已保存密码", "Password Saved")}</span>
                  <span className={styles.am__chev}>▾</span>
                </div>
              </div>
            </div>
          ) : (
            <div className={styles.am__empty}>{t("此示範只包含「帳號管理」。", "此示范只包含「账号管理」。", "This demo covers Account Manager only.")}</div>
          )}

          {dialog === "export" && (
            <div className={styles.am__overlay}>
              <div className={styles.am__dlg}>
                <div className={styles["am__dlg-head"]}>
                  {t("匯出資料", "导出数据", "Export data")}
                  <button onClick={() => setDialog("none")}>✕</button>
                </div>
                <p className={styles.am__warn}>{t("會匯出所有已儲存帳號（含密碼）與自訂設定。明文檔案任何人都讀得到密碼，請妥善保管；建議勾選加密。", "会导出所有已保存账号（含密码）与自定义设置。明文文件任何人都能读到密码，请妥善保管；建议勾选加密。", "Exports all saved accounts (including passwords) and customizations. A plaintext file exposes passwords to anyone who opens it. Keep it safe, or tick encrypt.")}</p>
                <label className={styles.am__check}>
                  <input checked={encrypt} onChange={(e) => setEncrypt(e.target.checked)} type="checkbox" /> {t("用密碼加密（AES-256）", "用密码加密（AES-256）", "Encrypt with a password (AES-256)")}
                </label>
                {encrypt && (
                  <input
                    value={pass}
                    onChange={(e) => setPass(e.target.value)}
                    className={clsx(styles.am__input, "ml-secret")}
                    type="text"
                    name="demo-secret"
                    placeholder={t("輸入密碼", "输入密码", "Enter password")}
                    {...NATIVE_INPUT}
                  />
                )}
                <div className={styles["am__dlg-actions"]}>
                  <button className={styles.am__cancel} onClick={() => setDialog("none")}>
                    {t("取消", "取消", "Cancel")}
                  </button>
                  <button
                    className={clsx(styles.am__ok, encrypt && pass.length < 4 && styles["am__ok--off"], (!encrypt || pass.length >= 4) && "ml-hint")}
                    disabled={encrypt && pass.length < 4}
                    onClick={doExport}
                  >
                    {t("匯出", "导出", "Export")}
                  </button>
                </div>
              </div>
            </div>
          )}
          <div className={clsx(styles.am__toast, toast && styles["am__toast--show"])}>✓ {t("已匯出", "已导出", "Exported")}</div>
        </UiToolbox>
      </UiFrame>
      <UiCoach done={done} action={done ? <button onClick={reset}>{t("再試一次", "再试一次", "Try again")}</button> : undefined}>
        {coach}
      </UiCoach>
    </div>
  );
}
