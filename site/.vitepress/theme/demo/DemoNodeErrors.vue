<script setup lang="ts">
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";

/**
 * The three faces of a bad connection node: a red "no OTP1 span" line above
 * the sign-in button, the website's verification alert that keeps coming
 * back, and a request timeout against tw.beanfun.com. Static on purpose.
 */
const t = useT();
</script>

<template>
  <div class="demo">
    <UiFrame :width="640" :height="330">
      <div class="ml ne">
        <div class="ml-glow"></div>

        <div class="ne__form">
          <div class="ne__input">••••••••</div>
          <p class="ne__err">Invalid credentials: failed to extract session key (no OTP1 span)</p>
          <div class="ne__submit ml-grad">{{ t("登入", "登录", "Sign In") }}</div>
          <span class="ne__tag" style="top: 40px; left: -30px">1</span>
        </div>

        <div class="ne__alert">
          <div class="ne__alert-host">tw.newlogin.beanfun.com {{ t("顯示", "显示", "says") }}</div>
          <p>感謝您的配合，您的資料已驗證成功，<br />請您於五分鐘內再次輸入您的帳號密碼進行驗證登入。！</p>
          <div class="ne__alert-btn">{{ t("確定", "确定", "OK") }}</div>
          <span class="ne__tag" style="top: -10px; left: -10px">2</span>
        </div>

        <div class="ne__timeout">
          <span class="ne__tag" style="top: -10px; left: -10px">3</span>
          Request timeout: https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0 (error sending request for url (…))
        </div>
      </div>
    </UiFrame>
    <UiCoach>
      {{ t("① 登入鈕上方「no OTP1 span」、② 官網驗證視窗按了確定又再彈出、③ Request timeout。三種都是節點問題。", "① 登录钮上方「no OTP1 span」、② 官网验证窗口点了确定又再弹出、③ Request timeout。三种都是节点问题。", "① \"no OTP1 span\" above the sign-in button; ② the website's verification alert that returns after OK; ③ Request timeout. All three point at the connection node.") }}
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.ne {
  position: relative;
  width: 100%;
  height: 100%;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
}
.ne__tag {
  position: absolute;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: var(--ml-accent);
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  box-shadow: 0 0 0 3px rgba(232, 162, 58, 0.25);
}
.ne__form {
  position: absolute;
  top: 28px;
  left: 44px;
  width: 260px;
  display: flex;
  flex-direction: column;
}
.ne__input {
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 13px;
  color: var(--ml-dim);
}
.ne__err {
  margin: 12px 0 8px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--ml-danger);
}
.ne__submit {
  height: 42px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2px;
  box-shadow: 0 2px 10px var(--ml-glow);
}
/* Chromium's alert() sheet, light regardless of app theme */
.ne__alert {
  position: absolute;
  top: 24px;
  right: 24px;
  width: 300px;
  padding: 14px 16px 12px;
  border-radius: 8px;
  background: #fff;
  color: #202124;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  font-family: "Segoe UI", "Microsoft JhengHei UI", system-ui, sans-serif;
}
.ne__alert-host {
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 10px;
}
.ne__alert p {
  margin: 0 0 14px;
  font-size: 12px;
  line-height: 1.55;
}
.ne__alert-btn {
  margin-left: auto;
  width: max-content;
  padding: 6px 14px;
  border-radius: 4px;
  background: #1f1f1f;
  color: #fff;
  font-size: 12px;
}
.ne__timeout {
  position: absolute;
  left: 44px;
  right: 24px;
  bottom: 24px;
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid rgba(232, 17, 35, 0.35);
  background: rgba(232, 17, 35, 0.06);
  font-size: 12px;
  line-height: 1.5;
  color: var(--ml-danger);
  word-break: break-all;
}
</style>
