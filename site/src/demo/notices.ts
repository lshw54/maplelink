/**
 * Headlines for the ticker above the nav, newest first. Each entry mirrors a
 * section on the announcements page (its `id` is that section's anchor), so a
 * click lands on the full text. Keep this list to what is worth a glance;
 * the announcements page keeps everything.
 */
export interface Notice {
  /** Anchor of the matching heading in announcements.mdx, all locales. */
  id: string;
  date: string;
  title: { "zh-TW": string; "zh-CN": string; en: string };
  /**
   * `notice` (the default) is about the project itself; `update` is what
   * changed in a release. Only notices ride the ticker — a release note
   * arrives with every version and would push the rest out of view.
   */
  kind?: "notice" | "update";
}

/** The kind of `n`, with the default applied. */
export function noticeKind(n: Notice): "notice" | "update" {
  return n.kind ?? "notice";
}

export const NOTICES: Notice[] = [
  {
    id: "v0-6-2",
    date: "2026-09",
    kind: "update",
    title: {
      "zh-TW": "MapleLink v0.6.2：客戶端管理員可修復到最新小版本、台灣懷舊服快捷登入、Enter 鍵取得 OTP。",
      "zh-CN": "MapleLink v0.6.2：客户端管理器可修复到最新小版本、台湾怀旧服快捷登录、Enter 键获取 OTP。",
      en: "MapleLink v0.6.2: the client manager repairs to the latest minor update, TW Classic sign-in shortcuts, and Enter for the OTP.",
    },
  },
  {
    id: "v0-6-1",
    date: "2026-09",
    kind: "update",
    title: {
      "zh-TW": "MapleLink v0.6.1：遊戲客戶端管理員支援加速器、修復進度即時顯示、可清理額外檔案。",
      "zh-CN": "MapleLink v0.6.1：游戏客户端管理器支持加速器、修复进度实时显示、可清理额外文件。",
      en: "MapleLink v0.6.1: the client manager works behind accelerators, shows repairs live, and cleans up extra files.",
    },
  },
  {
    id: "v0-6-0",
    date: "2026-09",
    kind: "update",
    title: {
      "zh-TW": "MapleLink v0.6.0：遊戲客戶端管理員、內建 beanfun 瀏覽器、官方網站上線。",
      "zh-CN": "MapleLink v0.6.0：游戏客户端管理器、内置 beanfun 浏览器、官方网站上线。",
      en: "MapleLink v0.6.0: game client manager, built-in beanfun browser, and the official website.",
    },
  },
  {
    id: "2026-09-download-source",
    date: "2026-09",
    title: {
      "zh-TW":
        "有重新打包的啟動器在外流傳。只從本站或 GitHub Releases 下載，其他來源都不是我們發出的。",
      "zh-CN":
        "有重新打包的启动器在外流传。只从本站或 GitHub Releases 下载，其他来源都不是我们发出的。",
      en: "Repackaged copies of the launcher are circulating. Download only from this site or GitHub Releases.",
    },
  },
  {
    id: "2026-07-dual-track",
    date: "2026-07",
    title: {
      "zh-TW":
        "MapleLink 與 Beanfun 雙線並行開發：只玩楓之谷用 MapleLink，其他橘子遊戲用 Beanfun。",
      "zh-CN":
        "MapleLink 与 Beanfun 双线并行开发：只玩枫之谷用 MapleLink，其他橘子游戏用 Beanfun。",
      en: "MapleLink and Beanfun are developed side by side: MapleLink for MapleStory only, Beanfun for every Gamania game.",
    },
  },
];
