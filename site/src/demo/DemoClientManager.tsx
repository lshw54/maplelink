import React, { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoClientManager.module.css";

type Phase = "idle" | "scanning" | "scanned" | "downloading" | "done";
type Filter = "all" | "issues" | "ok";

/**
 * The two files this walk-through pretends are broken. `from` and `span` are
 * the share of the repair's overall progress during which each one downloads,
 * so the two overlap the way parallel downloads do.
 */
const BROKEN = [
  {
    path: "Data/Map/Map001.wz",
    size: "412 MB",
    label: { tw: "檔案損壞", cn: "文件损坏", en: "damaged" },
    why: {
      tw: "大小相同但內容與官方清單不符。",
      cn: "大小相同但内容与官方清单不符。",
      en: "Same size, contents differ.",
    },
    from: 0,
    span: 60,
  },
  {
    path: "Data/Mob/Mob002.wz",
    size: "268 MB",
    label: { tw: "缺失", cn: "缺失", en: "missing" },
    why: { tw: "本機找不到這個檔案。", cn: "本机找不到这个文件。", en: "Not on disk." },
    from: 25,
    span: 75,
  },
];

const FINE = [
  "BlackCipher/BlackCall64.aes",
  "Canvas.dll",
  "Data/Base/Base.wz",
  "Data/Character/Character.wz",
  "MapleStory.exe",
];

/** How far one broken file has got, 0–1, at the repair's overall `pct`. */
function fileProgress(f: (typeof BROKEN)[number], pct: number): number {
  return Math.min(1, Math.max(0, (pct - f.from) / f.span));
}

/**
 * Toolbox → Game client manager: check the install, then fetch back only the
 * files that do not match. The numbers are a scripted example, not a live run.
 */
export default function DemoClientManager() {
  const t = useT();
  const [tab, setTab] = useState<"verify" | "download">("verify");
  const [phase, setPhase] = useState<Phase>("idle");
  const [pct, setPct] = useState(0);
  const [paused, setPaused] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [sourceTest, setSourceTest] = useState<"none" | "testing" | "done">("none");
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  // Drive whichever bar is running; pausing simply stops advancing it.
  useEffect(() => {
    if (phase !== "scanning" && phase !== "downloading") return;
    timer.current = setInterval(() => {
      setPct((p) => {
        if (paused) return p;
        const next = p + (phase === "scanning" ? 9 : 4);
        if (next >= 100) {
          setPhase(phase === "scanning" ? "scanned" : "done");
          return 100;
        }
        return next;
      });
    }, 140);
    return () => {
      if (timer.current) clearInterval(timer.current);
    };
  }, [phase, paused]);

  // The source test answers after a moment, like the real one.
  useEffect(() => {
    if (sourceTest !== "testing") return;
    const id = setTimeout(() => setSourceTest("done"), 800);
    return () => clearTimeout(id);
  }, [sourceTest]);

  function start(next: Phase) {
    setPct(0);
    setPaused(false);
    setPhase(next);
    // A repair points the list at what it is repairing, as the app does.
    setFilter(next === "downloading" ? "issues" : "all");
  }

  function reset() {
    setPhase("idle");
    setPct(0);
    setPaused(false);
    setFilter("all");
    setTab("verify");
    setSourceTest("none");
  }

  const busy = phase === "scanning" || phase === "downloading";
  // The results stay on screen once a check has run, including while the
  // repair downloads — only a fresh check clears them.
  const scanned = phase !== "idle" && phase !== "scanning";
  const repairing = phase === "downloading" || phase === "done";
  const progressOf = (f: (typeof BROKEN)[number]) =>
    phase === "done" ? 1 : phase === "downloading" ? fileProgress(f, pct) : 0;
  const doneCount = BROKEN.filter((f) => repairing && progressOf(f) >= 1).length;
  const inFlight = BROKEN.find((f) => phase === "downloading" && progressOf(f) > 0 && progressOf(f) < 1);

  const coach = (() => {
    if (tab === "download")
      return t(
        "「自動下載安裝」選一個空資料夾就會整個抓下來；「手動」只給你種子檔，自己用 BT 工具下載。",
        "「自动下载安装」选一个空文件夹就会整个抓下来；「手动」只给你种子文件，自己用 BT 工具下载。",
        "Download and install fetches the whole client into an empty folder; Manual just hands you the torrent.",
      );
    if (phase === "idle")
      return sourceTest === "done"
        ? t(
            "兩個來源都連得上。加速器只連得上其中一個時，「自動」會記住可用的那個。現在按「開始檢查」。",
            "两个来源都连得上。加速器只连得上其中一个时，「自动」会记住可用的那个。现在点「开始检查」。",
            "Both sources answer. When an accelerator reaches only one, Automatic remembers that one. Now press Check files.",
          )
        : t(
            "確認上面的遊戲路徑，然後按「開始檢查」。連不上官方清單時，可以先按「測試來源」。",
            "确认上面的游戏路径，然后点「开始检查」。连不上官方清单时，可以先点「测试来源」。",
            "Check the game path above, then press Check files. If the manifest won't load, try Test sources first.",
          );
    if (phase === "scanning")
      return paused
        ? t("已暫停，按「繼續」接著檢查。", "已暂停，点「继续」接着检查。", "Paused — press Resume to carry on.")
        : t(
            "逐個檔案比對官方清單。隨時可以暫停或取消。",
            "逐个文件比对官方清单。随时可以暂停或取消。",
            "Comparing each file against the official manifest. Pause or cancel any time.",
          );
    if (phase === "scanned")
      return t(
        "兩個檔案對不上，其餘 1261 個正常。按「下載並修復」就只補回這兩個。",
        "两个文件对不上，其余 1261 个正常。点「下载并修复」就只补回这两个。",
        "Two files do not match; the other 1261 are fine. Download and repair fetches only those two.",
      );
    if (phase === "downloading")
      return t(
        "兩個檔案同時下載，各有自己的進度條。完成的會移到「正常」，「需要處理」會一路減少。",
        "两个文件同时下载，各有自己的进度条。完成的会移到「正常」，「需要处理」会一路减少。",
        "Both files download at once, each with its own bar. Finished ones move to Fine, so To fix counts down.",
      );
    return t(
      "修好了。按「全部」可以看到兩個檔案都標示為「已修復」。",
      "修好了。点「全部」可以看到两个文件都标示为「已修复」。",
      "Repaired. Press All to see both files marked as repaired.",
    );
  })();

  const headline = (() => {
    if (phase === "idle") return t("準備就緒", "准备就绪", "Ready");
    if (phase === "scanning")
      return paused ? t("已暫停", "已暂停", "Paused") : t("檢查中", "检查中", "Checking");
    if (phase === "scanned")
      return t("2 個檔案需要處理", "2 个文件需要处理", "2 files need attention");
    if (phase === "downloading")
      return paused ? t("已暫停", "已暂停", "Paused") : t("下載中", "下载中", "Downloading");
    return t("完成，已寫入 2 個檔案。", "完成，已写入 2 个文件。", "Done. 2 files written.");
  })();

  const detail = (() => {
    if (phase === "scanning") return `${Math.round((pct / 100) * 1263)} / 1263 · 67.7 GB`;
    if (phase === "downloading") {
      const mb = Math.round((pct / 100) * 680);
      const left = Math.max(0, Math.round(((100 - pct) / 100) * 680 / 74));
      return `${doneCount} / 2 · ${mb} MB / 680 MB · 74.1 MB/s · ${t("剩約", "剩约", "about")} 0:${String(left).padStart(2, "0")}`;
    }
    if (phase === "scanned")
      return t("需要下載 680 MB。", "需要下载 680 MB。", "680 MB to download.");
    return "";
  })();

  const tone = phase === "done" ? "ok" : phase === "scanned" ? "warn" : "busy";

  const issuesLeft = BROKEN.filter((f) => !(repairing && progressOf(f) >= 1));
  const rows =
    filter === "issues"
      ? scanned
        ? issuesLeft.map((f) => ({ kind: "broken" as const, f }))
        : []
      : filter === "ok"
        ? [
            ...BROKEN.filter((f) => repairing && progressOf(f) >= 1).map((f) => ({ kind: "broken" as const, f })),
            ...(scanned ? FINE.map((p) => ({ kind: "fine" as const, p })) : []),
          ]
        : [
            ...(scanned ? BROKEN.map((f) => ({ kind: "broken" as const, f })) : []),
            ...FINE.map((p) => ({ kind: "fine" as const, p })),
          ];

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={760} height={540}>
        <div className={clsx("ml", styles.win)}>
          <div className={styles.win__bar}>
            <span className={styles.win__name}>
              {t("遊戲客戶端管理", "游戏客户端管理", "Game client manager")}
            </span>
            <span className={styles.win__btns}>−&nbsp;&nbsp;×</span>
          </div>

          {/* Four tiles in place of a title: the game, the install, the disk, the route. */}
          <div className={styles.tiles}>
            <div className={styles.tile}>
              <span className={styles.tile__cap}>{t("新楓之谷", "新枫之谷", "MapleStory TW")}</span>
              <span className={styles.tile__val}>V282.3</span>
              <span className={styles.tile__sub}>
                {t("發布於", "发布于", "Published")} 2026/09/04 · {t("主程式", "主程序", "Executable")} 2026/09/16
              </span>
            </div>
            <div className={styles.tile}>
              <span className={styles.tile__cap}>{t("本機客戶端", "本机客户端", "Installed client")}</span>
              <span className={clsx(styles.tile__val, styles.ok)}>
                {t("已是最新版本", "已是最新版本", "Up to date")}
              </span>
              <span className={styles.tile__sub}>
                {phase === "idle"
                  ? t("尚未檢查檔案", "尚未检查文件", "Files not checked yet")
                  : phase === "scanning"
                    ? t("檢查中", "检查中", "Checking")
                    : `${1261 + doneCount} / 1263 ${t("個檔案相符", "个文件相符", "files match")}`}
              </span>
            </div>
            <div className={styles.tile}>
              <span className={styles.tile__cap}>{t("磁碟空間", "磁盘空间", "Disk space")} · C:</span>
              <span className={styles.tile__val}>{t("可用 240 GB", "可用 240 GB", "240 GB free")}</span>
              <span className={styles.tile__sub}>
                {scanned && phase !== "done"
                  ? t("修復需 680 MB", "修复需 680 MB", "Repair needs 680 MB")
                  : t("完整安裝需 67.7 GB", "完整安装需 67.7 GB", "Full install needs 67.7 GB")}
              </span>
            </div>
            <div className={styles.tile}>
              <span className={styles.tile__cap}>{t("網路", "网络", "Network")}</span>
              <span className={styles.tile__val}>
                {t("香港", "香港", "Hong Kong")} <span className={styles.ok}>· 38 ms</span>
              </span>
              <span className={styles.tile__sub}>
                {t("未設系統代理", "未设系统代理", "No system proxy")}
                <span className={styles.link}>{t("重新測試", "重新测试", "Test again")}</span>
              </span>
            </div>
          </div>

          <div className={styles.tabs}>
            {(["verify", "download"] as const).map((id) => (
              <button
                key={id}
                onClick={() => setTab(id)}
                className={clsx(styles.tab, tab === id && styles.tab_on)}
              >
                {id === "verify"
                  ? t("檢查與修復", "检查与修复", "Check & repair")
                  : t("下載與安裝", "下载与安装", "Download & install")}
              </button>
            ))}
          </div>

          {tab === "verify" ? (
            <div className={styles.body}>
              <div>
                <div className={styles.path}>
                  <span className={styles.path__label}>{t("遊戲路徑", "游戏路径", "Game path")}</span>
                  <span className={styles.path__box}>C:\Games\MapleStory</span>
                  <span className={styles.btnGhost}>{t("瀏覽", "浏览", "Browse")}</span>
                </div>
                <div className={styles.source}>
                  <span>{t("清單來源", "清单来源", "List from")}</span>
                  <span className={styles.select}>{t("自動", "自动", "Automatic")} ▾</span>
                  <button
                    className={styles.link}
                    disabled={sourceTest === "testing" || busy}
                    onClick={() => setSourceTest("testing")}
                  >
                    {sourceTest === "testing"
                      ? t("測試中…", "测试中…", "Testing…")
                      : t("測試來源", "测试来源", "Test sources")}
                  </button>
                  <span className={styles.link}>{t("開啟清單", "打开清单", "Open the manifest")}</span>
                </div>
                {sourceTest === "done" && (
                  <div className={styles.source__result}>
                    <span className={styles.ok}>beanfun ✓ {t("0.4 秒", "0.4 秒", "0.4 s")}</span>
                    <span className={styles.ok}>
                      {t("HiNet 目錄", "HiNet 目录", "HiNet catalog")} ✓ {t("0.6 秒", "0.6 秒", "0.6 s")}
                    </span>
                    <span className={styles.dim}>
                      {t("自動模式會先用 beanfun", "自动模式会先用 beanfun", "Automatic asks beanfun first")}
                    </span>
                  </div>
                )}
              </div>

              <div className={styles.prog}>
                <div className={styles.prog__top}>
                  <span>{headline}</span>
                  <span className={styles.prog__pct}>{phase === "idle" ? 0 : pct}%</span>
                </div>
                <div className={styles.prog__track}>
                  <div
                    className={clsx(styles.prog__fill, styles[`prog__fill_${tone}`])}
                    style={{ width: `${phase === "idle" ? 0 : pct}%` }}
                  />
                </div>
                <span className={styles.prog__detail}>{detail}</span>
                <span className={styles.prog__file}>{inFlight ? inFlight.path : ""}</span>
              </div>

              <div className={styles.chips}>
                <button
                  className={clsx(styles.chip, filter === "all" && styles.chip_on)}
                  onClick={() => setFilter("all")}
                >
                  {t("全部", "全部", "All")} <b>1263</b>
                </button>
                <button
                  className={clsx(styles.chip, filter === "issues" && styles.chip_on)}
                  onClick={() => setFilter("issues")}
                >
                  {t("需要處理", "需要处理", "To fix")}{" "}
                  <b className={scanned ? styles.warn : styles.dim}>
                    {scanned ? issuesLeft.length : "—"}
                  </b>
                </button>
                <button
                  className={clsx(styles.chip, filter === "ok" && styles.chip_on)}
                  onClick={() => setFilter("ok")}
                >
                  {t("正常", "正常", "Fine")}{" "}
                  <b className={scanned ? styles.ok : styles.dim}>
                    {scanned ? 1261 + doneCount : "—"}
                  </b>
                </button>
                <span className={styles.chip}>
                  {t("額外檔案", "额外文件", "Extra")} <b>3</b>
                </span>
              </div>

              <div className={styles.list}>
                {rows.length === 0 && (
                  <p className={styles.empty}>{t("沒有符合的項目。", "没有符合的项目。", "Nothing here.")}</p>
                )}
                {rows.map((row) => {
                  if (row.kind === "fine") {
                    return (
                      <div key={row.p} className={styles.row}>
                        <span className={clsx(styles.row__tick, scanned ? styles.ok : styles.dim)}>
                          {scanned ? "✓" : "·"}
                        </span>
                        <span className={clsx(styles.row__path, styles.dim)}>{row.p}</span>
                        <span className={styles.dim}>
                          {scanned ? t("正常", "正常", "fine") : t("未檢查", "未检查", "not checked")}
                        </span>
                        <span className={styles.row__size}>—</span>
                      </div>
                    );
                  }
                  const f = row.f;
                  const got = progressOf(f);
                  const done = repairing && got >= 1;
                  const moving = phase === "downloading" && got > 0 && got < 1;
                  return (
                    <div key={f.path} className={styles.row}>
                      {done ? (
                        <span className={clsx(styles.row__tick, styles.ok)}>✓</span>
                      ) : moving ? (
                        <span className={clsx(styles.row__tick, styles.accent, styles.pulse)}>↓</span>
                      ) : (
                        <span className={styles.row__box}>✓</span>
                      )}
                      <span className={styles.row__path}>{f.path}</span>
                      <span
                        className={done ? styles.ok : moving ? styles.accent : styles.warn}
                        title={t(f.why.tw, f.why.cn, f.why.en)}
                      >
                        {done
                          ? t("已修復", "已修复", "repaired")
                          : moving
                            ? `${t("下載中", "下载中", "downloading")} ${Math.round(got * 100)}%`
                            : t(f.label.tw, f.label.cn, f.label.en)}
                      </span>
                      <span className={styles.row__size}>{f.size}</span>
                      {moving && <span className={styles.row__bar} style={{ width: `${got * 100}%` }} />}
                    </div>
                  );
                })}
              </div>
            </div>
          ) : (
            <div className={styles.body}>
              <div className={styles.card}>
                <div className={styles.card__head}>
                  <strong>
                    {t(
                      "以種子檔安裝或更新",
                      "以种子文件安装或更新",
                      "Install or update from a torrent",
                    )}
                  </strong>
                  <span className={styles.card__btns}>
                    <span className={styles.btnGhost}>
                      {t("手動：儲存種子檔", "手动：保存种子文件", "Manual: save the torrent")}
                    </span>
                    <span className={styles.btnMain}>
                      {t("自動下載安裝", "自动下载安装", "Download and install")}
                    </span>
                  </span>
                </div>
                <p>
                  {t(
                    "與 beanfun 遊戲管理員相同的來源：整個遊戲資料夾由官方 CDN 取得，不經安裝檔。",
                    "与 beanfun 游戏管理器相同的来源：整个游戏文件夹从官方 CDN 获取，不经安装包。",
                    "The same source the beanfun manager uses: the whole folder from the official CDN, no installer.",
                  )}
                </p>
              </div>
              <div className={styles.card}>
                <div className={styles.card__head}>
                  <strong>
                    {t("遊戲橘子遊戲管理器", "游戏橘子游戏管理器", "Gamania Games Manager")}
                    <span className={styles.badge}>{t("不建議", "不建议", "not advised")}</span>
                  </strong>
                  <span className={styles.card__btns}>
                    <span className={styles.btnGhost}>
                      {t("複製連結", "复制链接", "Copy link")}
                    </span>
                    <span className={styles.btnMain}>{t("下載", "下载", "Download")}</span>
                  </span>
                </div>
                <p>
                  {t(
                    "它會把 gamaniagames:// 的登錄機碼設回自己，MapleLink 的網頁啟動接管會失效。",
                    "它会把 gamaniagames:// 的注册表项设回自己，MapleLink 的网页启动接管会失效。",
                    "It points the gamaniagames:// registry key back at itself, disabling MapleLink's web-launch interception.",
                  )}
                </p>
              </div>
              <div className={styles.card}>
                <div className={styles.card__head}>
                  <strong>{t("官方清單", "官方清单", "The official manifest")}</strong>
                  <span className={styles.card__btns}>
                    <span className={styles.btnGhost}>
                      {t("開啟清單", "打开清单", "Open the manifest")}
                    </span>
                  </span>
                </div>
                <p>
                  {t(
                    "比對用的清單是公開的，可以自己核對每個檔案的路徑、大小與 SHA-256。",
                    "比对用的清单是公开的，可以自己核对每个文件的路径、大小与 SHA-256。",
                    "The manifest behind the comparison is public: check each path, size and SHA-256 yourself.",
                  )}
                </p>
              </div>
            </div>
          )}

          <div className={styles.foot}>
            {tab === "download" ? (
              <span className={styles.foot__note}>
                {t(
                  "本頁只提供官方連結與種子檔，不會自行下載。",
                  "本页只提供官方链接与种子文件，不会自行下载。",
                  "This tab only hands over official links and a torrent; it downloads nothing by itself.",
                )}
              </span>
            ) : (
              <>
                <span className={styles.foot__group}>
                  <span className={styles.foot__label}>{t("檢查方式", "检查方式", "CHECK")}</span>
                  <span className={styles.foot__opt}>
                    <i className={styles.radio_on} /> {t("快速", "快速", "Quick")}
                  </span>
                  <span className={styles.foot__opt}>
                    <i className={styles.radio} /> {t("完整", "完整", "Full")}
                  </span>
                </span>
                <span className={styles.foot__div} />
                <span className={styles.foot__group}>
                  <span className={styles.foot__label}>{t("選項", "选项", "OPTIONS")}</span>
                  <span className={styles.foot__opt}>
                    <i className={styles.check} /> {t("自動檢查", "自动检查", "Auto check")}
                  </span>
                  <span className={styles.foot__opt}>
                    {t("同時下載", "同时下载", "At once")} <span className={styles.select}>6 ▾</span>
                  </span>
                </span>
                <span className={styles.foot__spacer} />

                {busy ? (
                  <>
                    <button className={styles.btnGhost} onClick={() => setPaused(!paused)}>
                      {paused ? t("繼續", "继续", "Resume") : t("暫停", "暂停", "Pause")}
                    </button>
                    <button className={styles.btnGhost} onClick={reset}>
                      {t("取消", "取消", "Cancel")}
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      className={phase === "scanned" ? styles.btnGhost : styles.btnMain}
                      onClick={() => start("scanning")}
                    >
                      {phase === "idle"
                        ? t("開始檢查", "开始检查", "Check files")
                        : t("重新檢查", "重新检查", "Check again")}
                    </button>
                    {phase === "scanned" && (
                      <button className={styles.btnMain} onClick={() => start("downloading")}>
                        {t("下載並修復", "下载并修复", "Download and repair")}
                      </button>
                    )}
                  </>
                )}
              </>
            )}
          </div>
        </div>
      </UiFrame>

      <UiCoach
        done={phase === "done"}
        action={
          phase === "done" ? (
            <button className={styles.reset} onClick={reset}>
              {t("再試一次", "再试一次", "Try again")}
            </button>
          ) : undefined
        }
      >
        {coach}
      </UiCoach>
    </div>
  );
}
