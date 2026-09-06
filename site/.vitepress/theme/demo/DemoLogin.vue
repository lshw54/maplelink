<script setup lang="ts">
import { computed, ref } from "vue";
import { withBase } from "vitepress";
import { useT } from "./i18n";
import { NATIVE_INPUT } from "./native";
import UiFrame from "./UiFrame.vue";
import UiTitlebar from "./UiTitlebar.vue";
import UiCoach from "./UiCoach.vue";
import { useLatestVersion } from "./release";

/**
 * The sign-in window (350×620), playable. Flip the region flag to see the TW
 * form gain its QR / GamaPass buttons, open the QR view, tick the boxes, and
 * press sign in. Nothing is sent anywhere; the fields accept any text.
 */
const t = useT();
const version = useLatestVersion();
const region = ref<"HK" | "TW">("HK");
const classic = ref(false);
const view = ref<"form" | "qr">("form");
const account = ref("");
const password = ref("");
const remember = ref(true);
const autoLogin = ref(false);
const cafe = ref(false);
const busy = ref(false);
const done = ref(false);

function toggleRegion() {
  region.value = region.value === "HK" ? "TW" : "HK";
  classic.value = false;
  view.value = "form";
  done.value = false;
}
function submit() {
  if (busy.value || !account.value.trim() || !password.value.trim()) return;
  busy.value = true;
  setTimeout(() => {
    busy.value = false;
    done.value = true;
  }, 900);
}
function reset() {
  done.value = false;
  account.value = "";
  password.value = "";
  view.value = "form";
}
const canSubmit = computed(() => account.value.trim() !== "" && password.value.trim() !== "" && !busy.value);
const coach = computed(() => {
  if (done.value) return t("登入成功後會進入主頁。", "登录成功后会进入主页。", "After signing in, the main window opens.");
  if (view.value === "qr") return t("真實程式會顯示 QR Code，用手機 beanfun! App 掃描。", "真实程序会显示 QR Code，用手机 beanfun! App 扫描。", "The real app shows a QR code here; scan it with the beanfun! app.");
  if (region.value === "HK")
    return t("右上角旗標是地區。填入帳號密碼，按「登入」試試。", "右上角旗标是地区。填入账号密码，点「登录」试试。", "The flag at the top right is the region. Fill in any account and password, then press sign in.");
  return t("TW 的登入鈕右邊多了 QR Code 和 GamaPass。按第一個圖示看看。", "TW 的登录钮右边多了 QR Code 和 GamaPass。点第一个图标看看。", "TW adds QR Code and GamaPass beside the sign-in button. Try the first icon.");
});
</script>

