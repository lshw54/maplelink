import { useEffect, useRef } from "react";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";

const MENU_CLASS =
  "absolute top-full right-0 z-50 mt-1 min-w-[160px] animate-[ctxIn_0.15s_ease] rounded-[10px] border border-border bg-[var(--surface)] py-1.5 shadow-[0_8px_32px_rgba(0,0,0,0.3)] backdrop-blur-[20px]";
const ITEM_CLASS =
  "flex w-full items-center gap-2.5 px-4 py-2 text-left text-[12px] text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent";

/** Close the menu on any mousedown outside it. Registered a tick late so the
 *  click that opened the menu doesn't immediately close it. */
function useClickOutside(ref: React.RefObject<HTMLElement | null>, onClose: () => void) {
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    }
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
    }, 16);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [ref, onClose]);
}

export function BeansPopupMenu({
  t,
  region,
  onRefresh,
  onClose,
  sessionId,
}: {
  t: (key: string) => string;
  region: string;
  onRefresh: () => void;
  onClose: () => void;
  sessionId: string;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, onClose);

  async function handleTopup() {
    try {
      await commands.openGashPopup(sessionId);
    } catch {
      /* ignore */
    }
    onClose();
  }

  async function handleExchange() {
    try {
      await commands.openAuthPopup(
        sessionId,
        "https://m.beanfun.com/Deposite",
        t("launcher.beans_exchange"),
      );
    } catch {
      /* ignore */
    }
    onClose();
  }

  return (
    <div ref={menuRef} className={MENU_CLASS}>
      <button onClick={onRefresh} className={ITEM_CLASS}>
        <span className="w-4 text-center text-xs">🔄</span>
        {t("launcher.beans_refresh")}
      </button>
      <button onClick={handleTopup} className={ITEM_CLASS}>
        <span className="w-4 text-center text-xs">💳</span>
        {t("launcher.beans_topup")}
      </button>
      {region === "TW" && (
        <button onClick={handleExchange} className={ITEM_CLASS}>
          <span className="w-4 text-center text-xs">🎁</span>
          {t("launcher.beans_exchange")}
        </button>
      )}
    </div>
  );
}

export function MorePopupMenu({
  t,
  sessionId,
  onClose,
  onLogout,
}: {
  t: (key: string) => string;
  sessionId: string;
  onClose: () => void;
  /** Present in the compact layout, where there is no room for a logout button. */
  onLogout?: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, onClose);

  return (
    <div ref={menuRef} className={MENU_CLASS}>
      <button
        onClick={() => {
          commands.openMemberPopup(sessionId).catch(() => {});
          onClose();
        }}
        className={ITEM_CLASS}
      >
        <span className="w-4 text-center text-xs">👤</span>
        {t("launcher.member_center")}
      </button>
      <button
        onClick={() => {
          commands
            .openCustomerService(useAuthStore.getState().activeSessionId ?? "")
            .catch(() => {});
          onClose();
        }}
        className={ITEM_CLASS}
      >
        <span className="w-4 text-center text-xs">💬</span>
        {t("launcher.support")}
      </button>
      {onLogout && (
        <>
          <div className="mx-3 my-1 border-t border-border" />
          <button
            onClick={() => {
              onClose();
              onLogout();
            }}
            className={`${ITEM_CLASS} hover:text-[var(--danger)]`}
          >
            <span className="w-4 text-center text-xs">⏏</span>
            {t("launcher.logout")}
          </button>
        </>
      )}
    </div>
  );
}
