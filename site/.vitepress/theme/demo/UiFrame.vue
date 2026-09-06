<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

/**
 * Scales a piece of UI drawn at its natural pixel size down to whatever width
 * the page gives it, never up. The frame is sized purely by aspect ratio; the
 * content is absolutely positioned on top and transform-scaled, so it can never
 * stretch the layout around it.
 */
const props = defineProps<{ width: number; height: number }>();
const frame = ref<HTMLElement | null>(null);
const scale = ref(1);
let ro: ResizeObserver | null = null;
onMounted(() => {
  if (!frame.value) return;
  const fit = () => (scale.value = Math.min(1, (frame.value?.clientWidth ?? props.width) / props.width));
  fit();
  ro = new ResizeObserver(fit);
  ro.observe(frame.value);
});
onBeforeUnmount(() => ro?.disconnect());
</script>

<template>
  <div
    ref="frame"
    class="ui-frame"
    :style="{ aspectRatio: `${width} / ${height}`, maxWidth: `${width}px` }"
  >
    <div class="ui-frame__content" :style="{ width: `${width}px`, height: `${height}px`, transform: `scale(${scale})` }">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.ui-frame {
  position: relative;
  width: 100%;
}
.ui-frame__content {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: top left;
}
</style>
