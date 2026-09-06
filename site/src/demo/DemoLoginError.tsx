import React from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoLoginError.module.css";

/**
 * The two ways a failed sign-in shows up: a red line above the sign-in
 * button, and a toast at the bottom right. Both carry the same two messages
 * whether the password was wrong or the connection went through a bad node,
 * so the guide has to tell people to check both. Static on purpose.
 */
export default function DemoLoginError() {
  const t = useT();
  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={520} height={250}>
        <div className={clsx("ml", styles.le)}>
          <div className="ml-glow"></div>
          <div className={styles.le__form}>
            <span className={styles.le__label}>{t("密碼", "密码", "Password")}</span>
            <div className={styles.le__input}>••••••••</div>
            <p className={styles.le__err}>Invalid credentials: login failed: no auth key in response</p>
            <div className={clsx(styles.le__submit, "ml-grad")}>{t("登入", "登录", "Sign In")}</div>
          </div>
          <div className={styles.le__toast}>
            <svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.7 7.3a1 1 0 00-1.4 1.4L8.6 10l-1.3 1.3a1 1 0 101.4 1.4L10 11.4l1.3 1.3a1 1 0 001.4-1.4L11.4 10l1.3-1.3a1 1 0 00-1.4-1.4L10 8.6 8.7 7.3z" clipRule="evenodd"></path>
            </svg>
            <span>missing akey in response URL</span>
            <b>×</b>
          </div>
          <span className={clsx(styles.le__tag, styles["le__tag--a"])}>1</span>
          <span className={clsx(styles.le__tag, styles["le__tag--b"])}>2</span>
        </div>
      </UiFrame>
      <UiCoach>
        {t("① 登入鈕上方的紅字，② 右下角的提示。兩個都會在密碼錯誤或連線節點有問題時出現。", "① 登录钮上方的红字，② 右下角的提示。两个都会在密码错误或连接节点有问题时出现。", "① The red line above the sign-in button; ② the toast at the bottom right. Both appear for a wrong password and for a bad connection node alike.")}
      </UiCoach>
    </div>
  );
}
