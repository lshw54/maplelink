<script setup lang="ts">
import { computed, ref } from "vue";
import { useT } from "./i18n";
import UiFrame from "./UiFrame.vue";
import UiCoach from "./UiCoach.vue";
import UiTitlebar from "./UiTitlebar.vue";

/**
 * The web-launch page (560×640), reached from the globe button on the sign-in
 * page. One big switch, two behaviour toggles, a self-check list.
 */
const t = useT();
const enabled = ref(false);
const autoLaunch = ref(true);
const autoPaste = ref(true);
const msg = ref(false);

function toggle() {
  enabled.value = !enabled.value;
  msg.value = enabled.value;
}
const coach = computed(() =>
  enabled.value
    ? t("之後在 beanfun 官網登入並按「開始遊戲」，會改由 MapleLink 開遊戲並貼入 OTP。關掉開關就還原官方。", "之后在 beanfun 官网登录并点「开始游戏」，会改由 MapleLink 开游戏并贴入 OTP。关掉开关就还原官方。", "From now on, Start Game on the beanfun website launches through MapleLink and pastes the OTP. Turn the switch off to restore the original.")
    : t("入口在登入頁登入鈕右邊的地球圖示。下方四項自我檢查全綠後，開啟最上面的開關。", "入口在登录页登录钮右边的地球图标。下方四项自我检查全绿后，开启最上面的开关。", "Reached from the globe icon beside the sign-in button. Once the four checks below are green, turn on the top switch."),
);
</script>

<template>
  <div class="demo">
    <UiFrame :width="560" :height="640">
      <div class="ml wl">
        <div class="ml-glow"></div>
        <UiTitlebar page="main" region="HK" />
        <div class="wl__head"><span>🌐</span><b>{{ t("網頁登入開機（一鍵）", "网页登录开机（一键）", "Web-login launch (one-click)") }}</b><span class="wl__back">{{ t("返回", "返回", "Back") }}</span></div>
        <div class="wl__body">
          <p class="wl__intro">{{ t("只能用網頁登入（UU／VPN）時開啟。啟用後，在官網按「開始遊戲」會改由 MapleLink 開機，並自動填入帳號／OTP。", "只能用网页登录（UU／VPN）时开启。启用后，在官网点「开始游戏」会改由 MapleLink 开机，并自动填入账号／OTP。", "For when you can only log in via the website (UU / VPN). Once enabled, Start Game on the official site launches through MapleLink and auto-fills the account / OTP.") }}</p>

          <div class="wl__status" :class="enabled ? 'wl__status--ok' : 'wl__status--info'">
            <span>{{ enabled ? "✅" : "🔹" }}</span>
            {{ enabled ? t("全部就緒，網頁開機已開啟", "全部就绪，网页开机已开启", "All set. Web launch is enabled") : t("環境已就緒，開啟開關即可使用", "环境已就绪，开启开关即可使用", "Environment ready. Turn on the switch to use it") }}
          </div>

          <div class="wl__enable" :class="{ 'wl__enable--on': enabled }">
            <div>
              <b>{{ t("啟用網頁開機攔截", "启用网页开机拦截", "Enable web-launch interception") }}</b>
              <small>{{ enabled ? t("已開啟：官網「開始遊戲」會由 MapleLink 開機並自動填帳號／OTP。", "已开启：官网「开始游戏」会由 MapleLink 开机并自动填账号／OTP。", "On: the website's Start Game now launches through MapleLink and auto-fills the account / OTP.") : t("開啟後，官網「開始遊戲」會改由 MapleLink 開機並自動填帳號／OTP；關閉會還原官方。", "开启后，官网「开始游戏」会改由 MapleLink 开机并自动填账号／OTP；关闭会还原官方。", "When on, the website's Start Game launches through MapleLink and auto-fills the account / OTP; off restores the original.") }}</small>
            </div>
            <button class="wl__toggle wl__toggle--lg" :class="{ 'wl__toggle--on': enabled, 'ml-hint': !enabled }" @click="toggle"><i></i></button>
          </div>
          <p v-if="msg" class="wl__msg">{{ t("已啟用：之後在官網按「開始遊戲」就會由 MapleLink 開機。", "已启用：之后在官网点「开始游戏」就会由 MapleLink 开机。", "Enabled: clicking Start Game on the website will now launch through MapleLink.") }}</p>

          <div class="wl__sec">{{ t("啟動行為", "启动行为", "Launch behaviour") }}</div>
          <div class="wl__row"><div><b>{{ t("自動開啟遊戲", "自动打开游戏", "Auto-open the game") }}</b><small>{{ t("在官網按「開始遊戲」時自動開啟 MapleStory。", "在官网点「开始游戏」时自动打开 MapleStory。", "Open MapleStory automatically when you click Start Game on the website.") }}</small></div><button class="wl__toggle" :class="{ 'wl__toggle--on': autoLaunch }" @click="autoLaunch = !autoLaunch"><i></i></button></div>
          <div class="wl__row"><div><b>{{ t("自動填入帳號／OTP", "自动填入账号／OTP", "Auto-fill account / OTP") }}</b><small>{{ t("遊戲登入視窗出現後，自動填入帳號與 OTP。", "游戏登录窗口出现后，自动填入账号与 OTP。", "Fill the account and OTP once the game login window appears.") }}</small></div><button class="wl__toggle" :class="{ 'wl__toggle--on': autoPaste }" @click="autoPaste = !autoPaste"><i></i></button></div>

          <div class="wl__sec">{{ t("自我檢查", "自我检查", "Self-check") }}</div>
          <div class="wl__check"><i>✓</i><div><b>{{ t("程式名稱", "程序名称", "App executable") }}</b><small>MapleLink.exe</small></div></div>
          <div class="wl__check"><i>✓</i><div><b>{{ t("遊戲路徑", "游戏路径", "Game path") }}</b><small>C:\Games\MapleStory</small></div></div>
          <div class="wl__check"><i>✓</i><div><b>{{ t("語系轉換器（LR）", "语系转换器（LR）", "Locale Remulator (LR)") }}</b><small>{{ t("已就緒，可用 LR 啟動 MapleStory", "已就绪，可用 LR 启动 MapleStory", "Ready. LR can launch MapleStory") }}</small></div></div>
          <div class="wl__check"><i>✓</i><div><b>{{ t("官方啟動器（Gamania）", "官方启动器（Gamania）", "Official launcher (Gamania)") }}</b><small>{{ t("已偵測到 gamania Games Manager", "已检测到 gamania Games Manager", "Detected gamania Games Manager") }}</small></div></div>
        </div>
      </div>
    </UiFrame>
    <UiCoach :done="enabled">
      {{ coach }}
      <template v-if="enabled" #action><button @click="enabled = false; msg = false">{{ t("再試一次", "再试一次", "Try again") }}</button></template>
    </UiCoach>
  </div>
