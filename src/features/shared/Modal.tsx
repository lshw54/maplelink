import { useEffect, useCallback, type ReactNode } from "react";

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: ReactNode;
}

export function Modal({ isOpen, onClose, title, children }: ModalProps) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (!isOpen) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, handleKeyDown]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/45 backdrop-blur-[6px]"
      onMouseDown={onClose}
    >
      <div
        className="w-[340px] rounded-[14px] border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_12px_40px_rgba(0,0,0,0.3)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Header with close button */}
        <div className="flex items-center border-b border-[var(--tb-border)] px-5 py-3">
          {title && <span className="flex-1 text-sm font-bold text-[var(--text)]">{title}</span>}
          <button
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded text-sm text-[var(--text-dim)] transition-colors hover:bg-[var(--danger)] hover:text-white"
          >
            ×
          </button>
        </div>
        <div className="p-5">{children}</div>
      </div>
    </div>
  );
}
