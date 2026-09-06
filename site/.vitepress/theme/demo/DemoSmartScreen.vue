<script setup lang="ts">
import { computed, ref } from "vue";
import { useData } from "vitepress";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";

/**
 * The Windows SmartScreen dialog, playable. As on a real PC, "Run anyway" is
 * hidden until "More info" is clicked. The dialog's own language follows the
 * visitor's Windows, not this site, so a switch lets them see both.
 */
const t = useT();
const { lang } = useData();
const ui = ref<"zh" | "en">(lang.value.startsWith("en") ? "en" : "zh");
const expanded = ref(false);
const done = ref<"run" | "stop" | null>(null);

const s = computed(() => {
  const cn = lang.value === "zh-CN";
  if (ui.value === "en")
    return {
      title: "Windows protected your PC",
      body: "Microsoft Defender SmartScreen prevented an unrecognized app from starting. Running this app might put your PC at risk.",
      more: "More info",
      app: "App:",
      publisher: "Publisher:",
      unknown: "Unknown publisher",
      run: "Run anyway",
      stop: "Don't run",
    };
  return cn
    ? {
        title: "Windows 已保护你的电脑",
        body: "Microsoft Defender SmartScreen 已阻止启动一个未识别的应用。运行此应用可能会导致你的电脑存在风险。",
        more: "更多信息",
        app: "应用:",
        publisher: "发布者:",
        unknown: "未知发布者",
        run: "仍要运行",
        stop: "不运行",
      }
    : {
        title: "Windows 已保護您的電腦",
        body: "Microsoft Defender SmartScreen 已防止某個無法辨識的應用程式啟動。執行此應用程式可能會讓您的電腦暴露在風險中。",
        more: "其他資訊",
        app: "應用程式:",
        publisher: "發行者:",
        unknown: "未知的發行者",
        run: "仍要執行",
        stop: "不要執行",
      };
});

const coach = computed(() => {
  if (done.value === "run") return t("程式就會開啟。這個提示只在第一次出現。", "程序就会打开。这个提示只在第一次出现。", "The app opens. This prompt appears only the first time.");
  if (done.value === "stop") return t("什麼都不會發生。核對過 SHA256 就可以放心按另一個。", "什么都不会发生。核对过 SHA256 就可以放心点另一个。", "Nothing happens. Once the SHA256 matches, the other button is safe.");
  if (!expanded.value) return t("一開始沒有「仍要執行」。先按「其他資訊」。", "一开始没有「仍要运行」。先点「更多信息」。", "There is no Run anyway at first. Click More info.");
  return t("現在按「仍要執行」。發行者顯示未知是正常的，程式沒有買商業憑證。", "现在点「仍要运行」。发布者显示未知是正常的，程序没有买商业证书。", "Now click Run anyway. Unknown publisher is expected; the app has no commercial certificate.");
});
function reset() {
  expanded.value = false;
  done.value = null;
}
</script>

<template>
  <div class="demo">
    <div class="ss-lang">
      <span>{{ t("對話框語言跟隨你的 Windows：", "对话框语言跟随你的 Windows：", "The dialog follows your Windows language:") }}</span>
      <button :class="{ on: ui === 'zh' }" @click="ui = 'zh'; reset()">{{ lang === "zh-CN" ? "简体中文" : "繁體中文" }}</button>
      <button :class="{ on: ui === 'en' }" @click="ui = 'en'; reset()">English</button>
    </div>
    <UiFrame :width="520" :height="400">
      <div class="ss" :class="{ 'ss--done': done }">
        <button class="ss__close" aria-label="Close" @click="done = 'stop'">✕</button>
        <h2 class="ss__title">{{ s.title }}</h2>
        <p class="ss__body">{{ s.body }}</p>
        <button v-if="!expanded" class="ss__more" :class="{ 'ss-hint': !done }" @click="expanded = true">{{ s.more }}</button>
        <dl v-else class="ss__info">
          <dt>{{ s.app }}</dt>
          <dd>MapleLink.exe</dd>
          <dt>{{ s.publisher }}</dt>
          <dd>{{ s.unknown }}</dd>
        </dl>
        <div class="ss__actions">
          <button v-if="expanded" class="ss__btn" :class="{ 'ss-hint': !done }" @click="done = 'run'">{{ s.run }}</button>
          <button class="ss__btn" @click="done = 'stop'">{{ s.stop }}</button>
        </div>
      </div>
    </UiFrame>
    <UiCoach :done="done === 'run'">
      {{ coach }}
      <template v-if="done" #action><button @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.ss-lang {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 10px;
  font-size: 13px;
  color: var(--vp-c-text-2);
}
.ss-lang button {
  font: inherit;
  padding: 2px 10px;
  border-radius: 999px;
  border: 1px solid var(--vp-c-divider);
  background: none;
  color: var(--vp-c-text-2);
  cursor: pointer;
}
.ss-lang button.on {
  border-color: var(--vp-c-brand-1);
  color: var(--vp-c-brand-1);
}
/* Windows 10/11 SmartScreen: one flat blue sheet, Segoe UI, light text */
.ss {
  position: relative;
  width: 100%;
  height: 100%;
  padding: 40px 36px 32px;
  display: flex;
  flex-direction: column;
  background: #0078d7;
  color: #fff;
  font-family: "Segoe UI", "Microsoft JhengHei UI", "Microsoft YaHei UI", system-ui, sans-serif;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.45);
  user-select: none;
  transition: opacity 0.25s;
}
.ss--done {
  opacity: 0.55;
}
.ss :where(button) {
  font: inherit;
  color: inherit;
  background: none;
  border: 0;
  padding: 0;
  cursor: pointer;
}
.ss__close {
  position: absolute;
  top: 0;
  right: 0;
  width: 46px;
  height: 32px;
  font-size: 14px;
  background: #fff !important;
  color: #333 !important;
}
.ss__close:hover {
  background: #e81123 !important;
  color: #fff !important;
}
.ss__title {
  margin: 0 0 20px;
  font-size: 28px;
  font-weight: 300;
  letter-spacing: -0.01em;
  line-height: 1.2;
  border: 0;
  padding: 0;
}
.ss__body {
  margin: 0 0 18px;
  font-size: 15px;
  line-height: 1.45;
}
.ss__more {
  align-self: flex-start;
  font-size: 15px;
  text-decoration: underline;
  text-underline-offset: 3px;
  border-radius: 3px;
  padding: 1px 3px !important;
}
.ss__info {
  margin: 0;
  display: grid;
  grid-template-columns: max-content 1fr;
  column-gap: 12px;
  row-gap: 4px;
  font-size: 15px;
}
.ss__info dd {
  margin: 0;
}
.ss__actions {
  margin-top: auto;
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
.ss__btn {
  min-width: 108px;
  height: 36px;
  padding: 0 18px !important;
  background: #fff !important;
  color: #000 !important;
  font-size: 15px;
  font-weight: 600;
  border-radius: 2px;
}
.ss__btn:hover {
  background: #e6e6e6 !important;
}
.ss-hint {
  animation: ss-nudge 1.6s ease-in-out infinite;
}
@keyframes ss-nudge {
  0%,
  100% {
    box-shadow: 0 0 0 0 rgba(255, 255, 255, 0);
  }
  50% {
    box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.55);
  }
}
</style>
