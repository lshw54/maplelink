<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiToolbox from "./UiToolbox.vue";

/**
 * Toolbox → Account Manager, with the Export / Import pair. Export opens the
 * real dialog's layout: a warning, an encrypt tick box, a password field.
 */
const t = useT();
const tab = ref("account_manager");
const dialog = ref<"none" | "export">("none");
const encrypt = ref(true);
const pass = ref("");
const toast = ref(false);
const done = ref(false);

function doExport() {
  dialog.value = "none";
  toast.value = true;
  done.value = true;
  setTimeout(() => (toast.value = false), 1800);
}
function reset() {
  done.value = false;
  pass.value = "";
  dialog.value = "none";
  tab.value = "account_manager";
}
const coach = computed(() => {
  if (done.value) return t("備份檔存到你選的位置。在新電腦按「匯入資料」選這個檔就可以。", "备份文件存到你选的位置。在新电脑点「导入数据」选这个文件就可以。", "The backup is saved where you chose. On the new PC, click Import data and pick that file.");
  if (dialog.value === "export") return t("建議勾選加密並設密碼，明文檔任何人都讀得到密碼。然後按「匯出」。", "建议勾选加密并设密码，明文文件任何人都能读到密码。然后点「导出」。", "Tick encrypt and set a password; a plain file exposes every password. Then click Export.");
  if (tab.value !== "account_manager") return t("按左邊的「帳號管理」。", "点左边的「账号管理」。", "Click Account Manager on the left.");
  return t("右上角按「匯出資料」。", "右上角点「导出数据」。", "Click Export data at the top right.");
});
</script>

