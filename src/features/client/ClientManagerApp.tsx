import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { errorMessage } from "../../lib/errors";
// The same hooks the main window uses, so both react to theme, language and
// accent identically instead of drifting apart.
import { useInitialConfigSync, useThemeEffect } from "../../lib/hooks/use-app-chrome";
import { VerifyPanel, type Filter } from "./VerifyPanel";
import { DownloadPanel } from "./DownloadPanel";
import type {
  ClientDownloadFileDto,
  ClientDownloadReportDto,
  ClientFileState,
  ClientLocalVersionDto,
  ClientManifestAttemptDto,
  ClientManifestDto,
  ClientNetworkStatusDto,
  ClientProgressDto,
  ClientScanReportDto,
} from "../../lib/types";

type Phase = "loading" | "idle" | "scanning" | "scanned" | "downloading" | "done";

/** Answers to the one-time "check automatically?" prompt. */
const AUTO_CHECK_KEY = "client.auto_check";
const AUTO_CHECK_ASKED_KEY = "client.auto_check_asked";
/** How many files to fetch at once, remembered between runs. */
const CONCURRENCY_KEY = "client.concurrency";
const CONCURRENCY_DEFAULT = 6;
/**
 * What the setting offers.
 *
 * Six connections do not add bandwidth — they share the line — but on a long
 * path they use it better, because one stream spends its time waiting out the
 * round trip. The low end is for the opposite case: a slow or shaped line,
 * where splitting it six ways only means six files crawling instead of one
 * finishing.
 */
const CONCURRENCY_CHOICES = [1, 2, 4, 6, 8];
type Tone = "busy" | "ok" | "warn";
type Tab = "verify" | "download";

/** "3 分 20 秒" is noise in a progress line; "3:20" is not. */
function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(rest)}` : `${m}:${pad(rest)}`;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = n / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

/** RFC 3339 → `2026/09/11 16:40`, for "this copy is from ...". */
function formatCachedAt(raw: string): string {
  const at = new Date(raw);
  if (Number.isNaN(at.getTime())) return raw;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${at.getFullYear()}/${pad(at.getMonth() + 1)}/${pad(at.getDate())} ${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

/** `Thu, 10 Sep 2026 08:22:56 GMT` → `2026/09/10`. */
function formatHttpDate(raw: string | null): string {
  if (!raw) return "";
  const at = new Date(raw);
  if (Number.isNaN(at.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${at.getFullYear()}/${pad(at.getMonth() + 1)}/${pad(at.getDate())}`;
}

function Titlebar() {
  const { t } = useTranslation();
  const appWindow = getCurrentWindow();

  function handleDragStart(e: React.MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    appWindow.startDragging();
  }

  return (
    <div onMouseDown={handleDragStart} className="flex h-[34px] shrink-0 items-center">
      <span className="flex-1 px-4 text-[11px] font-bold tracking-[2px] text-text-faint">
        {t("client.title")}
      </span>
      <button
        onClick={() => appWindow.minimize()}
        aria-label={t("shared.titlebar.minimize")}
        className="flex h-[34px] w-[34px] items-center justify-center text-[14px] text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
      >
        −
      </button>
      <button
        onClick={() => appWindow.close()}
        aria-label={t("shared.titlebar.close")}
        className="flex h-[34px] w-[34px] items-center justify-center text-[16px] text-text-dim transition-all hover:bg-[var(--danger)] hover:text-white"
      >
        ×
      </button>
    </div>
  );
}

function Progress({
  fraction,
  headline,
  detail,
  tone,
  file,
}: {
  fraction: number;
  headline: string;
  detail: string;
  tone: Tone;
  /** The file being read or written right now, on a line of its own. */
  file?: string;
}) {
  const pct = Math.min(100, Math.max(0, fraction * 100));
  const fill = {
    busy: "from-accent to-[var(--accent-dark)]",
    ok: "from-green-500 to-green-600",
    warn: "from-yellow-500 to-yellow-600",
  }[tone];

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="truncate text-[12px] font-semibold text-[var(--text)]">{headline}</span>
        <span className="shrink-0 font-mono text-[11px] text-text-dim">{pct.toFixed(0)}%</span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-[var(--surface-hover)]">
        <div
          className={`h-full rounded-full bg-gradient-to-r ${fill} transition-[width] duration-200`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="h-4 truncate font-mono text-[10px] text-text-faint">{detail}</span>
      {/* Its own line: a path is 60 characters and used to push the numbers
          it was appended to off the end of the row. The name is what is being
          watched, so the folder in front of it is dimmed rather than cut. */}
      {file && (
        <span title={file} className="flex h-4 gap-0 truncate font-mono text-[10px]">
          <span className="truncate text-text-faint">
            {file.slice(0, file.lastIndexOf("/") + 1)}
          </span>
          <span className="shrink-0 font-semibold text-text-dim">
            {file.slice(file.lastIndexOf("/") + 1)}
          </span>
        </span>
      )}
    </div>
  );
}

