import React from "react";
import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import { useLocale, type Locale } from "../demo/i18n";
import LatestRelease from "../demo/LatestRelease";
import AppWindow from "../demo/AppWindow";

const BEANFUN = "https://github.com/pungin/Beanfun";
const REPO = "https://github.com/lshw54/maplelink";
const LICENSE = `${REPO}/blob/main/LICENSE`;
const QQ = "2157875454";

/** Per-locale copy. Paths are locale-relative; Link adds the base, Docusaurus adds the locale. */
const COPY: Record<Locale, Copy> = {
  "zh-TW": {
    title: "《新楓之谷》的第三方登入啟動器",
    eyebrow: "第三方 · 開放原始碼 · 非遊戲橘子官方",
    h1: ["《新楓之谷》的", "登入啟動器"],
    lead: "更快捷的帳號管理，更好的登入體驗。",
    fine1: ["只從本站或 GitHub Releases 下載。", "核對 SHA256", " · ", "新手教學"],
    fine2: ["也玩其他橘子遊戲？我們同時維護原版 ", "Beanfun", " 啟動器。"],
    help: [
      { h: "開始使用", items: [[["/guide", "新手教學"], "：下載、解壓、登入、取 OTP，約五分鐘"], ["需要 Windows 10 / 11（64 位元）及 WebView2"], ["不需要安裝，整個資料夾放哪裏都可以"]] },
      { h: "遇到問題", items: [[["/faq#webview2", "打不開、白屏、閃退"], "：安裝 WebView2"], [["/faq", "SmartScreen 警告、防毒誤判、加速器認不到"]], ["其他問題到 ", [`${REPO}/issues`, "GitHub Issues"], " 回報，或用 QQ 聯絡我們：" + QQ]] },
      { h: "下載與更新", items: [["只從本站或 ", [`${REPO}/releases`, "GitHub Releases"], " 下載"], ["執行前", ["/download#verify", "核對 SHA256"], "，不一致就刪除"], ["已在使用的話，程式會自動更新"]] },
    ],
    legal1: ["MapleLink 與 ", "Beanfun", " 均為第三方開放原始碼軟體，由同一批人並行維護，與遊戲橘子（Gamania）、beanfun! 及 Nexon 沒有任何關聯，也不是它們的官方產品。使用第三方啟動器可能違反遊戲服務條款，請自行評估風險；因使用本軟體造成的任何帳號或財物損失，開發團隊不承擔責任。"],
    legal2: ["記住的帳號密碼只儲存在你的電腦，以 Windows DPAPI 加密，不會傳送給開發團隊。程式碼以 ", "MIT 授權", "公開於 GitHub，任何人都可以查看與核對。"],
  },
  "zh-CN": {
    title: "《新枫之谷》的第三方登录启动器",
    eyebrow: "第三方 · 开放源代码 · 非游戏橘子官方",
    h1: ["《新枫之谷》的", "登录启动器"],
    lead: "更快捷的账号管理，更好的登录体验。",
    fine1: ["只从本站或 GitHub Releases 下载。", "核对 SHA256", " · ", "新手教学"],
    fine2: ["也玩其他橘子游戏？我们同时维护原版 ", "Beanfun", " 启动器。"],
    help: [
      { h: "开始使用", items: [[["/guide", "新手教学"], "：下载、解压、登录、取 OTP，约五分钟"], ["需要 Windows 10 / 11（64 位）及 WebView2"], ["不需要安装，整个文件夹放哪里都可以"]] },
      { h: "遇到问题", items: [[["/faq#webview2", "打不开、白屏、闪退"], "：安装 WebView2"], [["/faq", "SmartScreen 警告、杀毒误判、加速器认不到"]], ["其他问题到 ", [`${REPO}/issues`, "GitHub Issues"], " 反馈，或用 QQ 联系我们：" + QQ]] },
      { h: "下载与更新", items: [["只从本站或 ", [`${REPO}/releases`, "GitHub Releases"], " 下载"], ["运行前", ["/download#verify", "核对 SHA256"], "，不一致就删除"], ["已在使用的话，程序会自动更新"]] },
    ],
    legal1: ["MapleLink 与 ", "Beanfun", " 均为第三方开放源代码软件，由同一批人并行维护，与游戏橘子（Gamania）、beanfun! 及 Nexon 没有任何关联，也不是它们的官方产品。使用第三方启动器可能违反游戏服务条款，请自行评估风险；因使用本软件造成的任何账号或财物损失，开发团队不承担责任。"],
    legal2: ["记住的账号密码只保存在你的电脑，以 Windows DPAPI 加密，不会传送给开发团队。代码以 ", "MIT 许可", "公开于 GitHub，任何人都可以查看与核对。"],
  },
  en: {
    title: "Third-party sign-in launcher for MapleStory",
    eyebrow: "Third-party · Open source · Not a Gamania product",
    h1: ["The sign-in launcher", "for MapleStory"],
    lead: "Faster account management. A better sign-in experience.",
    fine1: ["Download only from this site or GitHub Releases. ", "Check the SHA256", " · ", "Beginner guide"],
    fine2: ["Play other Gamania games too? We also maintain the original ", "Beanfun", " launcher."],
    help: [
      { h: "Getting started", items: [[["/guide", "Beginner guide"], ": download, unpack, sign in, get an OTP. About five minutes"], ["Needs Windows 10 / 11 (64-bit) and WebView2"], ["Nothing to install. Keep the folder anywhere"]] },
      { h: "Something wrong?", items: [[["/faq#webview2", "Won't open, blank window, closes at once"], ": install WebView2"], [["/faq", "SmartScreen warning, antivirus flag, accelerator can't see it"]], ["Anything else: open an issue on ", [`${REPO}/issues`, "GitHub"], ", or reach us on QQ: " + QQ]] },
      { h: "Downloads and updates", items: [["Download only from this site or ", [`${REPO}/releases`, "GitHub Releases"]], [["/download#verify", "Check the SHA256"], " before running; delete it if it differs"], ["Once installed, the app updates itself"]] },
    ],
    legal1: ["MapleLink and ", "Beanfun", " are third-party open-source software maintained side by side by the same people. They are not affiliated with Gamania, beanfun! or Nexon and are not their official products. Using a third-party launcher may breach a game's terms of service; assess the risk yourself. The developers accept no liability for any account or financial loss arising from use of this software."],
    legal2: ["Remembered credentials stay on your PC, encrypted with Windows DPAPI, and are never sent to the developers. The source is published on GitHub under the ", "MIT licence", " for anyone to read and verify."],
  },
};

