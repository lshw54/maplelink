import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useConfig } from "../../lib/hooks/use-config";
import { useUiStore } from "../../lib/stores/ui-store";
import { applyAccent } from "../../lib/accent";
import type {
  ClientDownloadReportDto,
  ClientFileIssueDto,
  ClientManifestDto,
  ClientProgressDto,
  ClientScanReportDto,
} from "../../lib/types";

type Phase = "loading" | "idle" | "scanning" | "scanned" | "downloading" | "done";

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

/** Mirrors the main window's theme and language so the two look like one app. */
function useWindowChrome() {
  const { data: config } = useConfig();
  const setTheme = useUiStore((s) => s.setTheme);
  const setLanguage = useUiStore((s) => s.setLanguage);
  const theme = useUiStore((s) => s.theme);

  useEffect(() => {
    if (!config) return;
    setTheme(config.theme);
    setLanguage(config.language);
    applyAccent(config.accentColor ?? "");
  }, [config, setTheme, setLanguage]);

  useEffect(() => {
    const root = document.documentElement;
    const dark =
      theme === "dark" ||
      (theme === "system" && !window.matchMedia("(prefers-color-scheme: light)").matches);
    root.classList.toggle("light", !dark);
  }, [theme]);
}

function ProgressBar({ value, label }: { value: number; label: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="h-2 overflow-hidden rounded-full bg-[var(--surface-hover)]">
        <div
          className="h-full rounded-full bg-gradient-to-r from-accent to-[var(--accent-dark)] transition-[width] duration-200"
          style={{ width: `${Math.min(100, Math.max(0, value * 100))}%` }}
        />
      </div>
      <span className="truncate text-[11px] text-text-dim">{label}</span>
    </div>
  );
}

