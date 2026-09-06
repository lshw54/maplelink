<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiToolbox from "./UiToolbox.vue";

/**
 * Toolbox → Settings (game path) and Toolbox → Advanced (launch toggles).
 * `focus` picks which control the coach points at and which tab opens first.
 */
const props = withDefaults(defineProps<{ focus?: "path" | "patcher" }>(), { focus: "path" });
const t = useT();
const tab = ref(props.focus === "patcher" ? "advanced" : "settings");
const gamePath = ref<string | null>(null);
const killPatcher = ref(false);
const skipPlay = ref(true);
const autoLaunch = ref(false);
const done = ref(false);

function browse() {
  gamePath.value = "C:\\Games\\MapleStory";
  if (props.focus === "path") done.value = true;
}
function togglePatcher() {
  killPatcher.value = !killPatcher.value;
  if (props.focus === "patcher") done.value = true;
}
function reset() {
  done.value = false;
  gamePath.value = null;
  killPatcher.value = false;
  tab.value = props.focus === "patcher" ? "advanced" : "settings";
}
const coach = computed(() => {
  if (props.focus === "path") {
    if (done.value) return t("路徑會存入 config.ini。之後啟動遊戲就用這裏的位置。", "路径会存入 config.ini。之后启动游戏就用这里的位置。", "The path is saved to config.ini and used for every launch.");
    if (tab.value !== "settings") return t("按左邊的「設定」。", "点左边的「设置」。", "Click Settings on the left.");
    return t("遊戲路徑顯示「—」代表未偵測到。按「瀏覧」，選 MapleStory.exe 所在的資料夾。", "游戏路径显示「—」代表未检测到。点「浏览」，选 MapleStory.exe 所在的文件夹。", "A dash means no path was detected. Click Browse and pick the folder that holds MapleStory.exe.");
  }
  if (done.value) return killPatcher.value
    ? t("已開啟。啟動遊戲時會自動關閉 Patcher.exe。平時建議關閉。", "已开启。启动游戏时会自动关闭 Patcher.exe。平时建议关闭。", "On. Patcher.exe is closed as the game starts. Leave it off normally.")
    : t("已關閉，恢復正常更新。", "已关闭，恢复正常更新。", "Off. Updates run as normal.");
  if (tab.value !== "advanced") return t("按左邊的「進階」。", "点左边的「高级」。", "Click Advanced on the left.");
  return t("按「阻止遊戲自動更新」右邊的開關。", "点「阻止游戏自动更新」右边的开关。", "Flip the switch beside Block Game Auto-Update.");
});
</script>

