import { useState, useRef } from "react";
import { useTranslation } from "../../lib/i18n";
import { useAuthStore, type SessionEntry } from "../../lib/stores/auth-store";
import { useUiStore } from "../../lib/stores/ui-store";
import { commands } from "../../lib/tauri";

export function SessionTabs() {
  const { t } = useTranslation();
  const sessions = useAuthStore((s) => s.sessions);
  const activeSessionId = useAuthStore((s) => s.activeSessionId);
  const setActiveSessionId = useAuthStore((s) => s.setActiveSessionId);
  const removeSession = useAuthStore((s) => s.removeSession);
  const setPage = useUiStore((s) => s.setPage);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Use refs for drag state so closures always see latest values
  const dragSrcId = useRef<string | null>(null);
  const dragStartX = useRef(0);
  const dragOverIdxRef = useRef<number | null>(null);
  const [, forceRender] = useState(0);

  const entries = Array.from(sessions.values());
  if (entries.length === 0) return null;

  async function handleClose(sessionId: string) {
    try {
      await commands.logout(sessionId);
    } catch {
      /* ignore */
    }
    removeSession(sessionId);
    if (useAuthStore.getState().sessions.size === 0) setPage("login");
  }

  function handleAdd() {
    useUiStore.getState().addingSession = true;
    setPage("login");
  }

  function startRename(entry: SessionEntry) {
    setEditingId(entry.sessionId);
    setEditValue(entry.session.accountName);
    setTimeout(() => inputRef.current?.select(), 0);
  }

  function commitRename() {
    if (editingId && editValue.trim()) {
      const store = useAuthStore.getState();
      const entry = store.sessions.get(editingId);
      if (entry) {
        const updated = { ...entry, session: { ...entry.session, accountName: editValue.trim() } };
        const newSessions = new Map(store.sessions);
        newSessions.set(editingId, updated);
        useAuthStore.setState({
          sessions: newSessions,
          ...(store.activeSessionId === editingId ? { session: updated.session } : {}),
        });
      }
    }
    setEditingId(null);
  }

  function handleTabMouseDown(e: React.MouseEvent, id: string) {
    if (editingId || e.button !== 0) return;
    dragSrcId.current = id;
    dragStartX.current = e.clientX;
    dragOverIdxRef.current = null;

    const onMove = (ev: MouseEvent) => {
      if (!containerRef.current || !dragSrcId.current) return;
      if (Math.abs(ev.clientX - dragStartX.current) < 5) return;

      const tabs = containerRef.current.querySelectorAll<HTMLElement>("[data-tab-id]");
      let overIdx: number | null = null;
      tabs.forEach((tab, idx) => {
        const rect = tab.getBoundingClientRect();
        if (ev.clientX >= rect.left && ev.clientX <= rect.right) overIdx = idx;
      });
      if (overIdx !== dragOverIdxRef.current) {
        dragOverIdxRef.current = overIdx;
        forceRender((n) => n + 1);
      }
    };

    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);

      const srcId = dragSrcId.current;
      const targetIdx = dragOverIdxRef.current;
      dragSrcId.current = null;
      dragOverIdxRef.current = null;
      forceRender((n) => n + 1);

      if (!srcId || targetIdx === null) return;

      const store = useAuthStore.getState();
      const arr = Array.from(store.sessions.entries());
      const fromIdx = arr.findIndex(([sid]) => sid === srcId);
      if (fromIdx >= 0 && targetIdx >= 0 && fromIdx !== targetIdx) {
        const moved = arr.splice(fromIdx, 1)[0];
        if (moved) {
          arr.splice(targetIdx, 0, moved);
          useAuthStore.setState({ sessions: new Map(arr) });
        }
      }
    };

    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  return (
    <div
      ref={containerRef}
      className="flex shrink-0 items-center gap-0.5 overflow-x-auto border-b border-border bg-[var(--bg)] px-1 py-0.5"
    >
      {entries.map((entry, idx) => {
        const isActive = entry.sessionId === activeSessionId;
        const isEditing = editingId === entry.sessionId;
        const isDragOver = dragOverIdxRef.current === idx && dragSrcId.current !== null;

        return (
          <div
            key={entry.sessionId}
            data-tab-id={entry.sessionId}
            onMouseDown={(e) => handleTabMouseDown(e, entry.sessionId)}
            onClick={() => !isEditing && setActiveSessionId(entry.sessionId)}
            className={`group flex cursor-pointer select-none items-center gap-1 rounded-t-md px-2 py-1 text-[11px] transition-colors ${
              isActive
                ? "bg-[var(--surface)] text-accent"
                : "text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
            } ${isDragOver ? "border-l-2 border-accent" : ""}`}
          >
            <span
              className={`h-1.5 w-1.5 shrink-0 rounded-full ${entry.session.region === "TW" ? "bg-blue-400" : "bg-green-400"}`}
            />
            {isEditing ? (
              <input
                ref={inputRef}
                value={editValue}
                onChange={(e) => setEditValue(e.target.value)}
                onBlur={commitRename}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitRename();
                  if (e.key === "Escape") setEditingId(null);
                }}
                onClick={(e) => e.stopPropagation()}
                className="w-[80px] rounded bg-[var(--surface-hover)] px-1 text-[11px] text-[var(--text)] outline-none"
                autoFocus
              />
            ) : (
              <span className="max-w-[100px] truncate">{entry.session.accountName}</span>
            )}
            <span className="text-[9px] text-text-faint">{entry.session.region}</span>
            {!isEditing && (
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  startRename(entry);
                }}
                className="rounded p-0.5 text-text-faint opacity-0 transition-all hover:text-accent group-hover:opacity-100"
                title="Rename"
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 16 16"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M11.5 1.5l3 3L5 14H2v-3L11.5 1.5z" />
                </svg>
              </span>
            )}
            <span
              onClick={(e) => {
                e.stopPropagation();
                handleClose(entry.sessionId);
              }}
              className="rounded p-0.5 text-text-faint opacity-0 transition-all hover:bg-[rgba(239,68,68,0.1)] hover:text-red-400 group-hover:opacity-100"
              title="Close"
            >
              <svg
                width="8"
                height="8"
                viewBox="0 0 12 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              >
                <path d="M3 3L9 9M9 3L3 9" />
              </svg>
            </span>
          </div>
        );
      })}
      <button
        onClick={handleAdd}
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-[11px] text-text-faint transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
        title={t("launcher.add_session")}
      >
        +
      </button>
    </div>
  );
}
