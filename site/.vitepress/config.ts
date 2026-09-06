import { defineConfig } from "vitepress";
import { PRODUCTS } from "./products";

/** The repo this site's source lives in; edit links and the GitHub icon point here. */
const REPO_URL = `https://github.com/${PRODUCTS.maplelink.repo}`;

/** Set once the domain is bought; used for canonical URLs and the CNAME file. */
export const SITE_URL = "";

const editLink = (text: string) => ({ pattern: `${REPO_URL}/edit/main/site/:path`, text });

export default defineConfig({
  title: "MapleLink",
  head: [
    ["link", { rel: "icon", href: "/logo.png" }],
    ["meta", { name: "theme-color", content: "#e0432b" }],
  ],
  cleanUrls: true,
  lastUpdated: true,
  vite: {
    // The repo root tsconfig targets ES2024, which VitePress 1.x's esbuild does
    // not know; pin the target here so the root config is never consulted.
    esbuild: { tsconfigRaw: { compilerOptions: { target: "es2022" } } },
  },
  themeConfig: {
    logo: "/logo.png",
    socialLinks: [{ icon: "github", link: REPO_URL }],
    search: { provider: "local" },
    aside: true,
  },
  locales: {
    root: {
      label: "繁體中文",
      lang: "zh-TW",
      description: "《新楓之谷》的第三方登入啟動器：新手教學、唯一認可下載來源",
      themeConfig: {
        nav: [
          { text: "下載", link: "/download" },
          { text: "新手教學", link: "/guide/" },
          { text: "常見問題", link: "/faq" },
          { text: "公告", link: "/announcements" },
        ],
        outline: { label: "本頁目錄" },
        docFooter: { prev: "上一頁", next: "下一頁" },
        lastUpdated: { text: "最後更新" },
        editLink: editLink("在 GitHub 上編輯此頁"),
      },
    },
    "zh-CN": {
      label: "简体中文",
      lang: "zh-CN",
      description: "《新枫之谷》的第三方登录启动器：新手教学、唯一认可下载来源",
      themeConfig: {
        nav: [
          { text: "下载", link: "/zh-CN/download" },
          { text: "新手教学", link: "/zh-CN/guide/" },
          { text: "常见问题", link: "/zh-CN/faq" },
          { text: "公告", link: "/zh-CN/announcements" },
        ],
        outline: { label: "本页目录" },
        docFooter: { prev: "上一页", next: "下一页" },
        lastUpdated: { text: "最后更新" },
        editLink: editLink("在 GitHub 上编辑此页"),
      },
    },
    en: {
      label: "English",
      lang: "en-US",
      description: "Third-party sign-in launcher for MapleStory: beginner guide and the only recognised download source",
      themeConfig: {
        nav: [
          { text: "Download", link: "/en/download" },
          { text: "Guide", link: "/en/guide/" },
          { text: "FAQ", link: "/en/faq" },
          { text: "Announcements", link: "/en/announcements" },
        ],
        editLink: editLink("Edit this page on GitHub"),
      },
    },
  },
});