type Frag = string | [href: string, text: string];
interface Copy {
  title: string;
  eyebrow: string;
  h1: [string, string];
  lead: string;
  fine1: [string, string, string, string];
  fine2: [string, string, string];
  help: { h: string; items: Frag[][] }[];
  legal1: [string, string, string];
  legal2: [string, string, string];
}

function A({ href, children }: { href: string; children: React.ReactNode }) {
  return href.startsWith("http") ? <a href={href}>{children}</a> : <Link to={href}>{children}</Link>;
}

export default function Home() {
  const locale = useLocale();
  const c = COPY[locale];
  return (
    <Layout title={c.title} description={c.title}>
      <div className="landing">
        <section className="landing-hero">
          <div>
            <span className="landing-eyebrow">{c.eyebrow}</span>
            <h1>
              {c.h1[0]}
              <br />
              {c.h1[1]}
            </h1>
            <p className="landing-lead">{c.lead}</p>
            <LatestRelease product="maplelink" variant="hero" />
            <p className="landing-fine">
              {c.fine1[0]}
              <Link to="/download#verify">{c.fine1[1]}</Link>
              {c.fine1[2]}
              <Link to="/guide">{c.fine1[3]}</Link>
              <br />
              {c.fine2[0]}
              <a href={BEANFUN}>{c.fine2[1]}</a>
              {c.fine2[2]}
            </p>
          </div>
          <div className="landing-window">
            <AppWindow />
          </div>
        </section>

        <section className="landing-help">
          {c.help.map((col) => (
            <div key={col.h}>
              <h2>{col.h}</h2>
              <ul>
                {col.items.map((item, i) => (
                  <li key={i}>
                    {item.map((f, j) => (typeof f === "string" ? f : <A key={j} href={f[0]}>{f[1]}</A>))}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </section>

        <section className="landing-legal">
          <p>
            {c.legal1[0]}
            <a href={BEANFUN}>{c.legal1[1]}</a>
            {c.legal1[2]}
          </p>
          <p>
            {c.legal2[0]}
            <a href={LICENSE}>{c.legal2[1]}</a>
            {c.legal2[2]}
          </p>
        </section>
      </div>
    </Layout>
  );
}

