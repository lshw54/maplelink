<script setup lang="ts">
import { computed, ref } from "vue";
import { withBase } from "vitepress";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiTitlebar from "./UiTitlebar.vue";
import UiPlayColumn from "./UiPlayColumn.vue";
import UiAccountPanel from "./UiAccountPanel.vue";

/**
 * Several accounts open at once. "+" on the tab strip brings back the
 * sign-in page; signing in adds a tab. Each tab keeps its own accounts and
 * OTP panel; hovering a tab shows its close button.
 */
const t = useT();
interface Session {
  id: number;
  name: string;
  region: "HK" | "TW";
  beans: number;
  accounts: { initial: string; name: string }[];
}
const first = computed<Session>(() => ({
  id: 1,
  name: t("主帳", "主账", "Main"),
  region: "HK",
  beans: 120,
  accounts: [
    { initial: t("角", "角", "A"), name: t("角色一", "角色一", "Alpha") },
    { initial: t("角", "角", "B"), name: t("角色二", "角色二", "Bravo") },
  ],
}));
const sessions = ref<Session[]>([first.value]);
const active = ref(0);
const adding = ref(false);
const account = ref("");
const password = ref("");
const region = ref<"HK" | "TW">("TW");
const busy = ref(false);
let nextId = 2;

const session = computed(() => sessions.value[active.value]);
const canSubmit = computed(() => account.value.trim() !== "" && password.value.trim() !== "" && !busy.value);

function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  setTimeout(() => {
    const name = account.value.trim();
    sessions.value.push({
      id: nextId++,
      name,
      region: region.value,
      beans: region.value === "TW" ? 35 : 80,
      accounts: [
        { initial: name.charAt(0).toUpperCase(), name: t("角色一", "角色一", "Alpha") },
        { initial: name.charAt(0).toUpperCase(), name: t("練功號", "练功号", "Leveller") },
      ],
    });
    active.value = sessions.value.length - 1;
    adding.value = false;
    busy.value = false;
    account.value = "";
    password.value = "";
  }, 800);
}
function close(i: number) {
  sessions.value.splice(i, 1);
  if (active.value >= sessions.value.length) active.value = sessions.value.length - 1;
}
function reset() {
  sessions.value = [first.value];
  active.value = 0;
  adding.value = false;
}
const coach = computed(() => {
  if (adding.value) return t("這就是登入頁，只多了「返回帳號列表」。填好帳密按「登入」。", "这就是登录页，只多了「返回账号列表」。填好账密点「登录」。", "This is the sign-in page with one extra button, Back to Accounts. Fill in and sign in.");
  if (sessions.value.length >= 2) return t("每個分頁各有自己的帳號列表和 OTP。點分頁切換；滑到分頁上按 × 可登出該帳號。", "每个标签各有自己的账号列表和 OTP。点标签切换；移到标签上点 × 可登出该账号。", "Each tab has its own accounts and OTP. Click a tab to switch; hover one and press × to sign that account out.");
  return t("按分頁列右邊的「+」加入第二個帳號。", "点标签栏右边的「+」添加第二个账号。", "Click + at the end of the tab strip to add a second account.");
});
</script>