/** `HK` -> 香港, in the language the window is showing. */
function regionName(code: string, language: string): string | undefined {
  try {
    return new Intl.DisplayNames([language], { type: "region" }).of(code);
  } catch {
    return undefined;
  }
}

/** Latency that reads as good, fine, or worth a look. */
function latencyTone(ms: number): string {
  if (ms < 60) return "text-green-500";
  if (ms < 150) return "text-[var(--text)]";
  return "text-yellow-500";
}

/**
 * One fact about the install or the connection: a caption, the value, and a
 * line under it. Four of these replace a header that was a name, one line of
 * dates and a lot of nothing.
 */
function StatTile({
  caption,
  value,
  valueClass = "text-[var(--text)]",
  sub,
  title,
}: {
  caption: string;
  value: React.ReactNode;
  valueClass?: string;
  sub?: React.ReactNode;
  title?: string;
}) {
  return (
    <div
      title={title}
      className="flex min-w-0 flex-col gap-0.5 rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)] px-3.5 py-2.5"
    >
      <span className="truncate text-[10px] font-semibold tracking-[1px] text-text-faint">
        {caption}
      </span>
      <span className={`truncate text-[13px] leading-tight font-bold ${valueClass}`}>{value}</span>
      <span className="flex h-4 min-w-0 items-center truncate text-[10px] text-text-dim">
        {sub}
      </span>
    </div>
  );
}

