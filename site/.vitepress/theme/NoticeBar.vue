<script setup lang="ts">
import { computed } from "vue";
import { useData } from "vitepress";
import { NOTICES } from "../notices";

/**
 * A one-line ticker above the nav. Headlines scroll past slowly; hovering
 * pauses them, and each one links to its section on the announcements page.
 * With reduced motion the newest headline is shown still.
 */
const { lang, localeIndex } = useData();

const base = computed(() => (localeIndex.value === "root" ? "" : `/${localeIndex.value}`));
const key = computed(() =>
  lang.value === "zh-CN" ? "zh-CN" : lang.value.startsWith("en") ? "en-US" : "zh-TW",
);
const items = computed(() =>
  NOTICES.map((n) => ({
    id: n.id,
    date: n.date,
    title: n.title[key.value],
    href: `${base.value}/announcements#${n.id}`,
  })),
);
const label = computed(() =>
  key.value === "zh-CN" ? "公告" : key.value === "en-US" ? "Notice" : "公告",
);
const more = computed(() =>
  key.value === "zh-CN" ? "全部公告" : key.value === "en-US" ? "All announcements" : "全部公告",
);
/* Roughly 12 s per headline keeps a sentence readable at a glance. */
const duration = computed(() => `${Math.max(20, items.value.length * 12)}s`);
</script>

<template>
  <div v-if="items.length" class="notice">
    <span class="notice__label">{{ label }}</span>
    <div class="notice__track" :style="{ '--duration': duration }">
      <div class="notice__reel">
        <template v-for="pass in 2" :key="pass">
          <a v-for="n in items" :key="`${pass}-${n.id}`" class="notice__item" :href="n.href" :aria-hidden="pass === 2 || undefined" :tabindex="pass === 2 ? -1 : undefined">
            <time class="notice__date">{{ n.date }}</time>
            <span>{{ n.title }}</span>
          </a>
        </template>
      </div>
    </div>
    <a class="notice__more" :href="`${base}/announcements`">{{ more }}</a>
  </div>
</template>

<style scoped>
/* Pinned to the top of the viewport. VitePress reads --vp-layout-top-height
   (set in custom.css) to push the nav, sidebar and content down by the same
   amount, so nothing hides under the bar at any width. */
.notice {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: var(--vp-z-index-layout-top);
  display: flex;
  align-items: center;
  gap: 14px;
  height: var(--vp-layout-top-height);
  padding: 0 24px;
  background: color-mix(in srgb, var(--vp-c-brand-1) 12%, var(--vp-c-bg));
  color: var(--vp-c-text-1);
  font-size: 13px;
  border-bottom: 1px solid var(--vp-c-divider);
}
.notice__label {
  flex: none;
  font-weight: 700;
  font-size: 11px;
  letter-spacing: 0.08em;
  color: var(--vp-c-brand-1);
}
.notice__track {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  mask-image: linear-gradient(90deg, transparent, #000 24px, #000 calc(100% - 24px), transparent);
}
.notice__reel {
  display: flex;
  width: max-content;
  animation: notice-scroll var(--duration) linear infinite;
}
.notice__track:hover .notice__reel,
.notice__track:focus-within .notice__reel {
  animation-play-state: paused;
}
.notice__item {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding-right: 64px;
  white-space: nowrap;
  color: inherit;
  text-decoration: none;
}
.notice__item:hover span {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.notice__date {
  font-family: var(--vp-font-family-mono);
  font-size: 11px;
  color: var(--vp-c-text-3);
}
.notice__more {
  flex: none;
  font-size: 12px;
  color: var(--vp-c-brand-1);
  text-decoration: none;
  white-space: nowrap;
}
.notice__more:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}
@keyframes notice-scroll {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(-50%);
  }
}
@media (prefers-reduced-motion: reduce) {
  .notice__reel {
    animation: none;
  }
  .notice__item:not(:first-child) {
    display: none;
  }
  .notice__item span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
}
@media (max-width: 640px) {
  .notice {
    padding: 0 16px;
    gap: 10px;
  }
  .notice__more {
    display: none;
  }
}
</style>
