<script setup lang="ts">
import { ref } from "vue";
import { useT } from "./i18n";

/**
 * The right-hand side of the launcher's main window: session header, account
 * cards and the one-time-password panel, drawn to the app's own sizes. Fully
 * playable: pick a card, fetch an OTP (random digits, no network), toggle
 * auto-type. Emits what happened so a parent can narrate.
 */
export interface DemoAccount {
  initial: string;
  name: string;
}
const props = withDefaults(
  defineProps<{
    accounts: DemoAccount[];
    user: string;
    beans: number;
    /** HK shows the beans → game points conversion; TW does not. */
    region?: "HK" | "TW";
    /** Which control to nudge: a card, the fetch button, or nothing. */
    hint?: "pick" | "fetch" | null;
  }>(),
  { region: "HK", hint: null },
);
const emit = defineEmits<{ pick: [index: number]; fetched: [otp: string] }>();
const t = useT();

const selected = ref(0);
const autoInput = ref(true);
const otp = ref<string | null>(null);
const busy = ref(false);
const copied = ref(false);
const pasted = ref(false);

function pick(i: number) {
  selected.value = i;
  otp.value = null;
  emit("pick", i);
}
function copy() {
  if (!otp.value) return;
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
function fetchOtp() {
  if (busy.value) return;
  busy.value = true;
  otp.value = null;
  setTimeout(() => {
    otp.value = String(Math.floor(100000 + Math.random() * 900000));
    busy.value = false;
    // The real app copies the code as it arrives (green flash) and, with
    // auto-input on, also types it into the game window.
    copy();
    if (autoInput.value) {
      pasted.value = true;
      setTimeout(() => (pasted.value = false), 1600);
    }
    emit("fetched", otp.value);
  }, 650);
}
</script>

<template>
  <div class="acc">
    <div class="acc__bar">
      <span class="acc__avatar ml-grad">{{ user.charAt(0).toUpperCase() }}</span>
      <span>{{ user }}</span>
      <span class="acc__spacer"></span>
      <span class="acc__beans">
        <b class="acc__beans-n">{{ t("樂豆", "乐豆", "Beans") }}: <b>{{ beans }}</b></b>
        <template v-if="region === 'HK'">
          <i class="acc__sep">·</i>
          <span class="acc__pts">{{ t("遊戲點數", "游戏点数", "Game points") }}: {{ Math.floor(beans / 2.5) }}</span>
        </template>
      </span>
      <span class="acc__more">⋯</span>
    </div>

    <div class="acc__list">
      <div class="acc__head">
        <span class="acc__label">
          {{ t("帳號列表", "账号列表", "ACCOUNTS") }}
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><line x1="2" y1="4" x2="14" y2="4"></line><line x1="2" y1="8" x2="14" y2="8"></line><line x1="2" y1="12" x2="14" y2="12"></line></svg>
        </span>
        <span class="acc__refresh">{{ t("重新整理", "刷新", "Refresh") }}</span>
      </div>
      <div class="acc__grid">
        <button
          v-for="(a, i) in accounts"
          :key="a.name"
          class="acc__card"
          :class="{ 'acc__card--on': i === selected, 'ml-hint': hint === 'pick' && i === (selected === 1 ? 0 : 1) }"
          @click="pick(i)"
        >
          <span class="acc__init">{{ a.initial }}</span><span>{{ a.name }}</span>
        </button>
      </div>
    </div>

    <div class="acc__otp">
      <div class="acc__otp-head">
        <span>🔐 {{ t("一次性密碼", "一次性密码", "ONE-TIME PASSWORD") }}</span>
        <button class="acc__auto" @click="autoInput = !autoInput">
          {{ t("自動輸入", "自动输入", "Auto-type") }}
          <i class="acc__toggle" :class="{ 'acc__toggle--off': !autoInput }"><i></i></i>
        </button>
      </div>
      <div class="acc__otp-row">
        <button class="acc__code" :class="{ 'acc__code--empty': !otp, 'acc__code--copied': copied }" @click="copy">
          {{ otp ?? "••••••••••" }}
          <span class="acc__paste" :class="{ 'acc__paste--show': pasted }">{{ t("已貼入遊戲", "已贴入游戏", "Pasted into game") }}</span>
          <span class="acc__copyicon">{{ copied ? "✓" : "⧉" }}</span>
        </button>
        <div class="acc__fetch ml-grad" :class="{ 'ml-hint': hint === 'fetch' }">
          <button :class="{ 'acc__spin': busy }" @click="fetchOtp">↻</button>
          <span class="acc__caret">▾</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.acc {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.acc__bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ml-border);
  font-size: 12px;
  color: var(--ml-dim);
}
.acc__avatar {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
}
.acc__spacer {
  flex-grow: 1;
}
.acc__beans {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 6px;
  border: 1px solid rgba(232, 162, 58, 0.15);
  background: rgba(232, 162, 58, 0.08);
  white-space: nowrap;
}
.acc__beans-n {
  color: var(--ml-accent);
  font-weight: 600;
}
.acc__sep {
  color: var(--ml-faint);
  font-style: normal;
}
.acc__pts {
  color: var(--ml-dim);
}
.acc__more {
  width: 22px;
  text-align: center;
  font-size: 14px;
}
.acc__list {
  flex-grow: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
}
.acc__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ml-dim);
}
.acc__label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.acc__label svg {
  color: var(--ml-faint);
}
.acc__refresh {
  font-size: 12px;
  font-weight: 400;
  letter-spacing: 0;
  color: var(--ml-accent);
}
.acc__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}
.acc__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px;
  border-radius: 12px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 12px;
  font-weight: 500;
  transition: border-color 0.15s, background 0.15s, box-shadow 0.15s;
}
.acc__card:hover {
  background: var(--ml-surface-hover);
}
.acc__init {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  border: 2px solid var(--ml-border);
  background: var(--ml-surface-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 700;
  color: var(--ml-dim);
}
.acc__card:hover .acc__init,
.acc__card--on .acc__init {
  border-color: var(--ml-accent);
  color: var(--ml-accent);
}
.acc__card--on {
  border-color: var(--ml-accent);
  background: rgba(232, 162, 58, 0.05);
  box-shadow: 0 0 20px rgba(232, 162, 58, 0.15);
}
.acc__otp {
  margin: 0 12px 12px;
  padding: 14px;
  border-radius: 12px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.1), 0 0 0 1px var(--ml-border);
}
.acc__otp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2.5px;
  color: var(--ml-dim);
}
.acc__auto {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 400;
  letter-spacing: 0.5px;
  color: var(--ml-faint);
}
.acc__toggle {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 9px;
  background: rgba(232, 162, 58, 0.3);
  display: inline-block;
}
.acc__toggle i {
  position: absolute;
  top: 2px;
  left: 16px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--ml-accent);
  transition: left 0.15s;
}
.acc__toggle--off {
  background: var(--ml-surface-hover);
}
.acc__toggle--off i {
  left: 2px;
  background: var(--ml-dim);
}
.acc__otp-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.acc__code {
  position: relative;
  flex-grow: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 10px 16px;
  border-radius: 10px;
  border: 1px solid rgba(232, 162, 58, 0.08);
  background: rgba(232, 162, 58, 0.04);
  color: var(--ml-accent);
  font-family: var(--ml-mono);
  font-size: 22px;
  font-weight: 700;
  letter-spacing: 4px;
  box-shadow: inset 0 0 20px rgba(232, 162, 58, 0.06), inset 0 2px 8px rgba(0, 0, 0, 0.3);
  transition: color 0.2s, border-color 0.2s;
}
.acc__code--empty {
  color: var(--ml-faint);
}
.acc__code--copied {
  color: var(--ml-green);
  border-color: rgba(74, 222, 128, 0.4);
}
.acc__copyicon {
  position: absolute;
  right: 10px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
  letter-spacing: 0;
  color: var(--ml-faint);
}
.acc__code--copied .acc__copyicon {
  color: var(--ml-green);
}
.acc__paste {
  position: absolute;
  left: 50%;
  top: -30px;
  transform: translate(-50%, 6px);
  padding: 3px 10px;
  border-radius: 6px;
  background: var(--ml-green);
  color: #062b14;
  font-family: inherit;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0;
  opacity: 0;
  transition: opacity 0.2s, transform 0.2s;
  pointer-events: none;
  white-space: nowrap;
}
.acc__paste--show {
  opacity: 1;
  transform: translate(-50%, 0);
}
.acc__fetch {
  display: flex;
  border-radius: 10px;
  overflow: hidden;
  box-shadow: 0 2px 10px var(--ml-glow);
}
.acc__fetch button,
.acc__caret {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
}
.acc__fetch button:hover {
  background: rgba(255, 255, 255, 0.1);
}
.acc__spin {
  animation: acc-spin 0.65s linear infinite;
}
@keyframes acc-spin {
  to {
    transform: rotate(360deg);
  }
}
.acc__caret {
  width: 20px;
  border-left: 1px solid rgba(255, 255, 255, 0.25);
  font-size: 9px;
}
</style>
