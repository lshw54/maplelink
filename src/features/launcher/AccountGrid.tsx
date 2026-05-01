import { useState, useCallback, useRef, useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { useGameAccounts, useRefreshAccounts } from "../../lib/hooks/use-accounts";
import { useQueryClient } from "@tanstack/react-query";
import { useConfigStore } from "../../lib/stores/config-store";
import { commands } from "../../lib/tauri";
import { AccountContextMenu } from "./AccountContextMenu";
import type { GameAccountDto } from "../../lib/types";

interface AccountGridProps {
  selectedAccountId: string | null;
  onSelectAccount: (account: GameAccountDto) => void;
}

interface ContextState {
  position: { x: number; y: number };
  accountId: string;
}

type ViewMode = "card" | "list";

export function AccountGrid({ selectedAccountId, onSelectAccount }: AccountGridProps) {
  const { t } = useTranslation();
  const { data: accounts, isLoading } = useGameAccounts();
  const refreshAccounts = useRefreshAccounts();
  const queryClient = useQueryClient();
  const [contextMenu, setContextMenu] = useState<ContextState | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const viewMode = useConfigStore((s) => s.config?.accountViewMode ?? "card") as ViewMode;

  // Drag reorder
  const [dragOrder, setDragOrder] = useState<GameAccountDto[] | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [bumpId, setBumpId] = useState<string | null>(null);
  const dragState = useRef({ srcIdx: 0, startY: 0, active: false, items: [] as GameAccountDto[] });
  const displayAccounts = dragOrder ?? accounts ?? [];

  function toggleViewMode() {
    const next = viewMode === "card" ? "list" : "card";
    // Update local store immediately for instant UI feedback
    const store = useConfigStore.getState();
    if (store.config) {
      useConfigStore.setState({ config: { ...store.config, accountViewMode: next } });
    }
    commands.setConfig("accountViewMode", next).catch(() => {});
  }

  const handleContextMenu = useCallback((e: React.MouseEvent, accountId: string) => {
    e.preventDefault();
    setContextMenu({ position: { x: e.clientX, y: e.clientY }, accountId });
  }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  function handleCopyAccount(e: React.MouseEvent, accountId: string) {
    e.stopPropagation();
    navigator.clipboard.writeText(accountId);
    setCopiedId(accountId);
    setTimeout(() => setCopiedId(null), 1500);
  }

  function onGripDown(e: React.MouseEvent, idx: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const items = [...(accounts ?? [])];
    dragState.current = { srcIdx: idx, startY: e.clientY, active: false, items };

    const onMove = (ev: MouseEvent) => {
      if (!dragState.current.active) {
        if (Math.abs(ev.clientY - dragState.current.startY) < 4) return;
        dragState.current.active = true;
        setDraggingId(dragState.current.items[dragState.current.srcIdx]?.id ?? null);
        setDragOrder([...dragState.current.items]);
      }
      // Find target index from mouse Y
      const els = document.querySelectorAll<HTMLElement>("[data-acct-idx]");
      let target = dragState.current.srcIdx;
      els.forEach((el, i) => {
        const r = el.getBoundingClientRect();
        if (ev.clientY > r.top + r.height / 2) target = i;
      });
      target = Math.max(0, Math.min(target, dragState.current.items.length - 1));
      if (target !== dragState.current.srcIdx) {
        const arr = dragState.current.items;
        const displacedId = arr[target]?.id ?? null;
        const [moved] = arr.splice(dragState.current.srcIdx, 1);
        if (moved) arr.splice(target, 0, moved);
        dragState.current.srcIdx = target;
        setDragOrder([...arr]);
        // Bump the displaced item
        setBumpId(displacedId);
        setTimeout(() => setBumpId(null), 200);
      }
    };

    const onUp = async () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      if (dragState.current.active) {
        await commands.setAccountOrder(dragState.current.items.map((a) => a.id));
        await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      }
      setDragOrder(null);
      setDraggingId(null);
      dragState.current.active = false;
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  // Enter key → quick-copy selected account ID
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Enter" && selectedAccountId) {
        navigator.clipboard.writeText(selectedAccountId);
        setCopiedId(selectedAccountId);
        setTimeout(() => setCopiedId(null), 1500);
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [selectedAccountId]);

  return (
    <div className="flex flex-1 flex-col gap-2 overflow-hidden">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-semibold tracking-[1px] text-text-dim">{t("launcher.accounts")}</span>
          {/* View toggle */}
          <button
            onClick={toggleViewMode}
            title={viewMode === "card" ? t("launcher.view_list") : t("launcher.view_card")}
            className="rounded p-0.5 text-text-faint transition-colors hover:text-accent"
          >
            {viewMode === "card" ? (
              <svg
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              >
                <line x1="2" y1="4" x2="14" y2="4" />
                <line x1="2" y1="8" x2="14" y2="8" />
                <line x1="2" y1="12" x2="14" y2="12" />
              </svg>
            ) : (
              <svg
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              >
                <rect x="1" y="1" width="6" height="6" rx="1" />
                <rect x="9" y="1" width="6" height="6" rx="1" />
                <rect x="1" y="9" width="6" height="6" rx="1" />
                <rect x="9" y="9" width="6" height="6" rx="1" />
              </svg>
            )}
          </button>
        </div>
        <button onClick={refreshAccounts} className="text-[12px] text-accent hover:underline">
          {t("launcher.refresh")}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <p className="py-4 text-center text-xs text-text-dim">{t("app.loading")}</p>
        ) : !accounts?.length ? (
          <p className="py-4 text-center text-xs text-text-dim">{t("launcher.no_accounts")}</p>
        ) : viewMode === "card" ? (
          <div className="grid grid-cols-2 gap-2">
            {displayAccounts.map((account, idx) => (
              <CardItem
                key={account.id}
                account={account}
                isSelected={selectedAccountId === account.id}
                isCopied={copiedId === account.id}
                isDragging={draggingId === account.id}
                isBumped={bumpId === account.id}
                idx={idx}
                onSelect={() => !dragState.current.active && onSelectAccount(account)}
                onContextMenu={(e) => handleContextMenu(e, account.id)}
                onCopy={(e) => handleCopyAccount(e, account.id)}
                onGripDown={(e) => onGripDown(e, idx)}
                copyTitle={t("launcher.context.copy_account")}
              />
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-1">
            {displayAccounts.map((account, idx) => (
              <ListItem
                key={account.id}
                account={account}
                isSelected={selectedAccountId === account.id}
                isCopied={copiedId === account.id}
                isDragging={draggingId === account.id}
                isBumped={bumpId === account.id}
                idx={idx}
                onSelect={() => !dragState.current.active && onSelectAccount(account)}
                onContextMenu={(e) => handleContextMenu(e, account.id)}
                onCopy={(e) => handleCopyAccount(e, account.id)}
                onGripDown={(e) => onGripDown(e, idx)}
                copyTitle={t("launcher.context.copy_account")}
              />
            ))}
          </div>
        )}
      </div>

      <AccountContextMenu
        position={contextMenu?.position ?? null}
        account={displayAccounts.find((a) => a.id === contextMenu?.accountId) ?? null}
        onClose={closeContextMenu}
      />
    </div>
  );
}

/* ---- Card view item ---- */
function CardItem({
  account,
  isSelected,
  isCopied,
  isDragging,
  isBumped,
  idx,
  onSelect,
  onContextMenu,
  onCopy,
  onGripDown,
  copyTitle,
}: {
  account: GameAccountDto;
  isSelected: boolean;
  isCopied: boolean;
  isDragging: boolean;
  isBumped: boolean;
  idx: number;
  onSelect: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onCopy: (e: React.MouseEvent) => void;
  onGripDown: (e: React.MouseEvent) => void;
  copyTitle: string;
}) {
  const initial = account.displayName.charAt(0).toUpperCase();
  return (
    <button
      data-acct-idx={idx}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      className={`group relative flex flex-col items-center gap-1.5 rounded-xl border p-3 text-center backdrop-blur-sm transition-all duration-150 ${
        isDragging ? "opacity-50" : ""
      } ${isBumped ? "animate-[dragBump_0.2s_ease]" : ""} ${
        isSelected
          ? "border-accent bg-[rgba(232,162,58,0.05)] shadow-[0_0_20px_rgba(232,162,58,0.15)]"
          : "border-border bg-[var(--surface)] hover:border-[var(--border)] hover:bg-[var(--surface-hover)]"
      }`}
    >
      <CopyIcon
        isCopied={isCopied}
        onClick={onCopy}
        title={copyTitle}
        position="absolute top-1.5 right-1.5"
      />
      <span
        onMouseDown={onGripDown}
        className="absolute top-1.5 left-1.5 cursor-grab rounded p-0.5 text-text-faint opacity-0 transition-opacity group-hover:opacity-50 hover:text-accent active:cursor-grabbing"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
          <circle cx="3" cy="2" r="1" />
          <circle cx="7" cy="2" r="1" />
          <circle cx="3" cy="5" r="1" />
          <circle cx="7" cy="5" r="1" />
          <circle cx="3" cy="8" r="1" />
          <circle cx="7" cy="8" r="1" />
        </svg>
      </span>
      <div
        className={`flex h-[38px] w-[38px] items-center justify-center rounded-full border-2 text-sm font-bold transition-colors duration-150 ${
          isSelected
            ? "border-accent bg-[var(--surface-hover)] text-accent"
            : "border-border bg-[var(--surface-hover)] text-text-dim group-hover:border-accent group-hover:text-accent"
        }`}
      >
        {initial}
      </div>
      <span className="truncate text-xs font-medium text-[var(--text)]">{account.displayName}</span>
      <span className="truncate font-mono text-[12px] text-text-faint">{account.id}</span>
    </button>
  );
}

/* ---- List view item ---- */
function ListItem({
  account,
  isSelected,
  isCopied,
  isDragging,
  isBumped,
  idx,
  onSelect,
  onContextMenu,
  onCopy,
  onGripDown,
  copyTitle,
}: {
  account: GameAccountDto;
  isSelected: boolean;
  isCopied: boolean;
  isDragging: boolean;
  isBumped: boolean;
  idx: number;
  onSelect: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onCopy: (e: React.MouseEvent) => void;
  onGripDown: (e: React.MouseEvent) => void;
  copyTitle: string;
}) {
  const initial = account.displayName.charAt(0).toUpperCase();
  return (
    <button
      data-acct-idx={idx}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      className={`group flex items-center gap-2.5 rounded-lg border px-3 py-2 text-left transition-all duration-150 ${
        isDragging ? "opacity-50" : ""
      } ${isBumped ? "animate-[dragBump_0.2s_ease]" : ""} ${
        isSelected
          ? "border-accent bg-[rgba(232,162,58,0.05)]"
          : "border-border bg-[var(--surface)] hover:bg-[var(--surface-hover)]"
      }`}
    >
      <span
        onMouseDown={onGripDown}
        className="shrink-0 cursor-grab rounded p-0.5 text-text-faint opacity-0 transition-opacity group-hover:opacity-50 hover:text-accent active:cursor-grabbing"
      >
        <svg width="8" height="10" viewBox="0 0 10 10" fill="currentColor">
          <circle cx="3" cy="2" r="1" />
          <circle cx="7" cy="2" r="1" />
          <circle cx="3" cy="5" r="1" />
          <circle cx="7" cy="5" r="1" />
          <circle cx="3" cy="8" r="1" />
          <circle cx="7" cy="8" r="1" />
        </svg>
      </span>
      <div
        className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full border-[1.5px] text-xs font-bold ${
          isSelected
            ? "border-accent text-accent"
            : "border-border text-text-dim group-hover:border-accent group-hover:text-accent"
        }`}
      >
        {initial}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-xs font-medium text-[var(--text)]">{account.displayName}</div>
        <div className="truncate font-mono text-[11px] text-text-faint">{account.id}</div>
      </div>
      <CopyIcon isCopied={isCopied} onClick={onCopy} title={copyTitle} position="" />
    </button>
  );
}

/* ---- Shared copy icon ---- */
function CopyIcon({
  isCopied,
  onClick,
  title,
  position,
}: {
  isCopied: boolean;
  onClick: (e: React.MouseEvent) => void;
  title: string;
  position: string;
}) {
  return (
    <span
      role="button"
      tabIndex={-1}
      onClick={onClick}
      title={title}
      className={`${position} rounded p-0.5 transition-all ${
        isCopied
          ? "text-green-400 opacity-100"
          : "text-text-faint opacity-0 group-hover:opacity-100 hover:text-accent"
      }`}
    >
      {isCopied ? (
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="20 6 9 17 4 12" />
        </svg>
      ) : (
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="9" y="9" width="13" height="13" rx="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
    </span>
  );
}