<template>
  <div class="demo">
    <UiFrame :width="750" :height="490">
      <UiToolbox v-model:tab="tab">
        <div v-if="tab === 'settings'" class="st">
          <section class="st__sec">
            <h3>{{ t("遊戲", "游戏", "Game") }}</h3>
            <div class="st__card">
              <div class="st__row">
                <div class="st__label">{{ t("遊戲路徑", "游戏路径", "Game Path") }}</div>
                <span class="st__val st__mono">{{ gamePath ?? "—" }}</span>
                <button class="st__btn" :class="{ 'ml-hint': focus === 'path' && !done }" @click="browse">{{ t("瀏覽", "浏览", "Browse") }}</button>
              </div>
              <div class="st__row">
                <div class="st__label">{{ t("經典版 NGM 路徑", "经典版 NGM 路径", "Classic NGM path") }}</div>
                <span class="st__val st__mono">{{ t("自動偵測", "自动检测", "Auto-detect") }}</span>
                <button class="st__btn">{{ t("瀏覽", "浏览", "Browse") }}</button>
              </div>
            </div>
          </section>
          <section class="st__sec">
            <h3>{{ t("外觀", "外观", "Appearance") }}</h3>
            <div class="st__card">
              <div class="st__row">
                <div class="st__label">{{ t("主題", "主题", "Theme") }}</div>
                <span class="st__seg"><b>{{ t("系統", "系统", "System") }}</b><span>{{ t("深色", "深色", "Dark") }}</span><span>{{ t("淺色", "浅色", "Light") }}</span></span>
              </div>
              <div class="st__row">
                <div class="st__label">{{ t("語言", "语言", "Language") }}</div>
                <span class="st__dd">{{ t("繁體中文", "简体中文", "English") }} ▾</span>
              </div>
            </div>
          </section>
        </div>

        <div v-else-if="tab === 'advanced'" class="st">
          <section class="st__sec">
            <h3>{{ t("啟動", "启动", "Launch") }}</h3>
            <div class="st__card">
              <div class="st__row">
                <div class="st__label">{{ t("跳過 Play 視窗", "跳过 Play 窗口", "Skip Play Window") }}</div>
                <button class="st__toggle" :class="{ 'st__toggle--on': skipPlay }" @click="skipPlay = !skipPlay"><i></i></button>
              </div>
              <div class="st__row">
                <div class="st__label">{{ t("啟動後自動開遊戲", "启动后自动开游戏", "Auto-launch game") }}</div>
                <button class="st__toggle" :class="{ 'st__toggle--on': autoLaunch }" @click="autoLaunch = !autoLaunch"><i></i></button>
              </div>
              <div class="st__row">
                <div class="st__label">
                  {{ t("阻止遊戲自動更新", "阻止游戏自动更新", "Block Game Auto-Update") }}
                  <small>{{ t("啟動遊戲時自動關閉 Patcher.exe，防止遊戲強制更新。", "启动游戏时自动关闭 Patcher.exe，防止游戏强制更新。", "Kill Patcher.exe when launching to prevent forced updates.") }}</small>
                </div>
                <button class="st__toggle" :class="{ 'st__toggle--on': killPatcher, 'ml-hint': focus === 'patcher' && !done }" @click="togglePatcher"><i></i></button>
              </div>
            </div>
          </section>
        </div>

        <div v-else class="st st__empty">{{ t("此示範只包含「設定」與「進階」。", "此示范只包含「设置」与「高级」。", "This demo covers Settings and Advanced only.") }}</div>
      </UiToolbox>
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
.st {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.st__empty {
  padding: 24px 4px;
  font-size: 12px;
  color: var(--ml-dim);
}
.st__sec {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.st__sec h3 {
  margin: 0;
  padding: 0 4px;
  border: 0;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-faint);
}
.st__card {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: #141619;
  overflow: hidden;
}
.st__row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 10px 14px;
}
.st__row + .st__row {
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}
.st__label {
  flex: 1;
  min-width: 76px;
  font-size: 11.5px;
  font-weight: 500;
}
.st__label small {
  display: block;
  margin-top: 2px;
  font-size: 10.5px;
  line-height: 1.35;
  font-weight: 400;
  color: var(--ml-faint);
}
.st__val {
  max-width: 300px;
  text-align: right;
  font-size: 11px;
  line-height: 1.35;
  color: var(--ml-dim);
  word-break: break-all;
}
.st__mono {
  font-family: var(--ml-mono);
}
.st__btn {
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  font-size: 11px;
  font-weight: 500;
  color: var(--ml-dim);
}
.st__btn:hover {
  border-color: var(--ml-accent);
  color: var(--ml-accent);
}
.st__seg {
  display: flex;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  overflow: hidden;
  font-size: 11px;
  font-weight: 600;
}
.st__seg > * {
  padding: 4px 10px;
  color: var(--ml-dim);
}
.st__seg b {
  background: var(--ml-accent);
  color: #fff;
}
.st__dd {
  padding: 4px 10px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  font-size: 11px;
}
.st__toggle {
  position: relative;
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: var(--ml-border);
  transition: background 0.15s;
}
.st__toggle i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.15s;
}
.st__toggle--on {
  background: var(--ml-accent);
}
.st__toggle--on i {
  transform: translateX(16px);
}
</style>