<template>
  <div class="demo">
    <UiFrame :width="350" :height="620">
      <div class="ml login">
        <div class="ml-glow"></div>
        <UiTitlebar page="login" :region="region" :classic="classic" :hint="done ? null : 'region'" @toggle-region="toggleRegion" @toggle-classic="classic = !classic" />

        <div class="login__body">
          <div class="login__logo">
            <img :src="withBase('/logo.png')" alt="" />
            <span>MAPLELINK</span>
          </div>

          <form v-if="view === 'form'" class="login__form" @submit.prevent="submit">
            <label class="login__label">{{ t("帳號", "账号", "Username") }}</label>
            <input v-model="account" class="login__input" type="text" name="demo-account" :placeholder="t('輸入你的帳號', '输入你的账号', 'Enter your username')" v-bind="NATIVE_INPUT" />
            <label class="login__label">{{ t("密碼", "密码", "Password") }}</label>
            <div class="login__pw">
              <input v-model="password" class="login__input ml-secret" type="text" name="demo-secret" :placeholder="t('輸入你的密碼', '输入你的密码', 'Enter your password')" v-bind="NATIVE_INPUT" />
              <span class="login__eye">👁</span>
            </div>
            <div class="login__opts">
              <label><input v-model="remember" type="checkbox" /> {{ t("記住密碼", "记住密码", "Remember password") }}</label>
              <label><input v-model="autoLogin" type="checkbox" /> {{ t("自動登入", "自动登录", "Auto login") }}</label>
              <span class="login__forgot">{{ t("忘記密碼", "忘记密码", "Forgot password") }}</span>
            </div>
            <label class="login__cafe" :class="{ 'login__cafe--on': cafe }">
              <input v-model="cafe" type="checkbox" />
              <span>
                <b>🖥️ {{ t("網咖模式", "网吧模式", "Café mode") }}</b>
                <small>{{ t("公用電腦適用，關閉時清除本機資料。", "公用电脑适用，关闭时清除本机数据。", "For shared PCs. Wipes local data on close.") }}</small>
              </span>
            </label>
            <div class="login__row">
              <button type="submit" class="login__submit ml-grad" :class="{ 'login__submit--off': !canSubmit }" :disabled="!canSubmit">
                {{ busy ? t("登入中...", "登录中...", "Signing in...") : t("登入", "登录", "Sign In") }}
              </button>
              <template v-if="region === 'TW'">
                <button type="button" class="login__sq" :class="{ 'ml-hint': !done }" title="QR Code" @click="view = 'qr'">
                  <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><rect x="1" y="1" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.5"></rect><rect x="3" y="3" width="2" height="2" fill="currentColor"></rect><rect x="11" y="1" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.5"></rect><rect x="13" y="3" width="2" height="2" fill="currentColor"></rect><rect x="1" y="11" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.5"></rect><rect x="3" y="13" width="2" height="2" fill="currentColor"></rect><rect x="11" y="11" width="2" height="2" fill="currentColor"></rect><rect x="15" y="11" width="2" height="2" fill="currentColor"></rect><rect x="11" y="15" width="2" height="2" fill="currentColor"></rect><rect x="15" y="15" width="2" height="2" fill="currentColor"></rect><rect x="13" y="13" width="2" height="2" fill="currentColor"></rect></svg>
                </button>
                <button type="button" class="login__sq" :title="t('GamaPass 登入', 'GamaPass 登录', 'GamaPass login')">
                  <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><path d="M9 1.5L2 5.5V12.5L9 16.5L16 12.5V5.5L9 1.5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"></path><path d="M9 8.5V16.5" stroke="currentColor" stroke-width="1.5"></path><path d="M2 5.5L9 9.5L16 5.5" stroke="currentColor" stroke-width="1.5"></path></svg>
                </button>
                <button type="button" class="login__sq" :title="t('網頁登入開機', '网页登录开机', 'Web launch')">
                  <svg width="18" height="18" viewBox="0 0 18 18" fill="none"><circle cx="9" cy="9" r="7.25" stroke="currentColor" stroke-width="1.5"></circle><path d="M2 9H16M9 2C11 4 11.5 7 11.5 9C11.5 11 11 14 9 16C7 14 6.5 11 6.5 9C6.5 7 7 4 9 2Z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"></path></svg>
                </button>
              </template>
            </div>
          </form>

          <div v-else class="login__qr">
            <div class="login__qr-title">{{ t("QR CODE 登入", "QR CODE 登入", "QR CODE LOGIN") }}</div>
            <div class="login__qr-sub">{{ t("請使用 Beanfun App 掃描 QR Code", "请使用 Beanfun App 扫描 QR Code", "Scan the QR code with the Beanfun app") }}</div>
            <div class="login__qr-box">
              <div class="login__qr-img">
                <svg viewBox="0 0 21 21" width="150" height="150" shape-rendering="crispEdges">
                  <rect width="21" height="21" fill="#fff"></rect>
                  <path fill="#111" d="M0 0h7v7H0zM1 1v5h5V1zM2 2h3v3H2zM14 0h7v7h-7zM15 1v5h5V1zM16 2h3v3h-3zM0 14h7v7H0zM1 15v5h5v-5zM2 16h3v3H2zM8 0h1v1H8zM10 0h1v2h-1zM12 1h1v1h-1zM8 2h2v1H8zM11 3h2v1h-2zM8 4h1v2H8zM10 5h1v1h-1zM12 5h1v1h-1zM0 8h1v1H0zM2 8h2v1H2zM5 8h1v1H5zM7 8h1v2H7zM9 8h1v1H9zM11 8h2v1h-2zM14 8h1v1h-1zM16 8h1v2h-1zM18 8h1v1h-1zM20 8h1v1h-1zM1 10h1v1H1zM3 10h1v1H3zM5 10h1v1H5zM8 10h2v1H8zM12 10h1v1h-1zM14 10h1v1h-1zM17 10h1v1h-1zM19 10h2v1h-2zM0 12h1v1H0zM2 12h1v1H2zM4 12h2v1H4zM7 12h1v1H7zM9 12h1v1H9zM11 12h1v1h-1zM13 12h1v1h-1zM15 12h2v1h-2zM18 12h1v1h-1zM20 12h1v1h-1zM8 14h1v1H8zM10 14h2v1h-2zM13 14h1v1h-1zM15 14h1v1h-1zM17 14h3v1h-3zM9 15h1v1H9zM12 15h1v1h-1zM14 15h1v1h-1zM16 15h1v1h-1zM20 15h1v1h-1zM8 16h1v1H8zM10 16h1v1h-1zM12 16h2v1h-2zM15 16h1v1h-1zM17 16h2v1h-2zM9 17h1v1H9zM11 17h1v1h-1zM13 17h1v1h-1zM16 17h1v1h-1zM19 17h2v1h-2zM8 18h1v1H8zM10 18h2v1h-2zM14 18h1v1h-1zM17 18h1v1h-1zM9 19h1v1H9zM12 19h2v1h-2zM15 19h1v1h-1zM18 19h1v1h-1zM20 19h1v1h-1zM8 20h1v1H8zM11 20h1v1h-1zM13 20h1v1h-1zM16 20h2v1h-2zM19 20h1v1h-1z"></path>
                </svg>
              </div>
              <div class="login__qr-actions"><span>⧉ {{ t("複製 QR", "复制 QR", "Copy QR") }}</span><span>⤢ {{ t("放大", "放大", "Enlarge") }}</span></div>
              <div class="login__qr-wait">{{ t("等待掃描中...", "等待扫描中...", "Waiting for scan...") }}<small>{{ t("有效期", "有效期", "Valid for") }}: 04:37</small></div>
            </div>
            <button type="button" class="login__back" @click="view = 'form'">{{ t("← 返回一般登入", "← 返回普通登入", "← Back to login") }}</button>
          </div>
        </div>

        <div class="login__foot">
          <span class="login__online"><i></i>ONLINE <small>42ms</small></span>
          <span class="login__direct">▶ {{ t("免登入啟動遊戲", "免登录启动游戏", "Launch game without login") }}</span>
          <span class="login__ver">MapleLink v{{ version ?? "…" }}</span>
        </div>
      </div>
    </UiFrame>
    <UiCoach :done="done">
      {{ coach }}
      <template v-if="done" #action><button @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.login {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
}
.login__body {
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 0 36px;
}
.login__logo {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-bottom: 24px;
}
.login__logo img {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  margin-bottom: 10px;
  box-shadow: 0 4px 20px var(--ml-glow);
}
.login__logo span {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 5px;
  color: var(--ml-dim);
}
.login__form {
  width: 100%;
  display: flex;
  flex-direction: column;
}
.login__label {
  margin-bottom: 4px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.login__input {
  width: 100%;
  margin-bottom: 12px;
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 13px;
  color: var(--ml-text);
  outline: none;
}
.login__input::placeholder {
  font-size: 12px;
  color: var(--ml-faint);
}
.login__input:focus {
  border-color: var(--ml-accent);
  box-shadow: 0 0 0 3px rgba(232, 162, 58, 0.08);
}
.login__pw {
  position: relative;
}
.login__eye {
  position: absolute;
  right: 12px;
  top: 11px;
  font-size: 12px;
  color: var(--ml-dim);
}
.login__opts {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  font-size: 12px;
  color: var(--ml-dim);
}
.login__opts label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.login__opts input,
.login__cafe input {
  width: 14px;
  height: 14px;
  accent-color: var(--ml-accent);
  margin: 0;
}
.login__forgot {
  margin-left: auto;
  color: var(--ml-accent);
}
.login__cafe {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  cursor: pointer;
}
.login__cafe--on {
  border-color: rgba(232, 162, 58, 0.4);
  background: rgba(232, 162, 58, 0.06);
}
.login__cafe input {
  margin-top: 2px;
}
.login__cafe b {
  display: block;
  font-size: 12px;
  font-weight: 600;
}
.login__cafe small {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.4;
  color: var(--ml-dim);
}
.login__row {
  display: flex;
  gap: 8px;
}
.login__submit {
  flex: 1;
  height: 42px;
  border-radius: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2px;
  box-shadow: 0 2px 10px var(--ml-glow);
}
.login__submit--off {
  opacity: 0.4;
  cursor: default;
  box-shadow: none;
}
.login__sq {
  width: 42px;
  height: 42px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  color: var(--ml-dim);
}
.login__sq:hover {
  border-color: var(--ml-accent);
  color: var(--ml-accent);
}
.login__qr {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.login__qr-title {
  font-size: 12px;
  letter-spacing: 4px;
  color: var(--ml-dim);
}
.login__qr-sub {
  margin: 6px 0 20px;
  font-size: 12px;
  letter-spacing: 0.5px;
  color: var(--ml-faint);
}
.login__qr-box {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
  border-radius: 14px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
}
.login__qr-img {
  display: flex;
  padding: 8px;
  border-radius: 12px;
  background: #fff;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
}
.login__qr-actions {
  display: flex;
  gap: 8px;
  font-size: 11px;
  color: var(--ml-dim);
}
.login__qr-actions span {
  padding: 4px 8px;
  border-radius: 6px;
}
.login__qr-wait {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  font-size: 12px;
  letter-spacing: 1px;
  color: var(--ml-dim);
}
.login__qr-wait small {
  letter-spacing: 0;
  color: var(--ml-faint);
}
.login__back {
  width: 100%;
  margin-top: 12px;
  padding: 8px 14px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  font-size: 12px;
  font-weight: 600;
  color: var(--ml-dim);
}
.login__back:hover {
  color: var(--ml-accent);
  border-color: var(--ml-accent);
}
.login__foot {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding-bottom: 8px;
}
.login__online {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: var(--ml-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  color: var(--ml-dim);
}
.login__online i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ml-green);
  box-shadow: 0 0 6px rgba(74, 222, 128, 0.4);
}
.login__online small {
  font-weight: 400;
  letter-spacing: 0;
  color: var(--ml-faint);
}
.login__direct {
  padding: 4px 12px;
  font-size: 11px;
  color: var(--ml-dim);
}
.login__ver {
  font-family: var(--ml-mono);
  font-size: 10px;
  letter-spacing: 1px;
  color: var(--ml-faint);
}
</style>
