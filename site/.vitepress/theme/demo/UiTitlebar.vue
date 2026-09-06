<script setup lang="ts">
import { useT } from "./i18n";

/**
 * The 34px title bar. On the sign-in page it carries the classic toggle and
 * the region flag (both clickable); everywhere else just the region code.
 */
defineProps<{
  page: "login" | "main";
  region: "HK" | "TW";
  classic?: boolean;
  hint?: "region" | null;
}>();
const emit = defineEmits<{ "toggle-region": []; "toggle-classic": [] }>();
const t = useT();
</script>

<template>
  <div class="tb">
    <span class="tb__brand">MAPLELINK</span>
    <template v-if="page === 'login'">
      <button class="tb__btn" :class="{ 'tb__btn--on': classic }" :title="t('懷舊服', '怀旧服', 'Classic')" @click="emit('toggle-classic')">
        🍁<i v-if="classic" class="tb__under"></i>
      </button>
      <button class="tb__btn" :class="{ 'ml-hint': hint === 'region' }" :title="t('切換地區', '切换地区', 'Toggle region')" @click="emit('toggle-region')">
        {{ region === "TW" ? "🇹🇼" : "🇭🇰" }}<i v-if="!classic" class="tb__under"></i>
      </button>
    </template>
    <span v-else class="tb__btn tb__btn--faint">{{ region }}</span>
    <span class="tb__btn">🛠</span>
    <span class="tb__btn tb__btn--lg">−</span>
    <span class="tb__btn tb__btn--lg">×</span>
  </div>
</template>

<style scoped>
.tb {
  position: relative;
  height: 34px;
  display: flex;
  align-items: center;
}
.tb__brand {
  flex-grow: 1;
  padding-left: 16px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 3px;
  color: var(--ml-dim);
}
.tb__btn {
  position: relative;
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--ml-dim);
  border-radius: 4px;
}
button.tb__btn:hover {
  background: var(--ml-surface-hover);
  color: var(--ml-accent);
}
.tb__btn--on {
  color: var(--ml-accent);
}
.tb__btn--faint {
  color: var(--ml-faint);
}
.tb__btn--lg {
  font-size: 14px;
}
.tb__under {
  position: absolute;
  bottom: 5px;
  left: 50%;
  width: 12px;
  height: 2px;
  transform: translateX(-50%);
  border-radius: 2px;
  background: var(--ml-accent);
  opacity: 0.6;
}
</style>
