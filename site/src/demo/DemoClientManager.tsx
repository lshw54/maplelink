import React, { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiFrame from "./UiFrame";
import UiCoach from "./UiCoach";
import styles from "./DemoClientManager.module.css";

type Phase = "idle" | "scanning" | "scanned" | "downloading" | "done";

/** The two files this walk-through pretends are broken. */
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
  },
  {
    path: "Data/Mob/Mob002.wz",
    size: "268 MB",
    label: { tw: "缺失", cn: "缺失", en: "missing" },
    why: { tw: "本機找不到這個檔案。", cn: "本机找不到这个文件。", en: "Not on disk." },
  },
];

const FINE = [
  "BlackCipher/BlackCall64.aes",
  "Canvas.dll",
  "Data/Base/Base.wz",
  "Data/Character/Character.wz",
  "MapleStory.exe",
];

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
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  // Drive whichever bar is running; pausing simply stops advancing it.
  useEffect(() => {
    if (phase !== "scanning" && phase !== "downloading") return;
    timer.current = setInterval(() => {
      setPct((p) => {
        if (paused) return p;
        const next = p + (phase === "scanning" ? 9 : 6);
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

  function start(next: Phase) {
    setPct(0);
    setPaused(false);
    setPhase(next);
  }

  function reset() {
    setPhase("idle");
    setPct(0);
    setPaused(false);
    setTab("verify");
  }

  const busy = phase === "scanning" || phase === "downloading";
  // The results stay on screen once a check has run, including while the
  // repair downloads — only a fresh check clears them.
  const scanned = phase !== "idle" && phase !== "scanning";

  const coach = (() => {
    if (tab === "download")
      return t(
        "「自動下載安裝」選一個空資料夾就會整個抓下來；「手動」只給你種子檔，自己用 BT 工具下載。",
        "「自动下载安装」选一个空文件夹就会整个抓下来；「手动」只给你种子文件，自己用 BT 工具下载。",
        "Download and install fetches the whole client into an empty folder; Manual just hands you the torrent.",
      );
    if (phase === "idle")
      return t(
        "確認上面的遊戲路徑，然後按「開始檢查」。快速只比大小，數秒完成。",
        "确认上面的游戏路径，然后点「开始检查」。快速只比大小，数秒完成。",
        "Check the game path above, then press Check files. Quick compares sizes and takes seconds.",
      );
    if (phase === "scanning")
      return paused
        ? t(
            "已暫停，按「繼續」接返落去。",
            "已暂停，点「继续」接着来。",
            "Paused — press Resume to carry on.",
          )
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
        "只下載對不上的部分，下載完會先核對雜湊值才寫入。",
        "只下载对不上的部分，下载完会先校验哈希值才写入。",
        "Only the mismatched files are fetched, and each is hash-checked before it replaces anything.",
      );
    return t(
      "修好了。再檢查一次就會全部顯示正常。",
      "修好了。再检查一次就会全部显示正常。",
      "Repaired. Check again and everything reads as fine.",
    );
  })();

  const headline = (() => {
    if (phase === "idle") return t("準備就緒", "准备就绪", "Ready");
    if (phase === "scanning")
      return paused ? t("已暫停", "已暂停", "Paused") : t("檢查中", "检查中", "Checking");
    if (phase === "scanned")
      return t("2 個檔案需要處理", "2 个文件需要处理", "2 files need attention");
    if (phase === "downloading") return t("下載中", "下载中", "Downloading");
    return t("完成，已寫入 2 個檔案。", "完成，已写入 2 个文件。", "Done. 2 files written.");
  })();

  const detail = (() => {
    if (phase === "scanning") return `${Math.round((pct / 100) * 1263)} / 1263 · 67.7 GB`;
    if (phase === "downloading") return `${pct < 50 ? 1 : 2} / 2 · 74.1 MB/s`;
    if (phase === "scanned")
      return t("需要下載 680 MB。", "需要下载 680 MB。", "680 MB to download.");
    return "";
  })();

  const tone = phase === "done" ? "ok" : phase === "scanned" ? "warn" : "busy";

  return (
    <div className={clsx("demo", styles.demo)}>
      <UiFrame width={760} height={470}>
        <div className={clsx("ml", styles.win)}>
          <div className={styles.win__bar}>
            <span className={styles.win__name}>
              {t("遊戲客戶端管理", "游戏客户端管理", "Game client manager")}
            </span>
            <span className={styles.win__btns}>−&nbsp;&nbsp;×</span>
          </div>

          <div className={styles.head}>
            <h2>
              {t("新楓之谷", "新枫之谷", "MapleStory TW")} <em>V282.2</em>
            </h2>
            <span className={styles.head__pill}>
              ● {t("已是最新版本", "已是最新版本", "Up to date")}
            </span>
          </div>
          <p className={styles.head__meta}>
            {t("發布於", "发布于", "Published")} 2026/09/04 ·{" "}
            {t("主程式", "Executable", "Executable")} 2026/09/10 · 1263{" "}
            {t("個檔案", "个文件", "files")} · 67.7 GB
          </p>

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
              <div className={styles.path}>
                <span className={styles.path__label}>{t("遊戲路徑", "游戏路径", "Game path")}</span>
                <span className={styles.path__box}>C:\Games\MapleStory</span>
                <span className={styles.btnGhost}>{t("瀏覽", "浏览", "Browse")}</span>
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
              </div>

              <div className={styles.chips}>
                <span className={clsx(styles.chip, styles.chip_on)}>
                  {t("全部", "全部", "All")} <b>1263</b>
                </span>
                <span className={styles.chip}>
                  {t("需要處理", "需要处理", "To fix")}{" "}
                  <b className={scanned ? styles.warn : styles.dim}>
                    {phase === "done" ? 0 : scanned ? 2 : "—"}
                  </b>
                </span>
                <span className={styles.chip}>
                  {t("正常", "正常", "Fine")}{" "}
                  <b className={scanned ? styles.ok : styles.dim}>
                    {phase === "done" ? 1263 : scanned ? 1261 : "—"}
                  </b>
                </span>
              </div>

              <div className={styles.list}>
                {(phase === "scanned" || phase === "downloading") &&
                  BROKEN.map((f) => (
                    <div key={f.path} className={styles.row}>
                      <span className={styles.row__box}>✓</span>
                      <span className={styles.row__path}>{f.path}</span>
                      <span className={styles.warn} title={t(f.why.tw, f.why.cn, f.why.en)}>
                        {t(f.label.tw, f.label.cn, f.label.en)}
                      </span>
                      <span className={styles.row__size}>{f.size}</span>
                    </div>
                  ))}
                {FINE.map((p) => (
                  <div key={p} className={styles.row}>
                    <span className={clsx(styles.row__tick, scanned ? styles.ok : styles.dim)}>
                      {scanned ? "✓" : "·"}
                    </span>
                    <span className={clsx(styles.row__path, styles.dim)}>{p}</span>
                    <span className={styles.dim}>
                      {scanned ? t("正常", "正常", "fine") : t("未檢查", "未检查", "not checked")}
                    </span>
                    <span className={styles.row__size}>—</span>
                  </div>
                ))}
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
                    <i className={styles.check_on} /> {t("直連", "直连", "Direct")}
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