function IssueRow({
  issue,
  checked,
  onToggle,
}: {
  issue: ClientFileIssueDto;
  checked: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const kindKey = {
    missing: "client.issue_missing",
    sizeMismatch: "client.issue_size",
    hashMismatch: "client.issue_hash",
    unreadable: "client.issue_unreadable",
  }[issue.kind];

  return (
    <label className="flex cursor-pointer items-center gap-2.5 border-b border-[var(--tb-border)] px-3 py-2 last:border-b-0 hover:bg-[var(--surface-hover)]">
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        className="shrink-0 accent-[var(--accent)]"
      />
      <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-[var(--text)]">
        {issue.path}
      </span>
      <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold text-text-dim">
        {t(kindKey)}
      </span>
      <span className="w-20 shrink-0 text-right text-[11px] text-text-dim">
        {formatBytes(issue.expectedSize)}
      </span>
    </label>
  );
}

export function ClientManagerApp() {
  const { t } = useTranslation();
  useWindowChrome();

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
  const [showExtra, setShowExtra] = useState(false);

  useEffect(() => {
    document.title = t("client.title");
  }, [t]);

  // Load the manifest once, and pre-fill the folder from the configured game path.
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
      } catch (e) {
        if (!live) return;
        setError(String(e));
        setPhase("idle");
      }
    })();
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    const scan = listen<ClientProgressDto>("client-scan-progress", (e) => setProgress(e.payload));
    const down = listen<ClientProgressDto>("client-download-progress", (e) =>
      setProgress(e.payload),
    );
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

  const browse = useCallback(async () => {
    const picked = await commands.clientPickFolder(dir || null).catch(() => null);
    if (picked) {
      setDir(picked);
      setReport(null);
      setOutcome(null);
      setPhase("idle");
    }
  }, [dir]);

  const runScan = useCallback(async () => {
    setError(null);
    setOutcome(null);
    setReport(null);
    setProgress(null);
    setPhase("scanning");
    try {
      const r = await commands.clientScan(dir, mode);
      setReport(r);
      setSelected(new Set(r.issues.map((i) => i.path)));
      setPhase("scanned");
    } catch (e) {
      setError(String(e));
      setPhase("idle");
    }
  }, [dir, mode]);

  const runDownload = useCallback(async () => {
    setError(null);
    setProgress(null);
    setPhase("downloading");
    try {
      const r = await commands.clientDownload(dir, [...selected]);
      setOutcome(r);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("scanned");
    }
  }, [dir, selected]);

  const busy = phase === "scanning" || phase === "downloading";
  const selectedBytes = useMemo(() => {
    if (!report) return 0;
    return report.issues
      .filter((i) => selected.has(i.path))
      .reduce((sum, i) => sum + i.expectedSize, 0);
  }, [report, selected]);
  const notEnoughSpace = freeSpace !== null && selectedBytes > freeSpace;

  const fraction =
    progress && progress.bytesTotal > 0 ? progress.bytesDone / progress.bytesTotal : 0;
  const progressLabel = progress
    ? `${progress.done} / ${progress.total} · ${formatBytes(progress.bytesDone)} / ${formatBytes(progress.bytesTotal)}${progress.current ? ` · ${progress.current}` : ""}`
    : "";

  return (
    <div className="flex h-screen flex-col gap-3 overflow-hidden bg-[var(--bg)] p-4 text-[var(--text)]">
      <header className="flex shrink-0 flex-wrap items-baseline gap-x-3 gap-y-1">
        <h1 className="text-base font-bold">{t("client.title")}</h1>
        {manifest && (
          <span className="text-[12px] text-text-dim">
            {manifest.productName} · {manifest.version} ·{" "}
            {t("client.stats", {
              count: String(manifest.fileCount),
              size: formatBytes(manifest.totalBytes),
            })}
          </span>
        )}
      </header>

      <p className="shrink-0 rounded-[10px] border border-[rgba(234,179,8,0.3)] bg-[rgba(234,179,8,0.06)] px-3 py-2 text-[11px] leading-relaxed text-yellow-500">
        {t("client.warn_overwrite")}
      </p>

      <div className="flex shrink-0 flex-col gap-2 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] p-3">
        <div className="flex items-center gap-2">
          <span className="w-16 shrink-0 text-[11px] font-semibold text-text-dim">
            {t("client.folder_label")}
          </span>
          <input
            value={dir}
            onChange={(e) => setDir(e.target.value)}
            disabled={busy}
            spellCheck={false}
            placeholder={t("client.folder_hint")}
            className="min-w-0 flex-1 rounded-lg border border-border bg-[var(--surface)] px-2.5 py-1.5 font-mono text-[11px] text-[var(--text)] outline-none focus:border-accent disabled:opacity-60"
          />
          <button
            onClick={browse}
            disabled={busy}
            className="shrink-0 rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent disabled:opacity-60"
          >
            {t("client.browse")}
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
          <span className="w-16 shrink-0 text-[11px] font-semibold text-text-dim">
            {t("client.mode_label")}
          </span>
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
              <span className="text-text-dim">{t(`client.mode_${m}_hint`)}</span>
            </label>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={runScan}
            disabled={busy || !dir || !manifest}
            className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-1.5 text-[12px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95 disabled:opacity-50"
          >
            {report ? t("client.rescan") : t("client.scan")}
          </button>
          {busy && (
            <button
              onClick={() => commands.clientCancel()}
              className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("client.cancel")}
            </button>
          )}
          {phase === "loading" && (
            <span className="text-[11px] text-text-dim">{t("client.loading_manifest")}</span>
          )}
        </div>

        {busy && (
          <ProgressBar
            value={fraction}
            label={`${t(phase === "scanning" ? "client.scanning" : "client.downloading")} · ${progressLabel}`}
          />
        )}
      </div>

      {error && (
        <p className="shrink-0 rounded-[10px] border border-[rgba(239,68,68,0.3)] bg-[rgba(239,68,68,0.06)] px-3 py-2 text-[11px] text-red-400">
          {error}
        </p>
      )}

      {report && (
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 text-[11px]">
            <span className="text-green-500">
              {t("client.summary_ok", { count: String(report.okFiles) })}
            </span>
            <span className={report.issues.length > 0 ? "text-yellow-500" : "text-text-dim"}>
              {t("client.summary_issues", { count: String(report.issues.length) })}
            </span>
            {report.extraFiles.length > 0 && (
              <button
                onClick={() => setShowExtra((v) => !v)}
                className="text-text-dim underline decoration-dotted underline-offset-2 hover:text-accent"
              >
                {t("client.summary_extra", { count: String(report.extraFiles.length) })}
              </button>
            )}
            {report.cancelled && <span className="text-text-dim">{t("client.cancelled")}</span>}
          </div>

          {showExtra && report.extraFiles.length > 0 && (
            <div className="shrink-0 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] p-2.5">
              <p className="mb-1 text-[11px] text-text-dim">{t("client.extra_hint")}</p>
              <div className="max-h-24 overflow-y-auto font-mono text-[10px] text-text-dim">
                {report.extraFiles.map((f) => (
                  <div key={f} className="truncate">
                    {f}
                  </div>
                ))}
              </div>
            </div>
          )}

          {report.issues.some((i) => i.kind !== "missing") && (
            <p className="shrink-0 rounded-[10px] border border-[rgba(59,130,246,0.3)] bg-[rgba(59,130,246,0.06)] px-3 py-2 text-[11px] leading-relaxed text-blue-400">
              {t("client.mismatch_note")}
            </p>
          )}

          {report.issues.length === 0 ? (
            <p className="rounded-[10px] border border-[rgba(34,197,94,0.3)] bg-[rgba(34,197,94,0.06)] px-3 py-2 text-[12px] text-green-500">
              {t("client.all_good")}
            </p>
          ) : (
            <>
              <div className="flex shrink-0 items-center gap-3">
                <button
                  onClick={() =>
                    setSelected((prev) =>
                      prev.size === report.issues.length
                        ? new Set()
                        : new Set(report.issues.map((i) => i.path)),
                    )
                  }
                  disabled={busy}
                  className="rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-60"
                >
                  {t("client.select_all")}
                </button>
                <span className="text-[11px] text-text-dim">
                  {t("client.selected", {
                    count: String(selected.size),
                    size: formatBytes(selectedBytes),
                  })}
                </span>
              </div>

              <div className="min-h-0 flex-1 overflow-y-auto rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)]">
                {report.issues.map((issue) => (
                  <IssueRow
                    key={issue.path}
                    issue={issue}
                    checked={selected.has(issue.path)}
                    onToggle={() =>
                      setSelected((prev) => {
                        const next = new Set(prev);
                        if (next.has(issue.path)) next.delete(issue.path);
                        else next.add(issue.path);
                        return next;
                      })
                    }
                  />
                ))}
              </div>

              <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
                <span
                  className={`text-[11px] ${notEnoughSpace ? "text-red-400" : "text-text-dim"}`}
                >
                  {freeSpace !== null &&
                    (notEnoughSpace
                      ? t("client.not_enough_space", { size: formatBytes(freeSpace) })
                      : t("client.free_space", { size: formatBytes(freeSpace) }))}
                </span>
                <button
                  onClick={runDownload}
                  disabled={busy || selected.size === 0 || notEnoughSpace}
                  className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-1.5 text-[12px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95 disabled:opacity-50"
                >
                  {t("client.download")}
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {outcome && (
        <div className="shrink-0 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] p-3">
          <p className="text-[12px] font-semibold">
            {outcome.failures.length === 0
              ? t("client.download_done", { count: String(outcome.written) })
              : t("client.download_failed_some", {
                  written: String(outcome.written),
                  failed: String(outcome.failures.length),
                })}
          </p>
          {outcome.failures.length > 0 && (
            <div className="mt-1.5 max-h-24 overflow-y-auto font-mono text-[10px] text-red-400">
              {outcome.failures.map((f) => (
                <div key={f.path} className="truncate">
                  {f.path}: {f.error}
                </div>
              ))}
            </div>
          )}
          <p className="mt-1.5 text-[11px] text-text-dim">{t("client.rescan_hint")}</p>
        </div>
      )}
    </div>
  );
}
