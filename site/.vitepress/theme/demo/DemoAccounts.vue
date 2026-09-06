<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiAccountPanel from "./UiAccountPanel.vue";

/** The account list and OTP panel on their own, for the guide's step 6. */
const t = useT();
const accounts = computed(() => [
  { initial: t("角", "角", "A"), name: t("角色一", "角色一", "Alpha") },
  { initial: t("角", "角", "B"), name: t("角色二", "角色二", "Bravo") },
  { initial: t("倉", "仓", "M"), name: t("倉庫號", "仓库号", "Mule") },
  { initial: t("小", "小", "S"), name: t("小號", "小号", "Spare") },
]);
/** 0 pick · 1 fetch · 2 done */
const step = ref(0);
const key = ref(0);
const coach = computed(
  () =>
    [
      t("點一個帳號卡片選擇帳號。", "点一个账号卡片选择账号。", "Click an account card to select it."),
      t("按右下角的 ↻ 取得一次性密碼。", "点右下角的 ↻ 获取一次性密码。", "Click ↻ at the bottom right for a one-time password."),
      t("取得的 OTP 會自動貼進遊戲登入視窗，或複製到剪貼簿。", "获取的 OTP 会自动贴进游戏登录窗口，或复制到剪贴板。", "The OTP is typed into the game's sign-in window, or copied to the clipboard."),
    ][step.value],
);
</script>

<template>
  <div class="demo">
    <UiFrame :width="456" :height="440">
      <div class="ml panel">
        <UiAccountPanel
          :key="key"
          :accounts="accounts"
          :user="t('主帳', '主账', 'Main')"
          :beans="120"
          :hint="step === 0 ? 'pick' : step === 1 ? 'fetch' : null"
          @pick="step === 0 && (step = 1)"
          @fetched="step = 2"
        />
      </div>
    </UiFrame>
    <UiCoach :done="step === 2">
      {{ coach }}
      <template v-if="step === 2" #action><button @click="step = 0; key++">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.panel {
  width: 100%;
  height: 100%;
  border: 1px solid var(--ml-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);
}
</style>