</template>

<style scoped>
.demo {
  margin: 20px 0 28px;
}
.wl {
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
.wl__head {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  font-size: 13px;
  font-weight: 600;
}
.wl__head b {
  flex: 1;
}
.wl__back {
  padding: 5px 12px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  font-size: 12px;
  letter-spacing: 1px;
  text-transform: uppercase;
  color: var(--ml-dim);
}
.wl__body {
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
  overflow: hidden;
}
.wl__intro {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ml-dim);
}
.wl__status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: 10px;
  border: 1px solid;
  font-size: 12px;
  font-weight: 600;
}
.wl__status--info {
  border-color: rgba(96, 165, 250, 0.3);
  background: rgba(96, 165, 250, 0.08);
  color: #93c5fd;
}
.wl__status--ok {
  border-color: rgba(74, 222, 128, 0.3);
  background: rgba(74, 222, 128, 0.08);
  color: var(--ml-green);
}
.wl__enable,
.wl__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
}
.wl__enable {
  border-radius: 12px;
  padding: 12px 14px;
}
.wl__enable--on {
  border-color: rgba(232, 162, 58, 0.4);
  background: rgba(232, 162, 58, 0.06);
}
.wl__enable b,
.wl__row b,
.wl__check b {
  display: block;
  font-size: 12px;
  font-weight: 600;
}
.wl__enable b {
  font-size: 13px;
}
.wl__enable small,
.wl__row small,
.wl__check small {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.45;
  color: var(--ml-dim);
}
.wl__msg {
  margin: -6px 4px 0;
  font-size: 11px;
  color: var(--ml-dim);
}
.wl__sec {
  margin-top: 4px;
  padding: 0 2px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 2px;
  text-transform: uppercase;
  color: var(--ml-faint);
}
.wl__toggle {
  position: relative;
  flex: none;
  width: 44px;
  height: 24px;
  border-radius: 999px;
  background: var(--ml-border);
  transition: background 0.15s;
}
.wl__toggle i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.15s;
}
.wl__toggle--on {
  background: var(--ml-accent);
}
.wl__toggle--on i {
  transform: translateX(20px);
}
.wl__check {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 14px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: #141619;
}
.wl__check i {
  width: 24px;
  height: 24px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: rgba(74, 222, 128, 0.15);
  color: var(--ml-green);
  font-style: normal;
  font-size: 13px;
}
</style>
