import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { Modal } from "./Modal";
import type { UpdateInfoDto } from "../../lib/types";

interface Props {
  update: UpdateInfoDto;
  onClose: () => void;
}

export function UpdateDialog({ update, onClose }: Props) {
  const { t } = useTranslation();
  const [downloading, setDownloading] = useState(false);
  const [needsProxy, setNeedsProxy] = useState(false);
  const [useProxy, setUseProxy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  useEffect(() => {
    commands
      .testGithubAccess()
      .then((ok) => {
        if (!ok) {
          setNeedsProxy(true);
          setUseProxy(true);
        }
      })
      .catch(() => {});
  }, []);

  async function handleDownload() {
    setDownloading(true);
    setError(null);
    try {
      await commands.applyUpdate(update.downloadUrl, useProxy);
      setDone(true);
      setDownloading(false);
    } catch (e) {
      setError(
        typeof e === "object" && e !== null && "message" in e
          ? String((e as Record<string, unknown>).message)
          : "Download failed",
      );
      setDownloading(false);
    }
  }

  async function handleRestart() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }

  return (
    <Modal isOpen onClose={done ? handleRestart : onClose} title={t("update.title")}>
      <div className="flex flex-col gap-4">
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
              {done ? t("update.restart_required") : t("update.new_version_available")}
            </div>
          </div>
        </div>

        {!done && update.changelog && (
          <div className="max-h-[160px] overflow-y-auto rounded-lg bg-[var(--surface)] p-3 text-[11px] leading-relaxed text-text-dim">
            {update.changelog.split("\n").map((line, i) => (
              <div key={i}>{line || "\u00A0"}</div>
            ))}
          </div>
        )}

        {!done && needsProxy && (
          <label className="flex items-center gap-2 text-[11px] text-text-dim">
            <input
              type="checkbox"
              checked={useProxy}
              onChange={(e) => setUseProxy(e.target.checked)}
              className="h-3.5 w-3.5 accent-accent"
            />
            {t("update.use_mirror")}
          </label>
        )}

        {error && <div className="text-[11px] text-[var(--danger)]">{error}</div>}

        {done ? (
          <div className="flex justify-end">
            <button
              onClick={handleRestart}
              className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-semibold text-white transition-all hover:opacity-90 active:scale-95"
            >
              {t("update.restart")}
            </button>
          </div>
        ) : (
          <div className="flex justify-end gap-2">
            <button
              onClick={onClose}
              disabled={downloading}
              className="rounded-lg px-4 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("update.skip")}
            </button>
            <button
              onClick={handleDownload}
              disabled={downloading || !update.downloadUrl}
              className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-semibold text-white transition-all hover:opacity-90 active:scale-95 disabled:opacity-50"
            >
              {downloading ? t("update.downloading") : t("update.download")}
            </button>
          </div>
        )}
      </div>
    </Modal>
  );
}
