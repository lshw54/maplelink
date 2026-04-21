import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "../../lib/i18n";
import { open } from "@tauri-apps/plugin-shell";
import { commands } from "../../lib/tauri";
import { useRefreshAccounts } from "../../lib/hooks/use-accounts";
import { useQueryClient } from "@tanstack/react-query";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useConfigStore } from "../../lib/stores/config-store";
import { Modal } from "../shared/Modal";
import type { GameAccountDto } from "../../lib/types";

interface ContextMenuPosition {
  x: number;
  y: number;
}

interface AccountContextMenuProps {
  position: ContextMenuPosition | null;
  account: GameAccountDto | null;
  onClose: () => void;
}

type ModalView = { kind: "detail" } | { kind: "edit" } | { kind: "email"; email: string | null };

function MenuItem({
  onClick,
  icon,
  children,
}: {
  onClick: () => void;
  icon: string;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className="flex w-full items-center gap-2.5 px-4 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent"
    >
      <span className="w-4 text-center text-xs">{icon}</span>
      <span>{children}</span>
    </button>
  );
}

function Separator() {
  return <div className="mx-2.5 my-1 h-px bg-border" />;
}

function AccountDetailView({
  account,
  t,
}: {
  account: GameAccountDto;
  t: (key: string) => string;
}) {
  const hasDate = account.createdAt && account.createdAt.length > 0;
  const created = hasDate ? new Date(account.createdAt) : null;
  const [now] = useState(() => Date.now());
  const days = created ? Math.floor((now - created.getTime()) / 86_400_000) : null;
  const fmtDate = created
    ? `${created.getFullYear()}-${String(created.getMonth() + 1).padStart(2, "0")}-${String(created.getDate()).padStart(2, "0")}`
    : "—";
  const isNormal = account.status === "normal";
  const statusText = isNormal
    ? t("launcher.context.detail_normal")
    : t("launcher.context.detail_banned");

  return (
    <div className="flex flex-col gap-4">
      {/* Hero */}
      <div className="flex items-center gap-3.5">
        <div className="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-accent to-[#c47a1a] text-lg font-bold text-white shadow-[0_4px_16px_rgba(232,162,58,0.35)]">
          {(account.displayName || "?").charAt(0)}
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[15px] font-extrabold tracking-wide text-[var(--text)]">
            {account.displayName || "--"}
          </div>
          <div className="truncate font-mono text-[12px] text-[var(--text-faint)]">
            {account.id}
          </div>
        </div>
        <span
          className={`flex-shrink-0 rounded-full px-2.5 py-0.5 text-[12px] font-bold tracking-wider ${
            isNormal
              ? "bg-[rgba(74,222,128,0.1)] text-[var(--ok,#4ade80)]"
              : "bg-[rgba(239,68,68,0.1)] text-red-400"
          }`}
        >
          {statusText}
        </span>
      </div>

      {/* Stats grid */}
      <div className="grid grid-cols-3 gap-px overflow-hidden rounded-[10px] bg-[var(--tb-border)]">
        {[
          { label: t("launcher.context.detail_sn"), value: account.sn },
          { label: t("launcher.context.detail_tag"), value: `#${account.sn}` },
          { label: t("launcher.context.detail_created"), value: fmtDate },
        ].map((s) => (
          <div key={s.label} className="flex flex-col gap-0.5 bg-[var(--bg)] px-3 py-3">
            <span className="text-[12px] font-semibold tracking-[1.5px] text-[var(--text-faint)] uppercase">
              {s.label}
            </span>
            <span className="font-mono text-[12px] font-bold text-[var(--text)]">
              {s.value || "—"}
            </span>
          </div>
        ))}
      </div>

      {/* Days counter */}
      {days !== null && (
        <div className="relative flex flex-col items-center gap-0.5 py-2">
          <div className="pointer-events-none absolute h-[100px] w-[100px] rounded-full bg-[radial-gradient(circle,rgba(232,162,58,0.35),transparent_70%)] opacity-30" />
          <div className="relative text-[52px] leading-none font-black text-accent">{days}</div>
          <div className="text-[12px] font-semibold tracking-[2px] text-[var(--text-dim)] uppercase">
            {t("launcher.context.detail_days")}
          </div>
          <div className="mt-1.5 font-mono text-[12px] text-[var(--text-faint)]">
            since <span className="text-accent">{fmtDate}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function EditAccountView({
  account,
  t,
  region,
  onSave,
  onCancel,
}: {
  account: GameAccountDto;
  t: (key: string) => string;
  region: string;
  onSave: (name: string, syncToServer: boolean) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(account.displayName);
  const [syncToServer, setSyncToServer] = useState(false);
  const isTW = region === "TW";

  return (
    <div className="flex flex-col gap-4">
      <label className="text-[12px] text-[var(--text-dim)]">
        {t("launcher.context.edit_prompt")}
      </label>
      <input
        autoFocus
        name="edit-display-name"
        autoComplete="off"
        data-form-type="other"
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && name) onSave(name, syncToServer);
        }}
        className="rounded-lg border border-[var(--tb-border)] bg-[var(--bg)] px-3 py-2 text-xs text-[var(--text)] transition-colors outline-none focus:border-accent"
      />
      {isTW ? (
        <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-text-dim">
          <input
            type="checkbox"
            name="sync-to-server"
            checked={syncToServer}
            onChange={(e) => setSyncToServer(e.target.checked)}
            className="h-3.5 w-3.5 accent-accent"
          />
          {t("launcher.context.edit_sync")}
        </label>
      ) : (
        <span className="text-[11px] text-text-faint">{t("launcher.context.edit_local_only")}</span>
      )}
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="rounded-lg px-3 py-1.5 text-[12px] text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,255,255,0.05)]"
        >
          {t("common.cancel")}
        </button>
        <button
          onClick={() => name && onSave(name, syncToServer)}
          className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
        >
          {t("common.ok")}
        </button>
      </div>
    </div>
  );
}

function EmailResultView({ email, t }: { email: string | null; t: (key: string) => string }) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    if (!email) return;
    navigator.clipboard.writeText(email);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  if (!email) {
    return (
      <div className="text-center text-xs text-[var(--text-dim)]">
        {t("launcher.context.email_unavailable")}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="text-center text-xs text-[var(--text-dim)]">
        {t("launcher.context.email_result")}
      </div>
      <div className="truncate rounded-lg bg-[var(--bg)] px-3 py-2 text-center font-mono text-xs text-[var(--text)]">
        {email}
      </div>
      <button
        onClick={handleCopy}
        className="self-center rounded-lg bg-accent px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
      >
        {copied ? t("common.copied") : t("common.copy")}
      </button>
    </div>
  );
}

export function AccountContextMenu({ position, account, onClose }: AccountContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);
  const refreshAccounts = useRefreshAccounts();
  const queryClient = useQueryClient();
  const region = useConfigStore((s) => s.config?.region ?? "HK");
  const [modalView, setModalView] = useState<ModalView | null>(null);
  const [editError, setEditError] = useState(false);
  const modalViewRef = useRef<ModalView | null>(null);
  const [clampedPos, setClampedPos] = useState<{ x: number; y: number } | null>(null);

  // Clamp menu position to stay within window bounds.
  // Uses rAF to ensure the menu is fully painted before measuring.
  useEffect(() => {
    if (!position) return;

    const raf = requestAnimationFrame(() => {
      const el = menuRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const pad = 8;
      const menuW = rect.width || 170;
      const menuH = rect.height || 300;
      const x = Math.min(position.x, window.innerWidth - menuW - pad);
      const y = Math.min(position.y, window.innerHeight - menuH - pad);
      setClampedPos({ x: Math.max(pad, x), y: Math.max(pad, y) });
    });

    return () => cancelAnimationFrame(raf);
  }, [position]);

  // Keep ref in sync
  useEffect(() => {
    modalViewRef.current = modalView;
  }, [modalView]);

  useEffect(() => {
    if (!position) return;

    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }

    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (modalViewRef.current) {
          // Close modal AND context menu so it doesn't re-appear
          setModalView(null);
          setEditError(false);
        }
        onClose();
      }
    }

    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("contextmenu", handleClickOutside);
      document.addEventListener("keydown", handleEscape, true);
    }, 16);

    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("contextmenu", handleClickOutside);
      document.removeEventListener("keydown", handleEscape, true);
    };
  }, [position, onClose]);

  if (!position && !modalView) return null;
  if (!account) return null;

  function closeModal() {
    setModalView(null);
    setEditError(false);
    onClose();
  }

  // Close just the modal, keeping context menu hidden.
  // Since context menu only shows when `position && !modalView`,
  // we need to also clear position to prevent it from re-appearing.
  function closeModalOnly() {
    setModalView(null);
    setEditError(false);
    onClose();
  }

  function handleCopyAccount() {
    if (account) navigator.clipboard.writeText(account.id);
    onClose();
  }

  async function handleCopyCredentials() {
    if (!account) return;
    try {
      const creds = await commands.getGameCredentials(
        useAuthStore.getState().activeSessionId ?? "",
        account.id,
      );
      await navigator.clipboard.writeText(`${creds.accountId}\n${creds.otp}`);
    } catch {
      /* ignore */
    }
    onClose();
  }

  function handleEditAccount() {
    setModalView({ kind: "edit" });
  }

  async function handleEditSave(newName: string, syncToServer: boolean) {
    if (!account || newName === account.displayName) {
      closeModal();
      return;
    }

    let serverUpdated = false;
    if (syncToServer) {
      try {
        serverUpdated = await commands.changeAccountDisplayName(
          useAuthStore.getState().activeSessionId ?? "",
          account.id,
          newName,
        );
      } catch {
        // API failed
      }
      if (!serverUpdated) {
        setEditError(true);
        return;
      }
    }

    if (serverUpdated) {
      // Server updated — refresh from server
      refreshAccounts();
    } else {
      // Local-only update — save to display_overrides.json, then refresh
      await commands.setDisplayOverride(account.id, newName);
      // Invalidate query so useGameAccounts re-fetches + merges overrides
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
    }
    closeModal();
  }

  function handleAccountDetail() {
    setModalView({ kind: "detail" });
  }

  function handleMemberCenter() {
    commands.openMemberPopup(useAuthStore.getState().activeSessionId ?? "").catch(() => {});
    onClose();
  }

  function handleSupport() {
    commands.openCustomerService().catch(() => {});
    onClose();
  }

  function handleWebsite() {
    open("https://maplestory.beanfun.com/");
    onClose();
  }

  async function handleCheckEmail() {
    try {
      const email = await commands.getAuthEmail(useAuthStore.getState().activeSessionId ?? "");
      setModalView({ kind: "email", email: email || null });
    } catch {
      setModalView({ kind: "email", email: null });
    }
  }

  const modalTitle =
    modalView?.kind === "detail"
      ? t("launcher.context.detail")
      : modalView?.kind === "edit"
        ? t("launcher.context.edit_account")
        : modalView?.kind === "email"
          ? t("launcher.context.email")
          : undefined;

  return (
    <>
      {position && !modalView && (
        <div
          ref={menuRef}
          role="menu"
          className="fixed z-50 min-w-[170px] animate-[ctxIn_0.15s_ease] rounded-[10px] border border-border bg-[var(--surface)] py-1.5 shadow-[0_8px_32px_rgba(0,0,0,0.3)] backdrop-blur-[20px]"
          style={
            clampedPos ? { left: clampedPos.x, top: clampedPos.y } : { left: -9999, top: -9999 }
          }
        >
          <MenuItem icon="📋" onClick={handleCopyAccount}>
            {t("launcher.context.copy_account")}
          </MenuItem>
          <MenuItem icon="🔑" onClick={handleCopyCredentials}>
            {t("launcher.context.copy_credentials")}
          </MenuItem>
          <MenuItem icon="✏" onClick={handleEditAccount}>
            {t("launcher.context.edit_account")}
          </MenuItem>
          <MenuItem icon="📄" onClick={handleAccountDetail}>
            {t("launcher.context.detail")}
          </MenuItem>
          <Separator />
          <MenuItem icon="👤" onClick={handleMemberCenter}>
            {t("launcher.context.member")}
          </MenuItem>
          <MenuItem icon="🎧" onClick={handleSupport}>
            {t("launcher.context.support")}
          </MenuItem>
          <MenuItem icon="📧" onClick={handleCheckEmail}>
            {t("launcher.context.email")}
          </MenuItem>
          <Separator />
          <MenuItem icon="🌐" onClick={handleWebsite}>
            {t("launcher.context.website")}
          </MenuItem>
        </div>
      )}

      <Modal isOpen={modalView !== null} onClose={closeModalOnly} title={modalTitle}>
        {modalView?.kind === "detail" && <AccountDetailView account={account} t={t} />}
        {modalView?.kind === "edit" && (
          <>
            <EditAccountView
              account={account}
              t={t}
              region={region}
              onSave={handleEditSave}
              onCancel={closeModal}
            />
            {editError && (
              <div className="mt-2 text-center text-[12px] text-red-400">
                {t("launcher.context.edit_failed")}
              </div>
            )}
          </>
        )}
        {modalView?.kind === "email" && <EmailResultView email={modalView.email} t={t} />}
      </Modal>
    </>
  );
}