export function ClientManagerApp() {
  const { t, language } = useTranslation();
  useInitialConfigSync();
  useThemeEffect();

  const [tab, setTab] = useState<Tab>("verify");
  const [filter, setFilter] = useState<Filter>("all");
  const [phase, setPhase] = useState<Phase>("loading");
  const [manifest, setManifest] = useState<ClientManifestDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dir, setDir] = useState("");
  const [mode, setMode] = useState<"quick" | "full">("quick");
  const [progress, setProgress] = useState<ClientProgressDto | null>(null);
  const [report, setReport] = useState<ClientScanReportDto | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [outcome, setOutcome] = useState<ClientDownloadReportDto | null>(null);
  const [freeSpace, setFreeSpace] = useState<number | null>(null);
  const [local, setLocal] = useState<ClientLocalVersionDto | null | undefined>(undefined);
  const [paused, setPaused] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  // What the list load is trying right now, for the line that says so.
  const [manifestAttempt, setManifestAttempt] = useState<ClientManifestAttemptDto | null>(null);
  const [autoCheck, setAutoCheck] = useState(false);
  // Extra files picked for the Recycle Bin, the confirmation, and its outcome.
  const [extraSelected, setExtraSelected] = useState<Set<string>>(new Set());
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [extraBusy, setExtraBusy] = useState(false);
  const [extraNotice, setExtraNotice] = useState<string | null>(null);
  // `undefined` while the first measurement runs, so the tile says so rather
  // than showing a blank that reads as "no network".
  const [net, setNet] = useState<ClientNetworkStatusDto | undefined>(undefined);
  const [netTesting, setNetTesting] = useState(false);
  const [concurrency, setConcurrency] = useState(CONCURRENCY_DEFAULT);
  // `undefined` until the stored answer is read, so the prompt cannot flash.
  const [askAutoCheck, setAskAutoCheck] = useState<boolean | undefined>(undefined);
  // Set once the stored answer says the check should run without being asked.
  const autoOnOpen = useRef(false);
  // Bytes per second, measured between progress events rather than assumed.
  const [rate, setRate] = useState(0);
  const rateSample = useRef<{ at: number; bytes: number } | null>(null);
  /**
   * What each file is doing right now, for the list.
   *
   * Events arrive twice per file — 2,500 of them in a full install — so they
   * are collected in a ref and handed to React on the progress tick instead.
   * Rebuilding a 1,263-row list per event is what would make the window crawl
   * exactly while it is busiest.
   */
  const liveFiles = useRef(new Map<string, ClientFileState>());
  const liveDirty = useRef(false);
  /** How far each in-flight file has got, straight from the progress tick. */
  const [inFlight, setInFlight] = useState<ReadonlyMap<string, number>>(new Map());
  const [liveStates, setLiveStates] = useState<ReadonlyMap<string, ClientFileState>>(new Map());

  useEffect(() => {
    document.title = t("client.title");
  }, [t]);

  useEffect(() => {
    let live = true;
    (async () => {
      const [on, asked, atOnce] = await Promise.all([
        commands.prefGet(AUTO_CHECK_KEY).catch(() => null),
        commands.prefGet(AUTO_CHECK_ASKED_KEY).catch(() => null),
        commands.prefGet(CONCURRENCY_KEY).catch(() => null),
      ]);
      if (!live) return;
      const saved = Number(atOnce);
      if (CONCURRENCY_CHOICES.includes(saved)) setConcurrency(saved);
      setAutoCheck(on === "on");
      setAskAutoCheck(asked !== "1");
      autoOnOpen.current = on === "on" && asked === "1";
    })();
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    function track(p: ClientProgressDto) {
      setProgress(p);
      setInFlight(
        new Map(p.active.map((f) => [f.path, f.total > 0 ? f.done / f.total : 0] as const)),
      );
      if (liveDirty.current) {
        liveDirty.current = false;
        const next = new Map(liveFiles.current);
        setLiveStates(next);
        // A file that has been written is no longer work to pick: dropping it
        // here is what makes pressing repair again fetch only what is left.
        setSelected((prev) => {
          const keep = new Set(prev);
          let changed = false;
          for (const [path, state] of next) {
            if (state === "done" && keep.delete(path)) changed = true;
          }
          return changed ? keep : prev;
        });
      }
      const now = performance.now();
      const last = rateSample.current;
      // Average over at least half a second, or the number jumps around.
      if (last && now - last.at >= 500) {
        const perSecond = ((p.bytesDone - last.bytes) * 1000) / (now - last.at);
        setRate(Math.max(0, perSecond));
        rateSample.current = { at: now, bytes: p.bytesDone };
      } else if (!last) {
        rateSample.current = { at: now, bytes: p.bytesDone };
      }
    }
    const scan = listen<ClientProgressDto>("client-scan-progress", (e) => track(e.payload));
    const down = listen<ClientProgressDto>("client-download-progress", (e) => track(e.payload));
    const attempt = listen<ClientManifestAttemptDto>("client-manifest-attempt", (e) =>
      setManifestAttempt(e.payload),
    );
    const file = listen<ClientDownloadFileDto>("client-download-file", (e) => {
      liveFiles.current.set(e.payload.path, e.payload.state);
      liveDirty.current = true;
    });
    return () => {
      scan.then((un) => un());
      down.then((un) => un());
      file.then((un) => un());
      attempt.then((un) => un());
    };
  }, []);

  useEffect(() => {
    if (!dir) return;
    commands
      .clientFreeSpace(dir)
      .then(setFreeSpace)
      .catch(() => setFreeSpace(null));
  }, [dir, report]);

  // How this machine reaches the CDN. It needs the manifest for the address,
  // and after that it is measured again only when asked: it costs requests.
  const manifestVersion = manifest?.version;
  useEffect(() => {
    if (!manifestVersion) return;
    let live = true;
    (async () => {
      const status = await commands.clientNetworkStatus().catch(() => null);
      if (live && status) setNet(status);
    })();
    return () => {
      live = false;
    };
  }, [manifestVersion]);

  const retestNetwork = useCallback(async () => {
    setNetTesting(true);
    const status = await commands.clientNetworkStatus().catch(() => null);
    if (status) setNet(status);
    setNetTesting(false);
  }, []);

  // What the install says about itself, refreshed after a repair.
  useEffect(() => {
    if (!dir || !manifest) return;
    let live = true;
    commands
      .clientLocalVersion(dir)
      .then((v) => live && setLocal(v))
      .catch(() => live && setLocal(null));
    return () => {
      live = false;
    };
  }, [dir, manifest, outcome]);

  const browse = useCallback(async () => {
    const picked = await commands.clientPickFolder(dir || null).catch(() => null);
    if (picked) {
      setDir(picked);
      setReport(null);
      setOutcome(null);
      setProgress(null);
      setPhase("idle");
    }
  }, [dir]);

  const scanInto = useCallback(async (target: string, how: "quick" | "full") => {
    setError(null);
    setPaused(false);
    setRate(0);
    rateSample.current = null;
    setOutcome(null);
    setReport(null);
    setProgress(null);
    // The last repair's rows describe files as they were before this check;
    // leaving them would have the new result read through the old one.
    liveFiles.current = new Map();
    liveDirty.current = false;
    setLiveStates(new Map());
    setInFlight(new Map());
    // Picks belong to the list they were made from.
    setExtraSelected(new Set());
    setExtraNotice(null);
    setPhase("scanning");
    try {
      const r = await commands.clientScan(target, how);
      setReport(r);
      setSelected(new Set(r.files.filter((f) => f.kind !== null).map((f) => f.path)));
      setPhase("scanned");
      return r;
    } catch (e) {
      setError(errorMessage(e));
      setPhase("idle");
      return null;
    }
  }, []);

  const runScan = useCallback(() => scanInto(dir, mode), [scanInto, dir, mode]);

  /**
   * Ask beanfun for the list again.
   *
   * Loading already prefers the network and only falls back to the cached
   * copy, so this is the manual retry for when that fallback happened: the
   * connection is back, or the player wants to be sure the list is today's
   * before trusting a comparison. It is also the way out of a start-up that
   * failed with no cache to fall back on.
   */
  const loadedVersion = manifest?.version ?? null;
  const reloadManifest = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    setManifestAttempt(null);
    try {
      const m = await commands.clientLoadManifest();
      setManifest(m);
      // A result on screen describes the list it was compared against, so a
      // different version makes it wrong rather than merely old.
      if (loadedVersion !== null && loadedVersion !== m.version) {
        setReport(null);
        setSelected(new Set());
        setOutcome(null);
      }
      if (m.cachedAt) {
        // Still the stored copy: beanfun did not answer this time either.
        setError(t("client.manifest_still_offline"));
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setRefreshing(false);
      setManifestAttempt(null);
    }
  }, [t, loadedVersion]);

  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const [m, folder] = await Promise.all([
          commands.clientLoadManifest(),
          commands.clientDefaultFolder().catch(() => null),
        ]);
        if (!live) return;
        setManifestAttempt(null);
        setManifest(m);
        if (folder) setDir(folder);
        setPhase("idle");
        // Already answered, and the answer was yes: get on with it.
        if (folder && autoOnOpen.current) {
          autoOnOpen.current = false;
          await scanInto(folder, "quick");
        }
      } catch (e) {
        if (!live) return;
        setError(errorMessage(e));
        setPhase("idle");
      }
    })();
    return () => {
      live = false;
    };
    // `scanInto` has no dependencies of its own, so this still runs once.
  }, [scanInto]);

  const downloadInto = useCallback(
    async (target: string, paths: string[]) => {
      setError(null);
      setPaused(false);
      setRate(0);
      rateSample.current = null;
      setProgress(null);
      liveFiles.current = new Map();
      liveDirty.current = false;
      setLiveStates(new Map());
      setInFlight(new Map());
      // The whole list is 1,263 rows and the work is somewhere inside it, so a
      // repair starts by showing what it is repairing — a list that empties as
      // files land, with whatever is in flight at the top of what remains.
      setFilter("issues");
      setPhase("downloading");
      try {
        const r = await commands.clientDownload(target, paths, concurrency);
        setOutcome(r);
        setPhase("done");
      } catch (e) {
        setError(errorMessage(e));
        setPhase("scanned");
      } finally {
        // Whatever was mid-flight when the run ended is not still downloading —
        // a cancelled file has no event of its own, so it is dropped here.
        for (const [path, state] of liveFiles.current) {
          if (state === "downloading") liveFiles.current.delete(path);
        }
        liveDirty.current = false;
        setLiveStates(new Map(liveFiles.current));
        setInFlight(new Map());
      }
    },
    [concurrency],
  );

  /**
   * Move the picked extra files to the Recycle Bin, once confirmed.
   *
   * The list is updated from what the backend says it removed rather than from
   * what was asked: a file the game still holds open, or one refused by the
   * checks, stays listed and stays picked.
   */
  const removeExtra = useCallback(async () => {
    setConfirmRemove(false);
    setExtraBusy(true);
    setExtraNotice(null);
    setError(null);
    try {
      const r = await commands.clientRemoveExtra(dir, [...extraSelected]);
      const gone = new Set(r.removed);
      setReport((prev) =>
        prev ? { ...prev, extraFiles: prev.extraFiles.filter((p) => !gone.has(p)) } : prev,
      );
      setExtraSelected((prev) => new Set([...prev].filter((p) => !gone.has(p))));
      if (r.removed.length > 0) {
        setExtraNotice(
          t("client.extra_removed", {
            count: String(r.removed.length),
            size: formatBytes(r.bytes),
          }),
        );
      }
      const first = r.failures[0];
      if (first) {
        setError(
          t("client.extra_remove_failed", {
            count: String(r.failures.length),
            reason: `${first.path}: ${first.error}`,
          }),
        );
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setExtraBusy(false);
    }
  }, [dir, extraSelected, t]);

  const runDownload = useCallback(
    () => downloadInto(dir, [...selected]),
    [downloadInto, dir, selected],
  );

  /// Pick a folder, see what is missing, then fetch it — without making the
  /// player press the same two buttons in order.
  const startAutoInstall = useCallback(async () => {
    const picked = await commands.clientPickFolder(dir || null).catch(() => null);
    if (!picked) return;
    setDir(picked);
    setTab("verify");
    const r = await scanInto(picked, "quick");
    if (!r || r.cancelled || r.issueCount === 0) return;
    await downloadInto(
      picked,
      r.files.filter((f) => f.kind !== null).map((f) => f.path),
    );
  }, [dir, scanInto, downloadInto]);

  // Either answer is the answer: it is remembered, and the question is not
  // asked again. The toggle below the list is how it gets changed later.
  const answerAutoCheck = useCallback(
    (on: boolean) => {
      setAutoCheck(on);
      setAskAutoCheck(false);
      void commands.prefSet(AUTO_CHECK_KEY, on ? "on" : "off").catch(() => {});
      void commands.prefSet(AUTO_CHECK_ASKED_KEY, "1").catch(() => {});
      if (on && dir && phase === "idle") void scanInto(dir, "quick");
    },
    [dir, phase, scanInto],
  );

  const busy = phase === "scanning" || phase === "downloading";
  const selectedBytes = useMemo(
    () =>
      report
        ? report.files
            .filter((f) => f.kind !== null && selected.has(f.path))
            .reduce((sum, f) => sum + f.expectedSize, 0)
        : 0,
    [report, selected],
  );
  const notEnoughSpace = freeSpace !== null && selectedBytes > freeSpace;
  const hasWork = !!report && report.issueCount > 0 && !busy;

  const status: { headline: string; detail: string; tone: Tone; file?: string } = (() => {
    const left = progress ? progress.bytesTotal - progress.bytesDone : 0;
    const speed =
      rate > 0
        ? ` · ${t("client.rate", { rate: formatBytes(rate) })}${
            left > 0 ? ` · ${t("client.eta", { time: formatDuration(left / rate) })}` : ""
          }`
        : "";
    const counted = progress
      ? `${progress.done} / ${progress.total} · ${formatBytes(progress.bytesDone)} / ${formatBytes(progress.bytesTotal)}${busy ? speed : ""}`
      : "";
    // A reload after start-up says what it is trying as well, or switching the
    // source would look like nothing happened for up to half a minute.
    if (phase === "loading" || (refreshing && manifestAttempt)) {
      return {
        headline: manifestAttempt
          ? t("client.loading_manifest_via", {
              source: t(`client.manifest_source_${manifestAttempt.source}`),
              attempt: String(manifestAttempt.attempt),
              attempts: String(manifestAttempt.attempts),
            })
          : t("client.loading_manifest"),
        detail: "",
        tone: "busy",
      };
    }
    if (busy && paused) {
      return {
        headline: t("client.paused"),
        detail: counted,
        tone: "warn",
        file: progress?.current,
      };
    }
    if (phase === "scanning") {
      return {
        headline: t("client.scanning"),
        detail: counted,
        tone: "busy",
        file: progress?.current,
      };
    }
    if (phase === "downloading") {
      return {
        headline: t("client.downloading"),
        detail: counted,
        tone: "busy",
        file: progress?.current,
      };
    }
    if (outcome) {
      // A cancelled run stopped part-way; saying "done" would be a lie.
      if (outcome.cancelled) {
        return {
          headline: t("client.download_stopped", {
            written: String(outcome.written),
            total: String(outcome.requested),
          }),
          detail: t("client.rescan_hint"),
          tone: "warn",
        };
      }
      return outcome.failures.length === 0
        ? {
            headline: t("client.download_done", { count: String(outcome.written) }),
            detail: t("client.rescan_hint"),
            tone: "ok",
          }
        : {
            headline: t("client.download_failed_some", {
              written: String(outcome.written),
              failed: String(outcome.failures.length),
            }),
            detail: outcome.failures[0]?.error ?? "",
            tone: "warn",
          };
    }
    if (report?.cancelled) {
      return {
        headline: t("client.scan_cancelled_title"),
        detail: t("client.scan_cancelled_body", {
          done: String(report.files.length),
          total: String(report.totalFiles),
        }),
        tone: "warn",
      };
    }
    if (report) {
      return report.issueCount === 0
        ? { headline: t("client.all_good"), detail: counted, tone: "ok" }
        : {
            headline: t("client.needs_repair", { count: String(report.issueCount) }),
            detail: t("client.what_changed", {
              count: String(report.issueCount),
              size: formatBytes(report.bytesToFetch),
              version: manifest?.fullVersion ?? manifest?.version ?? "",
            }),
            tone: "warn",
          };
    }
    return { headline: t("client.ready"), detail: "", tone: "busy" };
  })();

  const fraction =
    phase === "scanned" || phase === "done"
      ? 1
      : progress && progress.bytesTotal > 0
        ? progress.bytesDone / progress.bytesTotal
        : 0;
  const exeDate = formatHttpDate(manifest?.exePatchDate ?? null);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-[var(--bg)] text-[var(--text)]">
      <Titlebar />

      {confirmRemove && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[4px]">
          <div className="w-[440px] rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)] p-5 shadow-[0_12px_40px_rgba(0,0,0,0.35)]">
            <h2 className="text-[14px] font-bold">
              {t("client.extra_confirm_title", { count: String(extraSelected.size) })}
            </h2>
            <p className="mt-2 text-[11px] leading-relaxed text-text-dim">
              {t("client.extra_confirm_body")}
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => setConfirmRemove(false)}
                className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
              >
                {t("client.cancel")}
              </button>
              <button
                onClick={() => void removeExtra()}
                className="rounded-lg bg-red-500 px-4 py-1.5 text-[11px] font-bold text-white transition-opacity hover:opacity-90"
              >
                {t("client.extra_confirm_yes")}
              </button>
            </div>
          </div>
        </div>
      )}

      {askAutoCheck && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[4px]">
          <div className="w-[420px] rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)] p-5 shadow-[0_12px_40px_rgba(0,0,0,0.35)]">
            <h2 className="text-[14px] font-bold">{t("client.auto_check_title")}</h2>
            <p className="mt-2 text-[11px] leading-relaxed text-text-dim">
              {t("client.auto_check_body")}
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => answerAutoCheck(false)}
                className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
              >
                {t("client.auto_check_no")}
              </button>
              <button
                onClick={() => answerAutoCheck(true)}
                className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-1.5 text-[11px] font-bold text-[var(--on-accent)] transition-opacity hover:opacity-90"
              >
                {t("client.auto_check_yes")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Four tiles and nothing above them. The game and its version used to
          sit over these as a title with a line of dates, and once the tiles
          existed that only said the same things twice. */}
      <div className="grid shrink-0 grid-cols-2 gap-2.5 px-6 pt-1 pb-4 min-[860px]:grid-cols-4">
        <StatTile
          caption={manifest?.productName ?? t("client.official_version")}
          value={manifest?.fullVersion ?? manifest?.version ?? "…"}
          sub={[
            manifest?.publishDate && t("client.published", { date: manifest.publishDate }),
            exeDate && t("client.exe_dated", { date: exeDate }),
          ]
            .filter(Boolean)
            .join(" · ")}
        />
        <StatTile
          caption={t("client.stat_local")}
          value={
            local === undefined
              ? "—"
              : local === null
                ? t("client.local_none")
                : local.matchesOfficial
                  ? t("client.up_to_date")
                  : t("client.update_available")
          }
          valueClass={
            local == null
              ? "text-text-faint"
              : local.matchesOfficial
                ? "text-green-500"
                : "text-yellow-500"
          }
          sub={
            // The version marker says which build this is; the file check says
            // whether the files agree with it. Two different questions.
            local && !local.matchesOfficial
              ? t("client.local_candidates", {
                  candidates:
                    local.candidates.map((v) => `V${v}`).join(" / ") || String(local.marker),
                })
              : phase === "scanning"
                ? t("client.scanning")
                : report && !report.cancelled
                  ? t("client.local_files_ok", {
                      ok: String(report.files.length - report.issueCount),
                      total: String(report.totalFiles),
                    })
                  : t("client.local_not_checked")
          }
        />
        <StatTile
          caption={
            /^[A-Za-z]:/.test(dir)
              ? `${t("client.stat_disk")} · ${dir.slice(0, 2).toUpperCase()}`
              : t("client.stat_disk")
          }
          value={freeSpace === null ? "—" : t("client.disk_free", { size: formatBytes(freeSpace) })}
          valueClass={notEnoughSpace ? "text-red-400" : "text-[var(--text)]"}
          sub={
            report && report.issueCount > 0
              ? t("client.disk_need_repair", { size: formatBytes(selectedBytes) })
              : manifest
                ? t("client.disk_need_full", { size: formatBytes(manifest.totalBytes) })
                : ""
          }
        />
        {/* Where the downloads come out and how fast the CDN answers along that
            route: one question, so one tile. */}
        <StatTile
          caption={t("client.stat_network")}
          title={net?.error ?? t("client.net_hint")}
          value={
            net === undefined || netTesting ? (
              t("client.net_testing")
            ) : (
              <>
                {net.country
                  ? (regionName(net.country, language) ?? net.country)
                  : t("client.net_region_unknown")}
                <span
                  className={net.latencyMs !== null ? latencyTone(net.latencyMs) : "text-red-400"}
                >
                  {" · "}
                  {net.latencyMs !== null ? `${net.latencyMs} ms` : t("client.net_unreachable")}
                </span>
              </>
            )
          }
          valueClass={net === undefined || netTesting ? "text-text-faint" : "text-[var(--text)]"}
          sub={
            <>
              <span className="min-w-0 truncate">
                {net === undefined
                  ? ""
                  : net.proxy
                    ? t("client.net_via_proxy", { proxy: net.proxy })
                    : net.pac
                      ? t("client.net_pac")
                      : t("client.net_no_proxy")}
              </span>
              <button
                onClick={() => void retestNetwork()}
                disabled={net === undefined || netTesting}
                className="ml-2 shrink-0 font-semibold text-text-dim underline decoration-dotted underline-offset-2 hover:text-accent disabled:no-underline disabled:opacity-50"
              >
                {t("client.net_retest")}
              </button>
            </>
          }
        />
      </div>

      {/* Tabs. */}
      <div className="flex shrink-0 items-center gap-1 border-b border-[var(--tb-border)] px-6">
        {(["verify", "download"] as const).map((id) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`-mb-px border-b-2 px-3 py-2 text-[12px] font-semibold transition-colors ${
              tab === id
                ? "border-accent text-[var(--text)]"
                : "border-transparent text-text-dim hover:text-[var(--text)]"
            }`}
          >
            {t(`client.tab_${id}`)}
          </button>
        ))}
      </div>

      {tab === "verify" ? (
        <>
          <div className="shrink-0 px-6 pt-3">
            <div className="flex items-center gap-2">
              <span className="shrink-0 text-[11px] font-semibold text-text-dim">
                {t("client.folder_label")}
              </span>
              <span
                title={dir}
                className="min-w-0 flex-1 truncate rounded-lg border border-[var(--tb-border)] bg-[var(--surface)] px-3 py-1.5 font-mono text-[11px] text-text-dim"
              >
                {dir || t("client.folder_hint")}
              </span>
              <button
                onClick={browse}
                disabled={busy}
                className="shrink-0 rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent disabled:opacity-50"
              >
                {t("client.browse")}
              </button>
            </div>
            <div className="mt-1 flex items-baseline gap-2">
              <p className="min-w-0 flex-1 text-[10px] text-text-faint">
                {t("client.folder_note")}
              </p>
              {/* The list the scan compares against, on the tab that does the
                  comparing — not only on the download tab. */}
              {manifest?.manifestUrl && (
                <button
                  onClick={() => commands.openExternal(manifest.manifestUrl).catch(() => {})}
                  title={t("client.manifest_hint")}
                  className="shrink-0 text-[10px] font-semibold text-text-dim underline decoration-dotted underline-offset-2 hover:text-accent"
                >
                  {t("client.manifest_open")}
                </button>
              )}
            </div>
            <div className="mt-3">
              <Progress
                fraction={fraction}
                headline={status.headline}
                detail={status.detail}
                tone={status.tone}
                file={status.file}
              />
            </div>
            {report &&
              report.issueCount > 0 &&
              report.files.some((f) => f.kind && f.kind !== "missing") && (
                <p className="mt-1 text-[11px] leading-relaxed text-blue-400">
                  {t("client.mismatch_note")}
                </p>
              )}
            {manifest?.cachedAt && (
              <div className="mt-2 flex items-center gap-3 rounded-lg border border-[rgba(234,179,8,0.3)] bg-[rgba(234,179,8,0.06)] px-3 py-2">
                <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-yellow-500">
                  {t("client.offline_manifest", { date: formatCachedAt(manifest.cachedAt) })}
                </p>
                <button
                  onClick={() => void reloadManifest()}
                  disabled={refreshing || busy}
                  className="shrink-0 rounded-lg border border-[rgba(234,179,8,0.4)] px-2.5 py-1 text-[11px] font-semibold text-yellow-500 transition-colors hover:bg-[rgba(234,179,8,0.12)] disabled:opacity-50"
                >
                  {refreshing ? t("client.manifest_refreshing") : t("client.manifest_refresh")}
                </button>
              </div>
            )}
            {error && (
              <div className="mt-2 flex items-center gap-3 rounded-lg border border-[rgba(239,68,68,0.3)] bg-[rgba(239,68,68,0.06)] px-3 py-2">
                <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-red-400">{error}</p>
                {/* No manifest means nothing can be compared, so the retry has
                    to be here rather than only on the offline notice. */}
                {!manifest && (
                  <button
                    onClick={() => void reloadManifest()}
                    disabled={refreshing}
                    className="shrink-0 rounded-lg border border-[rgba(239,68,68,0.4)] px-2.5 py-1 text-[11px] font-semibold text-red-400 transition-colors hover:bg-[rgba(239,68,68,0.12)] disabled:opacity-50"
                  >
                    {refreshing ? t("client.manifest_refreshing") : t("client.manifest_refresh")}
                  </button>
                )}
              </div>
            )}
          </div>

          <div className="h-3 shrink-0" />
          <VerifyPanel
            report={report}
            selected={selected}
            setSelected={setSelected}
            outdated={local != null && !local.matchesOfficial}
            live={liveStates}
            inFlight={inFlight}
            filter={filter}
            setFilter={setFilter}
            extraSelected={extraSelected}
            setExtraSelected={setExtraSelected}
            onRemoveExtra={() => setConfirmRemove(true)}
            extraBusy={extraBusy || busy}
            extraNotice={extraNotice}
          />
        </>
      ) : (
        <DownloadPanel onAutoInstall={startAutoInstall} />
      )}

      {/* Action bar. */}
      <div className="flex shrink-0 items-center gap-4 border-t border-[var(--tb-border)] bg-[var(--tb-bg)] px-6 py-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          {tab === "verify" ? (
            <>
              {/* Two groups, captioned and divided: how to check, and how to
                  behave while doing it. The long explanations move to tooltips
                  so the bar stays one line. */}
              <div className="flex flex-wrap items-center gap-x-5 gap-y-1.5">
                <div className="flex items-center gap-2.5">
                  <span className="text-[10px] font-semibold tracking-[1px] text-text-faint">
                    {t("client.group_mode")}
                  </span>
                  <div className="flex items-center gap-3">
                    {(["quick", "full"] as const).map((m) => (
                      <label
                        key={m}
                        title={t(`client.mode_${m}_hint`)}
                        className="flex cursor-pointer items-center gap-1.5 text-[11px]"
                      >
                        <input
                          type="radio"
                          checked={mode === m}
                          onChange={() => setMode(m)}
                          disabled={busy}
                          className="accent-[var(--accent)]"
                        />
                        <span className="font-semibold">{t(`client.mode_${m}`)}</span>
                      </label>
                    ))}
                  </div>
                </div>

                <span className="h-4 w-px bg-[var(--tb-border)]" />

                <div className="flex items-center gap-2.5">
                  <span className="text-[10px] font-semibold tracking-[1px] text-text-faint">
                    {t("client.group_options")}
                  </span>
                  <div className="flex items-center gap-3">
                    <label
                      title={t("client.auto_check_toggle_hint")}
                      className="flex cursor-pointer items-center gap-1.5 text-[11px]"
                    >
                      <input
                        type="checkbox"
                        checked={autoCheck}
                        onChange={(e) => answerAutoCheck(e.target.checked)}
                        disabled={busy}
                        className="accent-[var(--accent)]"
                      />
                      <span className="font-semibold">{t("client.auto_check_toggle")}</span>
                    </label>
                    <label
                      title={t("client.concurrency_hint")}
                      className="flex cursor-pointer items-center gap-1.5 text-[11px]"
                    >
                      <span className="font-semibold">{t("client.concurrency")}</span>
                      <select
                        value={concurrency}
                        onChange={(e) => {
                          const next = Number(e.target.value);
                          setConcurrency(next);
                          void commands.prefSet(CONCURRENCY_KEY, String(next)).catch(() => {});
                        }}
                        disabled={busy}
                        className="rounded-md border border-[var(--tb-border)] bg-[var(--surface)] px-1.5 py-0.5 text-[11px] font-semibold text-[var(--text)] outline-none focus:border-accent disabled:opacity-50"
                      >
                        {CONCURRENCY_CHOICES.map((n) => (
                          <option key={n} value={n}>
                            {n}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                </div>
              </div>
              {hasWork && (
                <span
                  className={`text-[11px] ${notEnoughSpace ? "text-red-400" : "text-text-faint"}`}
                >
                  {t("client.selected", {
                    count: String(selected.size),
                    size: formatBytes(selectedBytes),
                  })}
                  {freeSpace !== null &&
                    ` · ${
                      notEnoughSpace
                        ? t("client.not_enough_space", { size: formatBytes(freeSpace) })
                        : t("client.free_space", { size: formatBytes(freeSpace) })
                    }`}
                </span>
              )}
            </>
          ) : (
            <span className="text-[11px] text-text-faint">{t("client.download_tab_hint")}</span>
          )}
        </div>

        {tab === "verify" &&
          (busy ? (
            <>
              <button
                onClick={() => {
                  const next = !paused;
                  setPaused(next);
                  commands.clientSetPaused(next).catch(() => {});
                }}
                className="shrink-0 rounded-xl border border-border px-5 py-2.5 text-[13px] font-bold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
              >
                {paused ? t("client.resume") : t("client.pause")}
              </button>
              <button
                onClick={() => commands.clientCancel()}
                className="shrink-0 rounded-xl border border-border px-6 py-2.5 text-[13px] font-bold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
              >
                {t("client.cancel")}
              </button>
            </>
          ) : (
            <>
              <button
                onClick={runScan}
                disabled={!dir || !manifest}
                className={`shrink-0 rounded-xl px-6 py-2.5 text-[13px] font-bold transition-all active:scale-95 disabled:opacity-40 ${
                  hasWork
                    ? "border border-border text-text-dim hover:bg-[var(--surface-hover)]"
                    : "bg-gradient-to-br from-accent to-[var(--accent-dark)] text-[var(--on-accent)] hover:opacity-90"
                }`}
              >
                {report ? t("client.rescan") : t("client.scan")}
              </button>
              {hasWork && (
                <button
                  onClick={runDownload}
                  disabled={selected.size === 0 || notEnoughSpace}
                  className="shrink-0 rounded-xl bg-gradient-to-br from-accent to-[var(--accent-dark)] px-7 py-2.5 text-[13px] font-bold text-[var(--on-accent)] transition-all hover:opacity-90 active:scale-95 disabled:opacity-40"
                >
                  {t("client.download")}
                </button>
              )}
            </>
          ))}
      </div>
    </div>
  );
}
