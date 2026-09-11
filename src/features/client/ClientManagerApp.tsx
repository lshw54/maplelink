import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
// The same hooks the main window uses, so both react to theme, language and
// accent identically instead of drifting apart.
import { useInitialConfigSync, useThemeEffect } from "../../lib/hooks/use-app-chrome";
import { VerifyPanel } from "./VerifyPanel";
import { DownloadPanel } from "./DownloadPanel";
import type {
  ClientDownloadReportDto,
  ClientLocalVersionDto,
  ClientManifestDto,
  ClientProgressDto,
  ClientScanReportDto,
} from "../../lib/types";

type Phase = "loading" | "idle" | "scanning" | "scanned" | "downloading" | "done";

/** Answers to the one-time "check automatically?" prompt. */
const AUTO_CHECK_KEY = "client.auto_check";
const AUTO_CHECK_ASKED_KEY = "client.auto_check_asked";
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
}: {
  fraction: number;
  headline: string;
  detail: string;
  tone: Tone;
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
    </div>
  );
}

export function ClientManagerApp() {
  const { t } = useTranslation();
  useInitialConfigSync();
  useThemeEffect();

  const [tab, setTab] = useState<Tab>("verify");
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
  const [direct, setDirect] = useState(true);
  const [autoCheck, setAutoCheck] = useState(false);
  // `undefined` until the stored answer is read, so the prompt cannot flash.
  const [askAutoCheck, setAskAutoCheck] = useState<boolean | undefined>(undefined);
  // Set once the stored answer says the check should run without being asked.
  const autoOnOpen = useRef(false);
  // Bytes per second, measured between progress events rather than assumed.
  const [rate, setRate] = useState(0);
  const rateSample = useRef<{ at: number; bytes: number } | null>(null);

  useEffect(() => {
    document.title = t("client.title");
  }, [t]);

  useEffect(() => {
    let live = true;
    (async () => {
      const [on, asked] = await Promise.all([
        commands.prefGet(AUTO_CHECK_KEY).catch(() => null),
        commands.prefGet(AUTO_CHECK_ASKED_KEY).catch(() => null),
      ]);
      if (!live) return;
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
    return () => {
      scan.then((un) => un());
      down.then((un) => un());
    };
  }, []);

  useEffect(() => {
    if (!dir) return;
    commands
      .clientFreeSpace(dir)
      .then(setFreeSpace)
      .catch(() => setFreeSpace(null));
  }, [dir, report]);

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
    setPhase("scanning");
    try {
      const r = await commands.clientScan(target, how);
      setReport(r);
      setSelected(new Set(r.files.filter((f) => f.kind !== null).map((f) => f.path)));
      setPhase("scanned");
      return r;
    } catch (e) {
      setError(String(e));
      setPhase("idle");
      return null;
    }
  }, []);

  const runScan = useCallback(() => scanInto(dir, mode), [scanInto, dir, mode]);

  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const [m, folder] = await Promise.all([
          commands.clientLoadManifest(),
          commands.clientDefaultFolder().catch(() => null),
        ]);
        if (!live) return;
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
        setError(String(e));
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
      setPhase("downloading");
      try {
        const r = await commands.clientDownload(target, paths, direct);
        setOutcome(r);
        setPhase("done");
      } catch (e) {
        setError(String(e));
        setPhase("scanned");
      }
    },
    [direct],
  );

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

  const answerAutoCheck = useCallback(
    (on: boolean, remember: boolean) => {
      setAutoCheck(on);
      setAskAutoCheck(false);
      void commands.prefSet(AUTO_CHECK_KEY, on ? "on" : "off").catch(() => {});
      if (remember) void commands.prefSet(AUTO_CHECK_ASKED_KEY, "1").catch(() => {});
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

  const status: { headline: string; detail: string; tone: Tone } = (() => {
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
    if (phase === "loading") {
      return { headline: t("client.loading_manifest"), detail: "", tone: "busy" };
    }
    if (busy && paused) {
      return {
        headline: t("client.paused"),
        detail: `${counted}${progress?.current ? ` · ${progress.current}` : ""}`,
        tone: "warn",
      };
    }
    if (phase === "scanning") {
      return {
        headline: t("client.scanning"),
        detail: `${counted}${progress?.current ? ` · ${progress.current}` : ""}`,
        tone: "busy",
      };
    }
    if (phase === "downloading") {
      return { headline: t("client.downloading"), detail: counted, tone: "busy" };
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

      {askAutoCheck && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[4px]">
          <div className="w-[420px] rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)] p-5 shadow-[0_12px_40px_rgba(0,0,0,0.35)]">
            <h2 className="text-[14px] font-bold">{t("client.auto_check_title")}</h2>
            <p className="mt-2 text-[11px] leading-relaxed text-text-dim">
              {t("client.auto_check_body")}
            </p>
            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => answerAutoCheck(false, true)}
                className="rounded-lg px-3 py-1.5 text-[11px] text-text-faint transition-colors hover:bg-[var(--surface-hover)]"
              >
                {t("client.auto_check_never")}
              </button>
              <button
                onClick={() => answerAutoCheck(false, false)}
                className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
              >
                {t("client.auto_check_no")}
              </button>
              <button
                onClick={() => answerAutoCheck(true, true)}
                className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-1.5 text-[11px] font-bold text-[var(--on-accent)] transition-opacity hover:opacity-90"
              >
                {t("client.auto_check_yes")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Which game, which version, how it stands. Two lines, no boxes: the
          numbers are context, not the point of the window. */}
      <div className="flex shrink-0 items-start justify-between gap-4 px-6 pb-4">
        <div className="min-w-0">
          <div className="flex items-baseline gap-2">
            <h1 className="text-[20px] leading-none font-bold tracking-tight">
              {manifest?.productName ?? "…"}
            </h1>
            <span className="text-[13px] leading-none font-semibold text-text-dim">
              {manifest?.fullVersion ?? manifest?.version ?? ""}
            </span>
          </div>
          {manifest && (
            <p className="mt-1.5 text-[11px] leading-none text-text-faint">
              {[
                manifest.publishDate && t("client.published", { date: manifest.publishDate }),
                exeDate && t("client.exe_dated", { date: exeDate }),
                t("client.stats", {
                  count: String(manifest.fileCount),
                  size: formatBytes(manifest.totalBytes),
                }),
              ]
                .filter(Boolean)
                .join("  ·  ")}
            </p>
          )}
        </div>

        {local !== undefined && (
          <span
            className={`flex shrink-0 items-center gap-1.5 text-[11px] font-semibold ${
              local?.matchesOfficial ? "text-green-500" : "text-yellow-500"
            }`}
          >
            <span className="text-[8px]">●</span>
            {local === null
              ? t("client.no_client_here")
              : local.matchesOfficial
                ? t("client.up_to_date")
                : t("client.version_ambiguous", {
                    marker: String(local.marker),
                    candidates: local.candidates.join(" / ") || "?",
                  })}
          </span>
        )}
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
            <p className="mt-1 text-[10px] text-text-faint">{t("client.folder_note")}</p>
            <div className="mt-3">
              <Progress
                fraction={fraction}
                headline={status.headline}
                detail={status.detail}
                tone={status.tone}
              />
            </div>
            {report &&
              report.issueCount > 0 &&
              report.files.some((f) => f.kind && f.kind !== "missing") && (
                <p className="mt-1 text-[11px] leading-relaxed text-blue-400">
                  {t("client.mismatch_note")}
                </p>
              )}
            {error && (
              <p className="mt-2 rounded-lg border border-[rgba(239,68,68,0.3)] bg-[rgba(239,68,68,0.06)] px-3 py-2 text-[11px] text-red-400">
                {error}
              </p>
            )}
          </div>

          <div className="h-3 shrink-0" />
          <VerifyPanel report={report} selected={selected} setSelected={setSelected} />
        </>
      ) : (
        <DownloadPanel onAutoInstall={startAutoInstall} />
      )}

      {/* Action bar. */}
      <div className="flex shrink-0 items-center gap-4 border-t border-[var(--tb-border)] bg-[var(--tb-bg)] px-6 py-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          {tab === "verify" ? (
            <>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
                <label className="flex cursor-pointer items-center gap-1.5 text-[11px]">
                  <input
                    type="checkbox"
                    checked={autoCheck}
                    onChange={(e) => answerAutoCheck(e.target.checked, true)}
                    disabled={busy}
                    className="accent-[var(--accent)]"
                  />
                  <span className="font-semibold">{t("client.auto_check_toggle")}</span>
                  <span className="text-text-faint">{t("client.auto_check_toggle_hint")}</span>
                </label>
                <label className="flex cursor-pointer items-center gap-1.5 text-[11px]">
                  <input
                    type="checkbox"
                    checked={direct}
                    onChange={(e) => setDirect(e.target.checked)}
                    disabled={busy}
                    className="accent-[var(--accent)]"
                  />
                  <span className="font-semibold">{t("client.direct")}</span>
                  <span className="text-text-faint">{t("client.direct_hint")}</span>
                </label>
                {(["quick", "full"] as const).map((m) => (
                  <label key={m} className="flex cursor-pointer items-center gap-1.5 text-[11px]">
                    <input
                      type="radio"
                      checked={mode === m}
                      onChange={() => setMode(m)}
                      disabled={busy}
                      className="accent-[var(--accent)]"
                    />
                    <span className="font-semibold">{t(`client.mode_${m}`)}</span>
                    <span className="text-text-faint">{t(`client.mode_${m}_hint`)}</span>
                  </label>
                ))}
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
