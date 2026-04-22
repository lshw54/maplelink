import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

interface LogEntry {
  ts: string;
  level: LogLevel;
  msg: string;
}

const LEVEL_COLORS: Record<LogLevel, string> = {
  trace: "#636878",
  debug: "#7aa2f7",
  info: "#9ece6a",
  warn: "#e0af68",
  error: "#f7768e",
};

const LEVEL_ORDER: LogLevel[] = ["trace", "debug", "info", "warn", "error"];

function maskSensitive(msg: string): string {
  return msg
    .replace(/\b(T9|HK)[a-zA-Z0-9]{6,}/g, (m) => m.slice(0, 4) + "***" + m.slice(-2))
    .replace(/\b[a-f0-9]{20,}\b/gi, (m) => m.slice(0, 8) + "***" + m.slice(-4))
    .replace(
      /(bfWebToken[=:]\s*)([^\s;,]+)/gi,
      (_, p, v) => p + v.slice(0, 4) + "***" + v.slice(-2),
    )
    .replace(
      /\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi,
      (m) => m.slice(0, 8) + "-****-****",
    )
    .replace(/\b[\w.-]+@[\w.-]+\.\w+\b/g, (m) => {
      const [l, d] = m.split("@");
      return (l?.slice(0, 2) ?? "") + "***@" + (d ?? "");
    });
}

function parseLogLine(line: string): LogEntry | null {
  // Match tracing format: "2026-04-22T04:48:19.089+08:00  INFO module::path: message"
  // Also matches UTC "Z" suffix and optional span fields like "[]"
  const m = line.match(
    /^(\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2}))\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+(.+)$/,
  );
  if (m) {
    let msg = m[3] ?? "";
    // Strip leading "[] " span markers and module path prefix
    msg = msg.replace(/^(\[\S*\]\s*)*/, "").replace(/^\S+::\S+:\s*/, "");
    return {
      ts: (m[1] ?? "").replace("T", " ").slice(11, 19),
      level: (m[2] ?? "info").toLowerCase() as LogLevel,
      msg: maskSensitive(msg),
    };
  }
  if (line.trim()) return { ts: "", level: "info", msg: maskSensitive(line) };
  return null;
}