<template>
  <div class="demo">
    <UiFrame :width="760" :height="530">
      <div class="ml se">
        <div class="ml-glow"></div>
        <UiTitlebar :page="adding ? 'login' : 'main'" :region="adding ? region : session.region" @toggle-region="region = region === 'HK' ? 'TW' : 'HK'" />

        <div class="se__tabs">
          <button
            v-for="(s, i) in sessions"
            :key="s.id"
            class="se__tab"
            :class="{ 'se__tab--on': i === active && !adding }"
            @click="active = i; adding = false"
          >
            <i class="se__dot" :class="s.region === 'TW' ? 'se__dot--tw' : 'se__dot--hk'"></i>{{ s.name }}<small>{{ s.region }}</small>
            <span v-if="sessions.length > 1" class="se__close" :title="t('登出', '登出', 'Close')" @click.stop="close(i)">×</span>
          </button>
          <button class="se__tab se__tab--add" :class="{ 'ml-hint': sessions.length < 2 && !adding }" :title="t('新增帳號', '添加账号', 'Add Account')" @click="adding = true">+</button>
        </div>

        <div v-if="!adding" class="se__body">
          <div class="se__left"><UiPlayColumn :key="session.id" :can-classic="session.region === 'HK'" /></div>
          <div class="se__right"><UiAccountPanel :key="session.id" :accounts="session.accounts" :user="session.name" :beans="session.beans" :region="session.region" /></div>
        </div>

        <form v-else class="se__login" @submit.prevent="submit">
          <img :src="withBase('/logo.png')" alt="" />
          <span class="se__brand">MAPLELINK</span>
          <label>{{ t("帳號", "账号", "Username") }}</label>
          <input v-model="account" type="text" :placeholder="t('輸入你的帳號', '输入你的账号', 'Enter your username')" autocomplete="off" />
          <label>{{ t("密碼", "密码", "Password") }}</label>
          <input v-model="password" type="password" :placeholder="t('輸入你的密碼', '输入你的密码', 'Enter your password')" autocomplete="off" />
          <button type="submit" class="se__submit ml-grad" :class="{ 'se__submit--off': !canSubmit }" :disabled="!canSubmit">{{ busy ? t("登入中...", "登录中...", "Signing in...") : t("登入", "登录", "Sign In") }}</button>
          <button type="button" class="se__back" @click="adding = false">{{ t("← 返回帳號列表", "← 返回账号列表", "← Back to Accounts") }}</button>
        </form>
      </div>
    </UiFrame>
    <UiCoach :done="sessions.length >= 2 && !adding">
      {{ coach }}
      <template v-if="sessions.length >= 2 && !adding" #action><button @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.se {
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
.se__tabs {
  position: relative;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px 4px;
  border-bottom: 1px solid var(--ml-border);
}
.se__tab {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px 6px 0 0;
  font-size: 11px;
  color: var(--ml-dim);
}
.se__tab:hover {
  color: var(--ml-text);
}
.se__tab--on {
  background: var(--ml-surface);
  color: var(--ml-accent);
}
.se__tab--add {
  font-size: 12px;
  color: var(--ml-faint);
  border-radius: 4px;
}
.se__tab small {
  font-size: 9px;
  color: var(--ml-faint);
}
.se__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.se__dot--hk {
  background: var(--ml-green);
}
.se__dot--tw {
  background: #60a5fa;
}
.se__close {
  margin-left: 2px;
  padding: 0 3px;
  border-radius: 3px;
  color: var(--ml-faint);
  opacity: 0;
  transition: opacity 0.15s;
}
.se__tab:hover .se__close {
  opacity: 1;
}
.se__close:hover {
  color: var(--ml-accent);
}
.se__body {
  position: relative;
  display: flex;
  flex: 1;
  overflow: hidden;
}
.se__left {
  width: 40%;
}
.se__right {
  flex: 1;
  border-left: 1px solid var(--ml-border);
}
.se__login {
  position: relative;
  flex: 1;
  width: 300px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
}
.se__login img {
  width: 40px;
  height: 40px;
  margin: 0 auto 8px;
  border-radius: 10px;
  box-shadow: 0 4px 20px var(--ml-glow);
}
.se__brand {
  margin: 0 auto 20px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 5px;
  color: var(--ml-dim);
}
.se__login label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.se__login input {
  margin-bottom: 10px;
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 13px;
  color: var(--ml-text);
  outline: none;
}
.se__login input::placeholder {
  font-size: 12px;
  color: var(--ml-faint);
}
.se__login input:focus {
  border-color: var(--ml-accent);
}
.se__submit {
  height: 42px;
  border-radius: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2px;
  box-shadow: 0 2px 10px var(--ml-glow);
}
.se__submit--off {
  opacity: 0.4;
  cursor: default;
  box-shadow: none;
}
.se__back {
  margin-top: 12px;
  padding: 8px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  font-size: 12px;
  font-weight: 600;
  color: var(--ml-dim);
}
.se__back:hover {
  color: var(--ml-accent);
  border-color: var(--ml-accent);
}
</style>
