import { h } from "vue";
import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import LatestRelease from "./LatestRelease.vue";
import AppWindow from "./AppWindow.vue";
import NoticeBar from "./NoticeBar.vue";
import DemoLogin from "./demo/DemoLogin.vue";
import DemoAccounts from "./demo/DemoAccounts.vue";
import DemoPlay from "./demo/DemoPlay.vue";
import DemoSmartScreen from "./demo/DemoSmartScreen.vue";
import DemoSettings from "./demo/DemoSettings.vue";
import DemoBackup from "./demo/DemoBackup.vue";
import DemoAnnouncement from "./demo/DemoAnnouncement.vue";
import DemoRename from "./demo/DemoRename.vue";
import DemoBlank from "./demo/DemoBlank.vue";
import DemoSessions from "./demo/DemoSessions.vue";
import DemoWebLaunch from "./demo/DemoWebLaunch.vue";
import DemoLoginError from "./demo/DemoLoginError.vue";
import DemoNodeErrors from "./demo/DemoNodeErrors.vue";
import "./demo/ml.css";
import "./custom.css";

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      "layout-top": () => h(NoticeBar),
    });
  },
  enhanceApp({ app }) {
    app.component("LatestRelease", LatestRelease);
    app.component("AppWindow", AppWindow);
    app.component("DemoLogin", DemoLogin);
    app.component("DemoAccounts", DemoAccounts);
    app.component("DemoPlay", DemoPlay);
    app.component("DemoSmartScreen", DemoSmartScreen);
    app.component("DemoSettings", DemoSettings);
    app.component("DemoBackup", DemoBackup);
    app.component("DemoAnnouncement", DemoAnnouncement);
    app.component("DemoRename", DemoRename);
    app.component("DemoBlank", DemoBlank);
    app.component("DemoSessions", DemoSessions);
    app.component("DemoWebLaunch", DemoWebLaunch);
    app.component("DemoLoginError", DemoLoginError);
    app.component("DemoNodeErrors", DemoNodeErrors);
  },
} satisfies Theme;
