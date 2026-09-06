<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { withBase } from "vitepress";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiTitlebar from "./UiTitlebar.vue";

/**
 * The in-app announcement shown on first launch: a card over the dimmed
 * window whose only button counts down before it can be pressed. The real
 * app holds it for ten seconds; the demo uses five and says so.
 */
const HOLD = 5;
const t = useT();
const left = ref(HOLD);
const dismissed = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;
function start() {
  stop();
  left.value = HOLD;
  dismissed.value = false;
  timer = setInterval(() => {
    if (left.value > 0) left.value--;
    if (left.value === 0) stop();
  }, 1000);
}
function stop() {
  if (timer) clearInterval(timer);
  timer = null;
}
start();
onBeforeUnmount(stop);

const coach = computed(() =>
  dismissed.value
    ? t("關閉後不會再自動彈出。日後可在工具箱的「公告」分頁重看。", "关闭后不会再自动弹出。日后可在工具箱的「公告」标签重看。", "It will not pop up again. You can reread it later under Toolbox → Announcements.")
    : left.value > 0
      ? t("按鈕會鎖住幾秒，讓你先讀完（真實程式是 10 秒）。", "按钮会锁住几秒，让你先读完（真实程序是 10 秒）。", "The button stays locked for a few seconds so you read first (ten in the real app).")
      : t("可以按了。", "可以点了。", "You can press it now."),
);
</script>

<template>
  <div class="demo">
    <UiFrame :width="640" :height="440">
      <div class="ml an">
        <div class="ml-glow"></div>
        <UiTitlebar page="login" region="HK" />
        <div class="an__bg">
          <img :src="withBase('/logo.png')" alt="" />
          <span>MAPLELINK</span>
        </div>
        <div v-if="!dismissed" class="an__overlay">
          <div class="an__card">
            <div class="an__head"><span>📢</span><b>{{ t("下載來源提醒", "下载来源提醒", "Where to download this") }}</b></div>
            <div class="an__body">
              <p>{{ t("我們接獲回報，有非官方、疑似被重新打包的 Beanfun 啟動器在外流傳。", "我们接获回报，有非官方、疑似被重新打包的 Beanfun 启动器在外流传。", "We have had reports of unofficial, seemingly repackaged copies of the Beanfun launcher circulating.") }}</p>
              <p>{{ t("開發團隊每次發佈，只會提供 GitHub 上該版本 exe 的下載連結。我們給的永遠是連結，不是檔案。", "开发团队每次发布，只会提供 GitHub 上该版本 exe 的下载连结。我们给的永远是连结，不是档案。", "For every release, the development team gives out one thing: a link to that version's exe on GitHub. What we hand over is always a link, never a file.") }}</p>
              <p>{{ t("下載與使用前，請先確認來源安全可靠。不是從認可位置取得的，請立即刪除並重新下載。", "下载与使用前，请先确认来源安全可靠。不是从认可位置取得的，请立即删除并重新下载。", "Before you download it and before you run it, make sure the source is one you can trust. If it did not come from a recognised source, delete it and download it again.") }}</p>
            </div>
            <div class="an__foot">
              <button class="an__btn" :class="{ 'an__btn--off': left > 0, 'ml-hint': left === 0 }" :disabled="left > 0" @click="dismissed = true">
                {{ left > 0 ? t(`請先閱讀公告（${left} 秒）`, `请先阅读公告（${left} 秒）`, `Please read the notice (${left}s)`) : t("我已閱讀，下次不再顯示", "我已阅读，下次不再显示", "I've read it. Don't show again") }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </UiFrame>
    <UiCoach :done="dismissed">
      {{ coach }}
      <template v-if="dismissed" #action><button @click="start">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.an {
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
.an__bg {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.an__bg img {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  box-shadow: 0 4px 20px var(--ml-glow);
}
.an__bg span {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 5px;
  color: var(--ml-dim);
}
.an__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
}
.an__card {
  width: 540px;
  max-width: 100%;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
  overflow: hidden;
}
.an__head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  font-size: 16px;
  font-weight: 700;
}
.an__body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 18px 24px;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--ml-dim);
}
.an__body p {
  margin: 0;
}
.an__foot {
  padding: 14px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}
.an__btn {
  width: 100%;
  padding: 10px;
  border-radius: 8px;
  background: var(--ml-accent);
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  transition: opacity 0.2s;
}
.an__btn--off {
  opacity: 0.4;
  cursor: default;
}
</style>
