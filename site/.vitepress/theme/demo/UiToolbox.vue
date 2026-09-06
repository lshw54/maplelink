<script setup lang="ts">
import { useT } from "./i18n";
import UiTitlebar from "./UiTitlebar.vue";

/**
 * The toolbox window shell (750×490): title bar, the 150px tab rail on the
 * left, a back button at its foot, and a scrolling content area. Tabs are
 * clickable; the parent decides which one is shown.
 */
const props = defineProps<{ tab: string }>();
const emit = defineEmits<{ "update:tab": [key: string] }>();
const t = useT();

const TABS = [
  { key: "tools", icon: "🛠", label: () => t("工具", "工具", "Tools") },
  { key: "announcements", icon: "📢", label: () => t("公告", "公告", "Announcements") },
  { key: "account_manager", icon: "👤", label: () => t("帳號管理", "账号管理", "Account Manager") },
  { key: "settings", icon: "⚙", label: () => t("設定", "设置", "Settings") },
  { key: "advanced", icon: "🔧", label: () => t("進階", "高级", "Advanced") },
  { key: "about", icon: "ℹ", label: () => t("關於", "关于", "About") },
];
</script>

<template>
  <div class="ml tbx">
    <div class="ml-glow"></div>
    <UiTitlebar page="main" region="HK" />
    <div class="tbx__body">
      <nav class="tbx__nav">
        <button
          v-for="x in TABS"
          :key="x.key"
          class="tbx__tab"
          :class="{ 'tbx__tab--on': x.key === props.tab }"
          @click="emit('update:tab', x.key)"
        >
          <span class="tbx__icon">{{ x.icon }}</span>{{ x.label() }}
        </button>
        <div class="tbx__spacer"></div>
        <span class="tbx__back">{{ t("返回", "返回", "Back") }}</span>
      </nav>
      <div class="tbx__content">
        <slot />
      </div>
    </div>
  </div>
</template>

<style scoped>
.tbx {
  --tb-card: #141619;
  --tb-nav: #0a0c10;
  --tb-border: rgba(255, 255, 255, 0.1);
  --tb-input: #0f1114;
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
.tbx__body {
  position: relative;
  flex: 1;
  display: flex;
  min-height: 0;
}
.tbx__nav {
  width: 150px;
  flex: none;
  display: flex;
  flex-direction: column;
  padding: 16px 0;
  border-right: 1px solid var(--tb-border);
  background: var(--tb-nav);
}
.tbx__tab {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 18px;
  border-left: 3px solid transparent;
  text-align: left;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.5px;
  color: var(--ml-dim);
}
.tbx__tab:hover {
  color: var(--ml-text);
}
.tbx__tab--on {
  border-left-color: var(--ml-accent);
  background: rgba(232, 162, 58, 0.08);
  color: var(--ml-accent);
}
.tbx__icon {
  width: 20px;
  text-align: center;
  font-size: 14px;
}
.tbx__spacer {
  flex: 1;
}
.tbx__back {
  margin: 0 12px;
  padding: 8px;
  border: 1px solid var(--tb-border);
  border-radius: 8px;
  text-align: center;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.tbx__content {
  flex: 1;
  min-width: 0;
  padding: 16px;
  overflow: hidden;
}
</style>
