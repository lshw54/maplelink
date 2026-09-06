import React from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoNodeErrors.module.css";

/**
 * The three faces of a bad connection node: a red "no OTP1 span" line above
 * the sign-in button, the website's verification alert that keeps coming
 * back, and a request timeout against tw.beanfun.com. Static on purpose.
 */
export default function DemoNodeErrors() {
  const t = useT();
  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={640} height={330}>
        <div className={clsx("ml", styles.ne)}>
          <div className="ml-glow"></div>

          <div className={styles.ne__form}>
            <div className={styles.ne__input}>••••••••</div>
            <p className={styles.ne__err}>Invalid credentials: failed to extract session key (no OTP1 span)</p>
            <div className={clsx(styles.ne__submit, "ml-grad")}>{t("登入", "登录", "Sign In")}</div>
            <span className={styles.ne__tag} style={{ top: 40, left: -30 }}>
              1
            </span>
          </div>

          <div className={styles.ne__alert}>
            <div className={styles["ne__alert-host"]}>tw.newlogin.beanfun.com {t("顯示", "显示", "says")}</div>
            <p>
              感謝您的配合，您的資料已驗證成功，
              <br />
              請您於五分鐘內再次輸入您的帳號密碼進行驗證登入。！
            </p>
            <div className={styles["ne__alert-btn"]}>{t("確定", "确定", "OK")}</div>
            <span className={styles.ne__tag} style={{ top: -10, left: -10 }}>
              2
            </span>
          </div>

          <div className={styles.ne__timeout}>
            <span className={styles.ne__tag} style={{ top: -10, left: -10 }}>
              3
            </span>
            Request timeout: https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0 (error sending request for url (…))
          </div>
        </div>
      </UiFrame>
      <UiCoach>
        {t("① 登入鈕上方「no OTP1 span」、② 官網驗證視窗按了確定又再彈出、③ Request timeout。三種都是節點問題。", "① 登录钮上方「no OTP1 span」、② 官网验证窗口点了确定又再弹出、③ Request timeout。三种都是节点问题。", '① "no OTP1 span" above the sign-in button; ② the website\'s verification alert that returns after OK; ③ Request timeout. All three point at the connection node.')}
      </UiCoach>
    </div>
  );
}
