import { useState, useEffect, useRef } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useUpdateStore } from "../../lib/stores/update-store";

export function StatusBar() {
  const [online, setOnline] = useState(false);
  const [ms, setMs] = useState<number | null>(null);
  const [beat, setBeat] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    async function heartbeat() {
      const t0 = performance.now();
      try {
        await fetch("https://tw.beanfun.com/favicon.ico", {
          mode: "no-cors",
          cache: "no-store",
        });
        const elapsed = Math.round(performance.now() - t0);
        setOnline(true);
        setMs(elapsed);
        setBeat(true);
        setTimeout(() => setBeat(false), 400);
      } catch {
        setOnline(false);
        setMs(null);
      }
    }

    heartbeat();
    intervalRef.current = setInterval(heartbeat, 5000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  return (
    <div className="flex shrink-0 flex-col">
      <DownloadProgressBar />
      <div className="flex items-center justify-center gap-1.5 px-2 py-0.5 font-mono text-[12px] text-text-dim">
        <span
          className={`h-1.5 w-1.5 shrink-0 rounded-full transition-colors ${
            online ? "bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.4)]" : "bg-[var(--danger)]"
          } ${beat ? "animate-[hbeat_0.4s_ease]" : ""}`}
        />
        <span>{online ? "ONLINE" : "OFFLINE"}</span>
        <span className="text-[12px] text-text-faint">{ms !== null ? `${ms}ms` : "--"}</span>
      </div>
    </div>
  );
}

function formatSpeed(bps: number): string {
  if (bps < 1024) return `${bps} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
}

function DownloadProgressBar() {
  const { t } = useTranslation();
  const status = useUpdateStore((s) => s.status);
  const downloaded = useUpdateStore((s) => s.downloaded);
  const total = useUpdateStore((s) => s.total);
  const speed = useUpdateStore((s) => s.speed);
  const version = useUpdateStore((s) => s.version);
  const method = useUpdateStore((s) => s.method);
  const error = useUpdateStore((s) => s.error);

  if (status === "idle") return null;

  const percent = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
  const isDone = status === "done";
  const hasError = status === "error";

  async function handleRestart() {
    try {
      await commands.restartApp();
    } catch {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    }
  }

  if (isDone) {
    return (
      <div className="mx-2 mb-0.5 flex items-center justify-between rounded-md border border-accent/20 bg-accent/5 px-2.5 py-1">
        <span className="text-[10px] text-accent">
          v{version} {t("update.restart_required")}
        </span>
        <button
          onClick={handleRestart}
          className="rounded px-2 py-0.5 text-[10px] font-semibold text-accent transition-colors hover:bg-accent/10"
        >
          {t("update.restart")}
        </button>
      </div>
    );
  }

  if (hasError) {
    return (
      <div className="mx-2 mb-0.5 rounded-md border border-[var(--danger)]/20 bg-[var(--danger)]/5 px-2.5 py-1">
        <span className="text-[10px] text-[var(--danger)]">{error}</span>
      </div>
    );
  }

  // Downloading
  return (
    <div className="mx-2 mb-0.5 rounded-md border border-border bg-[var(--surface)] px-2.5 py-1">
      <div className="mb-1 h-1 overflow-hidden rounded-full bg-[var(--bg)]">
        <div
          className="h-full rounded-full bg-gradient-to-r from-accent to-[#c47a1a] transition-all duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
      <div className="flex items-center justify-between text-[10px] text-text-dim">
        <span>
          v{version} · {percent}%
        </span>
        <span>
          {formatSpeed(speed)}
          {" · "}
          {method === "proxy" ? t("update.method_proxy") : t("update.method_direct")}
        </span>
      </div>
    </div>
  );
}