export function DebugConsole() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<LogLevel>("info");
  const [search, setSearch] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoScroll && scrollRef.current)
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [logs, autoScroll]);

  // Initial load + periodic refresh
  useEffect(() => {
    function fetchLogs() {
      invoke<string>("get_recent_logs")
        .then((text) => {
          setLogs(
            text
              .split("\n")
              .map(parseLogLine)
              .filter((e): e is LogEntry => e !== null),
          );
        })
        .catch(() => {});
    }
    fetchLogs();
    const interval = setInterval(fetchLogs, 2000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const u = listen<{ level: string; module: string; message: string }>("debug-log", (ev) => {
      setLogs((p) => [
        ...p.slice(-999),
        {
          ts: new Date().toLocaleTimeString("en-GB"),
          level: (ev.payload.level.toLowerCase() || "info") as LogLevel,
          msg: maskSensitive(`[${ev.payload.module}] ${ev.payload.message}`),
        },
      ]);
    });
    return () => {
      u.then((f) => f());
    };
  }, []);

  const filtered = logs.filter((e) => {
    if (LEVEL_ORDER.indexOf(e.level) < LEVEL_ORDER.indexOf(filter)) return false;
    if (search && !e.msg.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const handleCopy = useCallback(() => {
    navigator.clipboard
      .writeText(filtered.map((e) => `[${e.ts}] ${e.level.toUpperCase()} ${e.msg}`).join("\n"))
      .catch(() => {});
  }, [filtered]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Titlebar */}
      <div
        onMouseDown={(e) => {
          if (!(e.target as HTMLElement).closest("button")) {
            e.preventDefault();
            getCurrentWindow().startDragging();
          }
        }}
        style={{
          display: "flex",
          alignItems: "center",
          height: 28,
          borderBottom: "1px solid rgba(255,255,255,0.06)",
          background: "#0d0f14",
          flexShrink: 0,
        }}
      >
        <span
          style={{
            paddingLeft: 12,
            fontSize: 10,
            fontWeight: 700,
            letterSpacing: 2,
            color: "#4a4e5a",
          }}
        >
          DEBUG CONSOLE
        </span>
        <div style={{ flex: 1 }} />
        <button
          onClick={() =>
            invoke("toggle_debug_window", { enable: false }).catch(() => getCurrentWindow().close())
          }
          style={{
            width: 28,
            height: 28,
            border: "none",
            background: "transparent",
            color: "#4a4e5a",
            fontSize: 14,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = "#e81123";
            e.currentTarget.style.color = "#fff";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = "transparent";
            e.currentTarget.style.color = "#4a4e5a";
          }}
        >
          ×
        </button>
      </div>

      {/* Toolbar */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "4px 10px",
          borderBottom: "1px solid rgba(255,255,255,0.06)",
          background: "#0d0f14",
          flexShrink: 0,
        }}
      >
        <select
          name="log-level-filter"
          value={filter}
          onChange={(e) => setFilter(e.target.value as LogLevel)}
          style={{
            background: "#181b22",
            color: "#c8cad0",
            border: "1px solid rgba(255,255,255,0.1)",
            borderRadius: 3,
            padding: "2px 4px",
            fontSize: 10,
            outline: "none",
          }}
        >
          {LEVEL_ORDER.map((l) => (
            <option key={l} value={l}>
              {l.toUpperCase()}
            </option>
          ))}
        </select>
        <input
          name="log-search"
          autoComplete="off"
          data-form-type="other"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Filter..."
          style={{
            flex: 1,
            background: "#181b22",
            color: "#c8cad0",
            border: "1px solid rgba(255,255,255,0.1)",
            borderRadius: 3,
            padding: "2px 6px",
            fontSize: 10,
            outline: "none",
          }}
        />
        <button
          onClick={() => setAutoScroll(!autoScroll)}
          title="Auto-scroll"
          style={{
            padding: "2px 5px",
            fontSize: 10,
            borderRadius: 3,
            border: "none",
            cursor: "pointer",
            background: autoScroll ? "rgba(158,206,106,0.15)" : "transparent",
            color: autoScroll ? "#9ece6a" : "#4a4e5a",
          }}
        >
          ↓
        </button>
        <button
          onClick={handleCopy}
          title="Copy"
          style={{
            padding: "2px 5px",
            fontSize: 10,
            border: "none",
            background: "transparent",
            color: "#4a4e5a",
            cursor: "pointer",
          }}
        >
          📋
        </button>
        <button
          onClick={() => invoke("open_log_folder").catch(() => {})}
          title="Open folder"
          style={{
            padding: "2px 5px",
            fontSize: 10,
            border: "none",
            background: "transparent",
            color: "#4a4e5a",
            cursor: "pointer",
          }}
        >
          📂
        </button>
        <button
          onClick={() => setLogs([])}
          title="Clear"
          style={{
            padding: "2px 5px",
            fontSize: 10,
            border: "none",
            background: "transparent",
            color: "#4a4e5a",
            cursor: "pointer",
          }}
        >
          🗑
        </button>
        <span style={{ fontSize: 10, color: "#4a4e5a" }}>{filtered.length}</span>
      </div>

      {/* Log area — uses inline style to guarantee scroll works */}
      <div
        ref={scrollRef}
        style={{ flex: 1, overflowY: "auto", overflowX: "hidden", padding: 8, lineHeight: 1.7 }}
      >
        {filtered.length === 0 ? (
          <div
            style={{
              display: "flex",
              height: "100%",
              alignItems: "center",
              justifyContent: "center",
              color: "#4a4e5a",
              fontSize: 11,
            }}
          >
            No logs
          </div>
        ) : (
          filtered.map((entry, i) => (
            <div key={i} style={{ whiteSpace: "pre-wrap", wordBreak: "break-all" }}>
              <span style={{ color: "#333640" }}>[{entry.ts}]</span>{" "}
              <span
                style={{
                  display: "inline-block",
                  width: 42,
                  textAlign: "right",
                  color: LEVEL_COLORS[entry.level],
                }}
              >
                {entry.level.toUpperCase()}
              </span>{" "}
              <span>{entry.msg}</span>
            </div>
          ))
        )}
      </div>

      {/* Status */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          padding: "2px 10px",
          borderTop: "1px solid rgba(255,255,255,0.06)",
          background: "#0d0f14",
          flexShrink: 0,
          fontSize: 9,
          color: "#333640",
        }}
      >
        <span>%LOCALAPPDATA%\com.maplelink.app\logs\maplelink.log</span>
        <span>{logs.length} entries</span>
      </div>
    </div>
  );
}
