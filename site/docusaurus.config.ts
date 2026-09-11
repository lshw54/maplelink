import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";
import { themes as prismThemes } from "prism-react-renderer";
import { PRODUCTS } from "./src/demo/products";

/** The repo this site's source lives in; edit links and the GitHub icon point here. */
const REPO_URL = `https://github.com/${PRODUCTS.maplelink.repo}`;

/**
 * Where the site is served from. The Pages workflow passes `/<repo>/` until a
 * CNAME exists, then `/`. `url` becomes the custom domain once there is one.
 */
const BASE = process.env.SITE_BASE ?? "/";
const URL = process.env.SITE_URL ?? "https://lshw54.github.io";

const config: Config = {
  title: "MapleLink",
  tagline: "《新楓之谷》的第三方登入啟動器",
  favicon: "logo.png",
  url: URL,
  baseUrl: BASE,
  trailingSlash: false,
  onBrokenLinks: "throw",
  onBrokenAnchors: "throw",
  markdown: {
    hooks: { onBrokenMarkdownLinks: "throw" },
    // Explicit `{#id}` heading anchors and `:::tip Title` admonitions, which
    // the v4 defaults switch off.
    mdx1Compat: { headingIds: true, admonitions: true },
  },

  future: {
    v4: true,
    // Everything `faster: true` turns on, except the one flag that breaks the
    // dev server: rspack's persistent cache panics the process when it cannot
    // replace its own cache directory ("should have bucket pack metas"), which
    // on Windows happens whenever another process is holding a file in
    // node_modules/.cache. The build then dies and only a manual cache wipe
    // brings it back — not a trade worth a few seconds of startup.
    faster: {
      swcJsLoader: true,
      swcJsMinimizer: true,
      swcHtmlMinimizer: true,
      lightningCssMinimizer: true,
      mdxCrossCompilerCache: true,
      rspackBundler: true,
      rspackPersistentCache: false,
      ssgWorkerThreads: true,
      gitEagerVcs: true,
    },
  },

  i18n: {
    defaultLocale: "zh-TW",
    locales: ["zh-TW", "zh-CN", "en"],
    // Short labels: the locale dropdown prints the current locale's label in
    // the navbar, and "繁體中文" next to the search box left no room to breathe.
    localeConfigs: {
      "zh-TW": { label: "繁中", htmlLang: "zh-TW" },
      "zh-CN": { label: "简中", htmlLang: "zh-CN" },
      en: { label: "EN", htmlLang: "en-US" },
    },
  },

  presets: [
    [
      "classic",
      {
        docs: {
          // Download, guide, FAQ and announcements are top-level pages, not a
          // docs tree, so they live at the root with no sidebar.
          routeBasePath: "/",
          sidebarPath: "./sidebars.ts",
          editUrl: `${REPO_URL}/edit/main/site/`,
          showLastUpdateTime: true,
        },
        blog: false,
        theme: { customCss: "./src/css/custom.css" },
      } satisfies Preset.Options,
    ],
  ],

  themes: [
    [
      "@easyops-cn/docusaurus-search-local",
      {
        hashed: true,
        language: ["zh", "en"],
        docsRouteBasePath: "/",
        indexBlog: false,
        // The playable demos carry UI copy that is not documentation.
        ignoreCssSelectors: [".demo", ".release"],
        highlightSearchTermsOnTargetPage: false,
        searchBarShortcutHint: false,
      },
    ],
  ],

  themeConfig: {
    colorMode: { defaultMode: "dark", respectPrefersColorScheme: true },
    navbar: {
      title: "MapleLink",
      logo: { alt: "", src: "logo.png" },
      items: [
        { to: "/download", label: "下載", position: "right" },
        { to: "/guide", label: "新手教學", position: "right" },
        { to: "/client", label: "遊戲客戶端", position: "right" },
        { to: "/faq", label: "常見問題", position: "right" },
        { to: "/announcements", label: "公告", position: "right" },
        { type: "localeDropdown", position: "right" },
        { href: REPO_URL, position: "right", className: "navbar-github", "aria-label": "GitHub" },
      ],
    },
    // The landing page carries its own footer; the theme footer would repeat it.
    footer: undefined,
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ["powershell"],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
