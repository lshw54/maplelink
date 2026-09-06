import { computed } from "vue";
import { useData } from "vitepress";

/** Three-way string picker for the demo components: zh-TW, zh-CN, en-US. */
export function useT() {
  const { lang } = useData();
  const key = computed(() =>
    lang.value === "zh-CN" ? 1 : lang.value.startsWith("en") ? 2 : 0,
  );
  return (zhTW: string, zhCN: string, en: string) => [zhTW, zhCN, en][key.value];
}
