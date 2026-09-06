<script setup lang="ts">
import { ref } from "vue";
import { withBase } from "vitepress";
import { useT } from "./i18n";
import { useLatestVersion } from "./release";

/**
 * The left-hand side of the main window: the live / classic switch, the game
 * badge, the round PLAY button, logout, and the status line at the bottom.
 * PLAY runs a short timer and then reports the game as running.
 */
withDefaults(defineProps<{ canClassic?: boolean; hint?: boolean }>(), { canClassic: true, hint: false });
const emit = defineEmits<{ launched: [] }>();
const t = useT();
const version = useLatestVersion();

const classic = ref(false);
const launching = ref(false);
const running = ref(false);

function play() {
  if (launching.value) return;
  launching.value = true;
  setTimeout(() => {
    launching.value = false;
    running.value = true;
    emit("launched");
  }, 800);
}
defineExpose({
  reset() {
    running.value = false;
  },
});
</script>

<template>
  <div class="play">
    <img class="play__ghost" :src="withBase('/MapleStory.png')" alt="" />
    <div class="play__col">
      <div v-if="canClassic" class="play__pill">
        <button :class="{ 'ml-grad-deep play__pill--on': !classic }" @click="classic = false">{{ t("正式服", "正式服", "Live") }}</button>
        <button :class="{ 'ml-grad-deep play__pill--on': classic }" @click="classic = true">{{ t("懷舊服", "怀旧服", "Classic") }}</button>
      </div>
      <div class="play__game"><img :src="withBase('/MapleStory.png')" alt="" /></div>
      <div class="play__name">{{ classic ? t("新楓之谷：經典版", "新枫之谷：经典版", "MapleStory Classic") : "MapleStory" }}</div>
      <div class="play__sub">Gamania · MMORPG</div>
      <button class="play__btn ml-grad-deep" :class="{ 'play__btn--busy': launching, 'ml-hint': hint }" @click="play">
        {{ launching ? "..." : t("開始遊戲", "开始游戏", "PLAY") }}
      </button>
      <span v-if="running" class="play__running">{{ t("執行中", "运行中", "Running") }} (PID: 24816)</span>
      <div class="play__logout">{{ t("登出", "登出", "LOGOUT") }}</div>
    </div>
    <div class="play__status">
      <span class="play__online"><i></i>ONLINE <small>42ms</small></span>
      <span class="play__ver">MapleLink v{{ version ?? "…" }}</span>
    </div>
  </div>
</template>

<style scoped>
.play {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 24px 64px;
}
.play__ghost {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 160px;
  height: 160px;
  transform: translate(-50%, -50%);
  opacity: 0.04;
  filter: blur(2px);
  pointer-events: none;
}
.play__col {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
}
.play__pill {
  display: flex;
  padding: 4px;
  border-radius: 999px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.15);
}
.play__pill button {
  padding: 6px 16px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 1px;
  color: var(--ml-dim);
}
.play__pill button:hover {
  color: var(--ml-text);
}
.play__pill button.play__pill--on {
  color: #fff;
  box-shadow: 0 2px 10px var(--ml-glow);
}
.play__game {
  width: 56px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 14px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface-hover);
}
.play__game img {
  width: 40px;
  height: 40px;
}
.play__name {
  font-size: 16px;
  font-weight: 800;
  letter-spacing: 1px;
}
.play__sub {
  margin-top: -8px;
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.play__btn {
  margin-top: 4px;
  width: 72px;
  height: 72px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 3px;
  box-shadow: 0 4px 24px var(--ml-glow), 0 0 0 3px rgba(232, 162, 58, 0.1);
  transition: transform 0.15s;
}
.play__btn:hover {
  transform: scale(1.08);
}
.play__btn:active {
  transform: scale(0.93);
}
.play__btn--busy {
  opacity: 0.4;
  transform: none !important;
}
.play__running {
  font-size: 12px;
  color: var(--ml-accent);
}
.play__logout {
  margin-top: 8px;
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--ml-surface);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 2px;
  color: var(--ml-dim);
}
.play__status {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding-bottom: 8px;
  font-family: var(--ml-mono);
}
.play__online {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  color: var(--ml-dim);
}
.play__online i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ml-green);
  box-shadow: 0 0 6px rgba(74, 222, 128, 0.4);
}
.play__online small {
  font-weight: 400;
  letter-spacing: 0;
  color: var(--ml-faint);
}
.play__ver {
  font-size: 10px;
  letter-spacing: 1px;
  color: var(--ml-faint);
}
</style>
