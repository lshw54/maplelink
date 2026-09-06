<script setup lang="ts">
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";

/**
 * What a PC without WebView2 shows: the window opens, but there is nothing to
 * draw the interface with, so it stays plain white (or closes at once).
 */
const t = useT();
</script>

<template>
  <div class="demo">
    <UiFrame :width="350" :height="300">
      <div class="bl">
        <div class="bl__bar"><span>MapleLink</span><span class="bl__ctl">−  □  ✕</span></div>
        <div class="bl__page"></div>
      </div>
    </UiFrame>
    <UiCoach>
      {{ t("整個視窗一片白、沒有任何文字，或一開啟就自己關掉，就是缺少 WebView2。", "整个窗口一片白、没有任何文字，或一打开就自己关掉，就是缺少 WebView2。", "An all-white window with no text, or one that closes by itself, means WebView2 is missing.") }}
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.bl {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid #555;
  border-radius: 8px;
  overflow: hidden;
  background: #fff;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
  font-family: "Segoe UI", system-ui, sans-serif;
}
.bl__bar {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  background: #f3f3f3;
  color: #333;
  font-size: 12px;
}
.bl__ctl {
  letter-spacing: 6px;
  color: #666;
}
.bl__page {
  flex: 1;
  background: #fff;
}
</style>
