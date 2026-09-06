<script setup lang="ts">
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";

/**
 * The two ways a failed sign-in shows up: a red line above the sign-in
 * button, and a toast at the bottom right. Both carry the same two messages
 * whether the password was wrong or the connection went through a bad node,
 * so the guide has to tell people to check both. Static on purpose.
 */
const t = useT();
</script>

<template>
  <div class="demo">
    <UiFrame :width="520" :height="250">
      <div class="ml le">
        <div class="ml-glow"></div>
        <div class="le__form">
          <span class="le__label">{{ t("密碼", "密码", "Password") }}</span>
          <div class="le__input">••••••••</div>
          <p class="le__err">Invalid credentials: login failed: no auth key in response</p>
          <div class="le__submit ml-grad">{{ t("登入", "登录", "Sign In") }}</div>
        </div>
        <div class="le__toast">
          <svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.7 7.3a1 1 0 00-1.4 1.4L8.6 10l-1.3 1.3a1 1 0 101.4 1.4L10 11.4l1.3 1.3a1 1 0 001.4-1.4L11.4 10l1.3-1.3a1 1 0 00-1.4-1.4L10 8.6 8.7 7.3z" clip-rule="evenodd"></path></svg>
          <span>missing akey in response URL</span>
          <b>×</b>
        </div>
        <span class="le__tag le__tag--a">1</span>
        <span class="le__tag le__tag--b">2</span>
      </div>
    </UiFrame>
    <UiCoach>
      {{ t("① 登入鈕上方的紅字，② 右下角的提示。兩個都會在密碼錯誤或連線節點有問題時出現。", "① 登录钮上方的红字，② 右下角的提示。两个都会在密码错误或连接节点有问题时出现。", "① The red line above the sign-in button; ② the toast at the bottom right. Both appear for a wrong password and for a bad connection node alike.") }}
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.le {
  position: relative;
  width: 100%;
  height: 100%;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
}
.le__form {
  position: absolute;
  top: 24px;
  left: 36px;
  width: 300px;
  display: flex;
  flex-direction: column;
}
.le__label {
  margin-bottom: 4px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.le__input {
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 13px;
  color: var(--ml-dim);
}
.le__err {
  margin: 12px 0 8px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--ml-danger);
}
.le__submit {
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
.le__toast {
  position: absolute;
  right: 12px;
  bottom: 32px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  max-width: 200px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(232, 17, 35, 0.4);
  background: var(--ml-bg);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
  font-size: 12px;
  line-height: 1.4;
}
.le__toast svg {
  flex: none;
  margin-top: 2px;
  color: var(--ml-danger);
}
.le__toast b {
  flex: none;
  color: var(--ml-dim);
  font-weight: 400;
}
.le__tag {
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
.le__tag--a {
  top: 96px;
  left: 8px;
}
.le__tag--b {
  right: 224px;
  bottom: 44px;
}
</style>
