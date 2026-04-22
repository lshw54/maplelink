import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useUpdateStore } from "../../lib/stores/update-store";
import { Modal } from "./Modal";
import type { UpdateInfoDto } from "../../lib/types";

interface Props {
  update: UpdateInfoDto;
  onClose: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`;
}

export function UpdateDialog({ update, onClose }: Props) {
  const { t } = useTranslation();
  const store = useUpdateStore();
  const [needsProxy, setNeedsProxy] = useState(false);
  const [useProxy, setUseProxy] = useState(false);
  const [probeComplete, setProbeComplete] = useState(false);

  const isDownloading = store.status === "downloading";
  const isDone = store.status === "done";
  const hasError = store.status === "error";

  // Probe GitHub connectivity on mount
  useEffect(() => {
    commands
      .testGithubAccess()
      .then((ok) => {
        if (!ok) {
          setNeedsProxy(true);
          setUseProxy(true);
        }
        setProbeComplete(true);
      })
      .catch(() => setProbeComplete(true));
  }, []);

  // Listen for download progress events from backend
  useEffect(() => {
    const unlisten = listen<{ downloaded: number; total: number; speed: number }>(
      "update-download-progress",
      (event) => {
        store.updateProgress(event.payload.downloaded, event.payload.total, event.payload.speed);
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function handleDownload() {
    const method = useProxy ? "proxy" : "direct";
    store.startDownload(update.version, update.downloadUrl, update.isPrerelease, method);
    try {
      await commands.applyUpdate(update.downloadUrl, useProxy);
      store.setDone();
    } catch (e) {
      const msg =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as Record<string, unknown>).message)
          : "Download failed";
      store.setError(msg);
    }
  }

  function handleBackground() {
    // Close dialog — download continues, progress shows in StatusBar
    onClose();
  }

  async function handleRestart() {
    try {
      await commands.restartApp();
    } catch {
      // Fallback: just close the window
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    }
  }

  const percent =
    store.total > 0 ? Math.min(100, Math.round((store.downloaded / store.total) * 100)) : 0;

  return (
    <Modal isOpen onClose={isDone ? handleRestart : onClose} title={t("update.title")}>
      <div className="flex flex-col gap-4">
        {/* Header */}
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-[10px] bg-gradient-to-br from-accent to-[#c47a1a] text-lg font-bold text-white shadow-lg">
            ↑
          </div>
          <div>
            <div className="text-sm font-semibold text-[var(--text)]">
              v{update.version}
              {update.isPrerelease && (
                <span className="ml-2 rounded bg-[rgba(232,162,58,0.15)] px-1.5 py-0.5 text-[10px] text-accent">
                  PRE
                </span>
              )}
            </div>
            <div className="text-[11px] text-text-dim">
              {isDone ? t("update.restart_required") : t("update.new_version_available")}
            </div>
          </div>
        </div>

        {/* Changelog */}
        {!isDone && !isDownloading && update.changelog && (
          <div className="max-h-[120px] overflow-y-auto rounded-lg bg-[var(--surface)] p-3 text-[11px] leading-relaxed text-text-dim">
            {update.changelog.split("\n").map((line, i) => (
              <div key={i}>{line || "\u00A0"}</div>
            ))}
          </div>
        )}

        {/* Download progress */}
        {isDownloading && (
          <div className="flex flex-col gap-2">
            <div className="h-2 overflow-hidden rounded-full bg-[var(--surface)]">
              <div
                className="h-full rounded-full bg-gradient-to-r from-accent to-[#c47a1a] transition-all duration-300"
                style={{ width: `${percent}%` }}
              />
            </div>
            <div className="flex items-center justify-between text-[11px] text-text-dim">
              <span>
                {formatBytes(store.downloaded)}
                {store.total > 0 ? ` / ${formatBytes(store.total)}` : ""}
                {" · "}
                {percent}%
              </span>
              <span>{formatSpeed(store.speed)}</span>
            </div>
            <div className="text-[10px] text-text-faint">
              {t("update.method")}:{" "}
              {useProxy ? t("update.method_proxy") : t("update.method_direct")}
            </div>
          </div>
        )}

        {/* Error */}
        {hasError && store.error && (
          <div className="text-[11px] text-[var(--danger)]">{store.error}</div>
        )}

        {/* Proxy toggle — always visible before download starts */}
        {!isDownloading && !isDone && (
          <label className="flex items-center gap-2 text-[11px] text-text-dim">
            <input
              type="checkbox"
              name="use-proxy"
              checked={useProxy}
              onChange={(e) => setUseProxy(e.target.checked)}
              className="h-3.5 w-3.5 accent-accent"
            />
            {t("update.use_mirror")}
            {needsProxy && (
              <span className="text-[10px] text-accent">({t("update.mirror_recommended")})</span>
            )}
          </label>
        )}

        {/* Actions */}
        {isDone ? (
          <div className="flex justify-end gap-2">
            <button
              onClick={onClose}
              className="rounded-lg px-4 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("update.restart_later")}
            </button>
            <button
              onClick={handleRestart}
              className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-semibold text-white transition-all hover:opacity-90 active:scale-95"
            >
              {t("update.restart")}
            </button>
          </div>
        ) : isDownloading ? (
          <div className="flex justify-end">
            <button
              onClick={handleBackground}
              className="rounded-lg px-4 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("update.background")}
            </button>
          </div>
        ) : (
          <div className="flex justify-end gap-2">
            <button
              onClick={onClose}
              className="rounded-lg px-4 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("update.skip")}
            </button>
            <button
              onClick={handleDownload}
              disabled={!update.downloadUrl || !probeComplete}
              className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-semibold text-white transition-all hover:opacity-90 active:scale-95 disabled:opacity-50"
            >
              {t("update.download")}
            </button>
          </div>
        )}
      </div>
    </Modal>
  );
}
