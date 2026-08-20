import { useEffect, useRef } from "react";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useConfigStore } from "../../lib/stores/config-store";

const MENU_BASE =
  "z-50 animate-[ctxIn_0.15s_ease] rounded-[10px] border border-border bg-[var(--tb-card)] shadow-[0_8px_32px_rgba(0,0,0,0.3)]";
const ITEM_BASE =
  "flex w-full items-center text-left text-[var(--text)] transition-colors hover:bg-[rgba(232,162,58,0.08)] hover:text-accent";

/** Menu / item classes; the compact launcher gets a tighter menu so it doesn't
 *  dwarf the little window it pops over. */
function useMenuClasses() {
  const compact = useConfigStore((s) => s.config?.compactUi ?? false);
  return {
    menu: `${MENU_BASE} ${compact ? "min-w-[140px] py-1" : "min-w-[160px] py-1.5"}`,
    item: `${ITEM_BASE} ${compact ? "gap-2 px-3 py-[5px] text-[11.5px]" : "gap-2.5 px-4 py-2 text-[12px]"}`,
  };
}

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
  alignLeft = false,
}: {
  t: (key: string) => string;
  region: string;
  onRefresh: () => void;
  onClose: () => void;
  sessionId: string;
  /** Anchor to the trigger's left edge (the compact layout has beans on the left). */
  alignLeft?: boolean;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, onClose);
  const cls = useMenuClasses();

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
    <div
      ref={menuRef}
      className={`absolute top-full mt-1 ${cls.menu} ${alignLeft ? "left-0" : "right-0"}`}
    >
      <button onClick={onRefresh} className={cls.item}>
        <span className="w-4 text-center text-xs">🔄</span>
        {t("launcher.beans_refresh")}
      </button>
      <button onClick={handleTopup} className={cls.item}>
        <span className="w-4 text-center text-xs">💳</span>
        {t("launcher.beans_topup")}
      </button>
      {region === "TW" && (
        <button onClick={handleExchange} className={cls.item}>
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
  const cls = useMenuClasses();

  return (
    <div ref={menuRef} className={`absolute top-full right-0 mt-1 ${cls.menu}`}>
      <button
        onClick={() => {
          commands.openMemberPopup(sessionId).catch(() => {});
          onClose();
        }}
        className={cls.item}
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
        className={cls.item}
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
            className={`${cls.item} hover:text-[var(--danger)]`}
          >
            <span className="w-4 text-center text-xs">⏏</span>
            {t("launcher.logout")}
          </button>
        </>
      )}
    </div>
  );
}

/** The ▾ next to "Get OTP" in the compact launcher. Opens upward — it sits at
 *  the bottom of a small window. */
export function OtpMoreMenu({
  items,
  onClose,
}: {
  items: { icon: string; label: string; onClick: () => void; disabled?: boolean }[];
  onClose: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, onClose);
  const cls = useMenuClasses();

  return (
    <div ref={menuRef} className={`absolute right-0 bottom-full mb-1 ${cls.menu}`}>
      {items.map((it) => (
        <button
          key={it.label}
          disabled={it.disabled}
          onClick={() => {
            onClose();
            it.onClick();
          }}
          className={`${cls.item} disabled:opacity-40`}
        >
          <span className="w-4 text-center text-xs">{it.icon}</span>
          {it.label}
        </button>
      ))}
    </div>
  );
}
