import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface LogEntry {
  ts: string;
  level: string;
  msg: string;
}

export function DebugConsole() {
  const [logs, setLogs] = useState<LogEntry[]>([
    { ts: formatTime(), level: "info", msg: "Debug console ready." },
  ]);
  const areaRef = useRef<HTMLDivElement>(null);

  // Auto-scroll on new logs
  useEffect(() => {
    if (areaRef.current) {
      areaRef.current.scrollTop = areaRef.current.scrollHeight;
    }
  }, [logs]);

  // Listen for log events from backend
  useEffect(() => {
    const unlisten = listen<{ level: string; module: string; message: string }>(
      "debug-log",
      (event) => {
        setLogs((prev) => [
          ...prev,
          {
            ts: formatTime(),
            level: event.payload.level,
            msg: `[${event.payload.module}] ${event.payload.message}`,
          },
        ]);
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  function handleClose() {
    invoke("toggle_debug_window", { enable: false }).catch(() => {
      getCurrentWindow().close();
    });
  }

  function handleDragStart(e: React.MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    getCurrentWindow().startDragging();
  }

  return (
    <div className="flex h-screen flex-col">
      {/* Titlebar */}
      <div
        onMouseDown={handleDragStart}
        className="flex h-7 shrink-0 items-center border-b border-[rgba(255,255,255,0.05)] bg-[#0d0f13]"
      >
        <span className="pl-3 text-[12px] font-semibold tracking-[1px] text-[#555a65]">
          DEBUG CONSOLE
        </span>
        <div className="flex-1" />
        <button
          onClick={handleClose}
          className="flex h-7 w-7 items-center justify-center rounded-tr-lg text-sm text-[#555a65] transition-all hover:bg-[#e81123] hover:text-white"
        >
          ×
        </button>
      </div>

      {/* Log area */}
      <div ref={areaRef} className="flex-1 overflow-y-auto p-2 leading-relaxed">
        {logs.map((entry, i) => (
          <div
            key={i}
            className={`whitespace-pre-wrap break-all ${
              entry.level === "error"
                ? "text-[#e81123]"
                : entry.level === "warn"
                  ? "text-[#e8a23a]"
                  : ""
            }`}
          >
            <span className="text-[#555a65]">[{entry.ts}]</span> <span>{entry.msg}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function formatTime(): string {
  return new Date().toLocaleTimeString("en-GB");
}
