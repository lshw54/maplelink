<script setup lang="ts">
import { computed, ref } from "vue";
import { withBase } from "vitepress";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiTitlebar from "./UiTitlebar.vue";

/** The "rename to Beanfun.exe" prompt some users see on first launch. */
const t = useT();
const dontAsk = ref(false);
const working = ref(false);
const done = ref<"renamed" | "later" | null>(null);

function confirm() {
  working.value = true;
  setTimeout(() => {
    working.value = false;
    done.value = "renamed";
  }, 900);
}
function reset() {
  done.value = null;
  dontAsk.value = false;
}
const coach = computed(() => {
  if (done.value === "renamed") return t("程式改名為 Beanfun.exe 並重新啟動。功能完全一樣，自動更新也會保留新名稱。", "程序改名为 Beanfun.exe 并重新启动。功能完全一样，自动更新也会保留新名称。", "The app renames itself to Beanfun.exe and restarts. Nothing else changes; auto-update keeps the new name.");
  if (done.value === "later") return t("不改名也可以照常使用，只是部分加速器認不到。", "不改名也可以照常使用，只是部分加速器认不到。", "You can carry on without renaming; some accelerators just will not see the app.");
  return t("用網遊加速器就按「改名並重啟」；不用就按「稍後」。", "用网游加速器就点「改名并重启」；不用就点「稍后」。", "Using a game accelerator? Click Rename. Otherwise click Later.");
});
</script>

<template>
  <div class="demo">
    <UiFrame :width="520" :height="400">
      <div class="ml rn">
        <div class="ml-glow"></div>
        <UiTitlebar page="login" region="HK" />
        <div class="rn__bg"><img :src="withBase('/logo.png')" alt="" /><span>MAPLELINK</span></div>
        <div v-if="!done" class="rn__overlay">
          <div class="rn__card">
            <div class="rn__head"><span>🚀</span><b>{{ t("改名以配合加速器？", "改名以配合加速器？", "Rename for accelerator?") }}</b></div>
            <div class="rn__body">
              <p>{{ t("偵測到你的連線來自中國大陸。網遊加速器是按程式名稱來加速的。將本程式改名為 Beanfun.exe，加速器就能對應，登入與 reCAPTCHA 才會穩定。", "检测到你的连接来自中国大陆。网游加速器是按进程名称来加速的。将本程序改名为 Beanfun.exe，加速器就能对应，登录与 reCAPTCHA 才会稳定。", "Your connection looks like it is from mainland China. Game accelerators route traffic by process name. Renaming this app to Beanfun.exe lets the accelerator match it, so login and reCAPTCHA work reliably.") }}</p>
              <div class="rn__names"><span>MapleLink.exe</span><i>→</i><b>Beanfun.exe</b></div>
              <small>{{ t("程式會自行改名並重新啟動。之後自動更新會保留新名稱。", "程序会自行改名并重新启动。之后自动更新会保留新名称。", "The app will rename itself and restart. Auto-update keeps the new name.") }}</small>
              <label class="rn__check"><input v-model="dontAsk" type="checkbox" /> {{ t("不再提示", "不再提示", "Don't ask again") }}</label>
              <div class="rn__actions">
                <button class="rn__later" @click="done = 'later'">{{ t("稍後", "稍后", "Later") }}</button>
                <button class="rn__ok" :class="{ 'rn__ok--busy': working }" :disabled="working" @click="confirm">{{ working ? t("改名中…", "改名中…", "Renaming…") : t("改名並重啟", "改名并重启", "Rename & restart") }}</button>
              </div>
            </div>
          </div>
        </div>
        <div v-else-if="done === 'renamed'" class="rn__restart"><span class="rn__brand">BEANFUN</span><small>{{ t("重新啟動中…", "重新启动中…", "Restarting…") }}</small></div>
      </div>
    </UiFrame>
    <UiCoach :done="done === 'renamed'">
      {{ coach }}
      <template v-if="done" #action><button @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.rn {
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
.rn__bg,
.rn__restart {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.rn__bg img {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  box-shadow: 0 4px 20px var(--ml-glow);
}
.rn__bg span,
.rn__brand {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 5px;
  color: var(--ml-dim);
}
.rn__restart small {
  font-size: 11px;
  color: var(--ml-faint);
}
.rn__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
}
.rn__card {
  width: 318px;
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
  overflow: hidden;
}
.rn__head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  font-size: 14px;
  font-weight: 700;
}
.rn__body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 20px;
}
.rn__body p {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--ml-dim);
}
.rn__names {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #0f1114;
  font-family: var(--ml-mono);
  font-size: 12px;
}
.rn__names span {
  color: var(--ml-dim);
}
.rn__names i {
  color: var(--ml-accent);
  font-style: normal;
}
.rn__body small {
  font-size: 11px;
  line-height: 1.5;
  color: var(--ml-faint);
}
.rn__check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  cursor: pointer;
}
.rn__check input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: var(--ml-accent);
}
.rn__actions {
  display: flex;
  gap: 8px;
}
.rn__later,
.rn__ok {
  flex: 1;
  padding: 8px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
}
.rn__later {
  border: 1px solid var(--ml-border);
}
.rn__later:hover {
  background: var(--ml-surface-hover);
}
.rn__ok {
  background: var(--ml-accent);
  color: #fff;
}
.rn__ok--busy {
  opacity: 0.6;
  cursor: default;
}
</style>
