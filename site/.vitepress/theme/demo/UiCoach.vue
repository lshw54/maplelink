<script setup lang="ts">
/** The one-line prompt under a demo: what to click next, or that it is done. */
defineProps<{ done?: boolean }>();
</script>

<template>
  <div class="coach" :class="{ 'coach--done': done }">
    <span class="coach__text"><slot /></span>
    <slot name="action" />
  </div>
</template>

<style scoped>
.coach {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin-top: 10px;
  padding: 7px 14px;
  border-radius: 999px;
  font-size: 13px;
  line-height: 1.5;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}
.coach--done {
  background: rgba(74, 222, 128, 0.12);
  color: #15803d;
}
.dark .coach--done {
  color: #8ff0b0;
}
.coach :deep(button) {
  font: inherit;
  font-weight: 700;
  color: inherit;
  background: none;
  border: 0;
  padding: 0;
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
}
</style>