<template>
  <div class="demo">
    <UiFrame :width="750" :height="490">
      <UiToolbox v-model:tab="tab">
        <div v-if="tab === 'account_manager'" class="am">
          <div class="am__head">
            <span class="am__title">{{ t("已儲存帳號", "已保存账号", "Saved Accounts") }}</span>
            <span class="am__spacer"></span>
            <button class="am__io" :class="{ 'ml-hint': !done && dialog === 'none' }" @click="dialog = 'export'">⤓ {{ t("匯出資料", "导出数据", "Export data") }}</button>
            <button class="am__io">⤒ {{ t("匯入資料", "导入数据", "Import data") }}</button>
          </div>
          <div class="am__list">
            <div class="am__row">
              <span class="am__avatar ml-grad">{{ t("主", "主", "M") }}</span>
              <span class="am__name">{{ t("主帳", "主账", "Main") }}</span>
              <span class="am__pill">HK</span>
              <span class="am__pill am__pill--ok">{{ t("已儲存密碼", "已保存密码", "Password Saved") }}</span>
              <span class="am__chev">▾</span>
            </div>
            <div class="am__row">
              <span class="am__avatar ml-grad">{{ t("台", "台", "T") }}</span>
              <span class="am__name">{{ t("台服", "台服", "TW alt") }}</span>
              <span class="am__pill">TW</span>
              <span class="am__pill am__pill--ok">{{ t("已儲存密碼", "已保存密码", "Password Saved") }}</span>
              <span class="am__chev">▾</span>
            </div>
          </div>
        </div>
        <div v-else class="am__empty">{{ t("此示範只包含「帳號管理」。", "此示范只包含「账号管理」。", "This demo covers Account Manager only.") }}</div>

        <div v-if="dialog === 'export'" class="am__overlay">
          <div class="am__dlg">
            <div class="am__dlg-head">{{ t("匯出資料", "导出数据", "Export data") }}<button @click="dialog = 'none'">✕</button></div>
            <p class="am__warn">{{ t("會匯出所有已儲存帳號（含密碼）與自訂設定。明文檔案任何人都讀得到密碼，請妥善保管；建議勾選加密。", "会导出所有已保存账号（含密码）与自定义设置。明文文件任何人都能读到密码，请妥善保管；建议勾选加密。", "Exports all saved accounts (including passwords) and customizations. A plaintext file exposes passwords to anyone who opens it. Keep it safe, or tick encrypt.") }}</p>
            <label class="am__check"><input v-model="encrypt" type="checkbox" /> {{ t("用密碼加密（AES-256）", "用密码加密（AES-256）", "Encrypt with a password (AES-256)") }}</label>
            <input v-if="encrypt" v-model="pass" class="am__input" type="password" :placeholder="t('輸入密碼', '输入密码', 'Enter password')" />
            <div class="am__dlg-actions">
              <button class="am__cancel" @click="dialog = 'none'">{{ t("取消", "取消", "Cancel") }}</button>
              <button class="am__ok" :class="{ 'am__ok--off': encrypt && pass.length < 4, 'ml-hint': !encrypt || pass.length >= 4 }" :disabled="encrypt && pass.length < 4" @click="doExport">{{ t("匯出", "导出", "Export") }}</button>
            </div>
          </div>
        </div>
        <div class="am__toast" :class="{ 'am__toast--show': toast }">✓ {{ t("已匯出", "已导出", "Exported") }}</div>
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
.am {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.am__empty {
  padding: 24px 4px;
  font-size: 12px;
  color: var(--ml-dim);
}
.am__head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.am__title {
  padding: 0 4px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-faint);
}
.am__spacer {
  flex: 1;
}
.am__io {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
  font-size: 11px;
  font-weight: 600;
  color: var(--ml-dim);
}
.am__io:hover {
  color: var(--ml-accent);
  border-color: var(--ml-accent);
}
.am__list {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: #141619;
  overflow: hidden;
}
.am__row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
}
.am__row + .am__row {
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}
.am__avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
}
.am__name {
  flex: 1;
  font-size: 11.5px;
  font-weight: 500;
}
.am__pill {
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--ml-surface-hover);
  font-size: 10.5px;
  font-weight: 600;
  color: var(--ml-dim);
}
.am__pill--ok {
  background: rgba(74, 222, 128, 0.1);
  color: var(--ml-green);
}
.am__chev {
  color: var(--ml-faint);
  font-size: 11px;
}
.am__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
}
.am__dlg {
  width: 340px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 20px 18px;
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
}
.am__dlg-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 14px;
  font-weight: 700;
}
.am__dlg-head button {
  color: var(--ml-faint);
  font-size: 14px;
}
.am__warn {
  margin: 0;
  padding: 8px 12px;
  border-radius: 10px;
  border: 1px solid rgba(234, 179, 8, 0.3);
  background: rgba(234, 179, 8, 0.06);
  font-size: 11px;
  line-height: 1.5;
  color: #eab308;
}
.am__check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  cursor: pointer;
}
.am__check input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: var(--ml-accent);
}
.am__input {
  width: 100%;
  padding: 8px 12px;
  border-radius: 8px;
  border: 1px solid var(--ml-border);
  background: var(--ml-surface);
  font-size: 12px;
  outline: none;
}
.am__input:focus {
  border-color: var(--ml-accent);
}
.am__dlg-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.am__cancel {
  padding: 6px 12px;
  border-radius: 8px;
  font-size: 12px;
  color: var(--ml-dim);
}
.am__cancel:hover {
  background: var(--ml-surface-hover);
}
.am__ok {
  padding: 6px 16px;
  border-radius: 8px;
  background: var(--ml-accent);
  color: #fff;
  font-size: 12px;
  font-weight: 600;
}
.am__ok--off {
  opacity: 0.4;
  cursor: default;
}
.am__toast {
  position: absolute;
  left: 50%;
  bottom: 16px;
  transform: translate(-50%, 8px);
  padding: 6px 12px;
  border-radius: 8px;
  background: var(--ml-green);
  color: #062b14;
  font-size: 12px;
  font-weight: 600;
  opacity: 0;
  transition: opacity 0.2s, transform 0.2s;
  pointer-events: none;
}
.am__toast--show {
  opacity: 1;
  transform: translate(-50%, 0);
}
</style>
