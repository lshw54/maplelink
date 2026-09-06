<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./demo/i18n";
import UiFrame from "./demo/UiFrame.vue";
import UiTitlebar from "./demo/UiTitlebar.vue";
import UiPlayColumn from "./demo/UiPlayColumn.vue";
import UiAccountPanel from "./demo/UiAccountPanel.vue";

/**
 * The launcher's main window (760×530), playable, for the landing page. It is
 * assembled from the same pieces the guide embeds one at a time. A coach bar
 * at the bottom walks a visitor through the three clicks the real app takes;
 * every control works in any order. Account names are examples.
 */
const t = useT();

interface Session {
  id: string;
  name: string;
  region: "HK" | "TW";
  beans: number;
  accounts: { initial: string; name: string }[];
}
const sessions = computed<Session[]>(() => [
  {
    id: "hk",
    name: t("主帳", "主账", "Main"),
    region: "HK",
    beans: 120,
    accounts: [
      { initial: t("角", "角", "A"), name: t("角色一", "角色一", "Alpha") },
      { initial: t("角", "角", "B"), name: t("角色二", "角色二", "Bravo") },
      { initial: t("倉", "仓", "M"), name: t("倉庫號", "仓库号", "Mule") },
      { initial: t("小", "小", "S"), name: t("小號", "小号", "Spare") },
    ],
  },
  {
    id: "tw",
    name: t("台服", "台服", "TW alt"),
    region: "TW",
    beans: 35,
    accounts: [
      { initial: t("台", "台", "T"), name: t("台服主號", "台服主号", "TW main") },
      { initial: t("練", "练", "L"), name: t("練功號", "练功号", "Leveller") },
    ],
  },
]);

const active = ref(0);
const session = computed(() => sessions.value[active.value]);
/** 0 pick an account · 1 fetch an OTP · 2 launch · 3 done */
const step = ref(0);
const key = ref(0);
const play = ref<InstanceType<typeof UiPlayColumn> | null>(null);

function pickSession(i: number) {
  if (i === active.value) return;
  active.value = i;
  key.value++;
  play.value?.reset();
}
function reset() {
  active.value = 0;
  step.value = 0;
  key.value++;
  play.value?.reset();
}
const coach = computed(
  () =>
    [
      t("試試看：按一個帳號卡片選擇帳號", "试试看：点一个账号卡片选择账号", "Try it: click an account card to select it"),
      t("按 ↻ 取得一次性密碼", "点 ↻ 获取一次性密码", "Click ↻ to get a one-time password"),
      t("OTP 已貼進遊戲。按「開始遊戲」", "OTP 已贴进游戏。点「开始游戏」", "The OTP is in the game. Press PLAY"),
      t("就是這樣，三下就進遊戲。", "就是这样，三下就进游戏。", "That is it. Three clicks into the game."),
    ][step.value],
);
</script>

<template>
  <UiFrame :width="760" :height="530">
    <div class="ml win">
      <div class="ml-glow"></div>
      <UiTitlebar page="main" :region="session.region" />

      <div class="win__tabs">
        <button
          v-for="(s, i) in sessions"
          :key="s.id"
          class="win__tab"
          :class="{ 'win__tab--on': i === active }"
          @click="pickSession(i)"
        >
          <i class="win__dot" :class="s.region === 'TW' ? 'win__dot--tw' : 'win__dot--hk'"></i>{{ s.name }}<small>{{ s.region }}</small>
        </button>
        <span class="win__tab win__tab--add">+</span>
      </div>

      <div class="win__body">
        <div class="win__left">
          <UiPlayColumn ref="play" :can-classic="session.region === 'HK'" :hint="step === 2" @launched="step <= 2 && (step = 3)" />
        </div>
        <div class="win__right">
          <UiAccountPanel
            :key="key"
            :accounts="session.accounts"
            :user="session.name"
            :beans="session.beans"
            :region="session.region"
            :hint="step === 0 ? 'pick' : step === 1 ? 'fetch' : null"
            @pick="step === 0 && (step = 1)"
            @fetched="step <= 1 && (step = 2)"
          />
        </div>
      </div>

      <div class="win__coach" :class="{ 'win__coach--done': step === 3 }">
        <span>{{ coach }}</span>
        <button v-if="step === 3" @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button>
      </div>
    </div>
  </UiFrame>
</template>

<style scoped>
.win {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5);
}
.win__tabs {
  position: relative;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px 4px;
  border-bottom: 1px solid var(--ml-border);
}
.win__tab {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 6px 6px 0 0;
  font-size: 11px;
  color: var(--ml-dim);
}
.win__tab:hover {
  color: var(--ml-text);
}
.win__tab--on {
  background: var(--ml-surface);
  color: var(--ml-accent);
}
.win__tab--add {
  font-size: 12px;
  color: var(--ml-faint);
}
.win__tab small {
  font-size: 9px;
  color: var(--ml-faint);
}
.win__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.win__dot--hk {
  background: var(--ml-green);
}
.win__dot--tw {
  background: #60a5fa;
}
.win__body {
  position: relative;
  display: flex;
  flex-grow: 1;
  overflow: hidden;
}
.win__left {
  width: 40%;
}
.win__right {
  flex-grow: 1;
  border-left: 1px solid var(--ml-border);
}
.win__coach {
  position: absolute;
  left: 50%;
  bottom: 10px;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 7px 14px;
  border-radius: 999px;
  background: rgba(232, 162, 58, 0.14);
  border: 1px solid rgba(232, 162, 58, 0.35);
  color: #f3c77a;
  font-size: 12px;
  white-space: nowrap;
  backdrop-filter: blur(6px);
  pointer-events: none;
}
.win__coach--done {
  background: rgba(74, 222, 128, 0.12);
  border-color: rgba(74, 222, 128, 0.35);
  color: #8ff0b0;
  pointer-events: auto;
}
.win__coach button {
  font-weight: 700;
  text-decoration: underline;
  text-underline-offset: 2px;
}
</style>
