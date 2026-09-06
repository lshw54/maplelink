<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiPlayColumn from "./UiPlayColumn.vue";

/** The PLAY column on its own, for the guide's launch step. */
const t = useT();
const launched = ref(false);
const col = ref<InstanceType<typeof UiPlayColumn> | null>(null);
const coach = computed(() =>
  launched.value
    ? t("遊戲已啟動。上方的正式服／懷舊服切換只在 HK 帳號出現。", "游戏已启动。上方的正式服／怀旧服切换只在 HK 账号出现。", "The game is running. The Live / Classic switch above appears for HK accounts only.")
    : t("按圓形的「開始遊戲」。", "点圆形的「开始游戏」。", "Press the round PLAY button."),
);
function reset() {
  launched.value = false;
  col.value?.reset();
}
</script>

<template>
  <div class="demo">
    <UiFrame :width="304" :height="440">
      <div class="ml panel">
        <div class="ml-glow"></div>
        <UiPlayColumn ref="col" :hint="!launched" @launched="launched = true" />
      </div>
    </UiFrame>
    <UiCoach :done="launched">
      {{ coach }}
      <template v-if="launched" #action><button @click="reset">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.panel {
  position: relative;
  width: 100%;
  height: 100%;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
}
</style>
