/**
 * Headlines for the ticker above the nav, newest first. Each entry mirrors a
 * section on the announcements page (its `id` is that section's anchor), so a
 * click lands on the full text. Keep this list to what is worth a glance;
 * the announcements page keeps everything.
 */
export interface Notice {
  /** Anchor of the matching heading in announcements.md, all locales. */
  id: string;
  date: string;
  title: { "zh-TW": string; "zh-CN": string; "en-US": string };
}

export const NOTICES: Notice[] = [
  {
    id: "2026-09-download-source",
    date: "2026-09",
    title: {
      "zh-TW": "有重新打包的啟動器在外流傳。只從本站或 GitHub Releases 下載，其他來源都不是我們發出的。",
      "zh-CN": "有重新打包的启动器在外流传。只从本站或 GitHub Releases 下载，其他来源都不是我们发出的。",
      "en-US": "Repackaged copies of the launcher are circulating. Download only from this site or GitHub Releases.",
    },
  },
  {
    id: "2026-07-dual-track",
    date: "2026-07",
    title: {
      "zh-TW": "MapleLink 與 Beanfun 雙線並行開發：只玩楓之谷用 MapleLink，其他橘子遊戲用 Beanfun。",
      "zh-CN": "MapleLink 与 Beanfun 双线并行开发：只玩枫之谷用 MapleLink，其他橘子游戏用 Beanfun。",
      "en-US": "MapleLink and Beanfun are developed side by side: MapleLink for MapleStory only, Beanfun for every Gamania game.",
    },
  },
];
