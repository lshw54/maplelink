import { useEffect } from "react";
import { useErrorToastStore, type Toast } from "../../lib/stores/error-toast-store";

const AUTO_DISMISS_MS = 5000;

export function ErrorToastContainer() {
  const toasts = useErrorToastStore((s) => s.toasts);

  return (
    <div className="pointer-events-none fixed right-3 bottom-8 z-50 flex flex-col gap-2">
      {toasts.map((toast) => (
        <ErrorToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  );
}

function ErrorToastItem({ toast }: { toast: Toast }) {
  const removeToast = useErrorToastStore((s) => s.removeToast);

  // Auto-dismiss non-critical toasts after 5 seconds
  useEffect(() => {
    if (toast.critical) return;
    const timer = setTimeout(() => removeToast(toast.id), AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [toast.id, toast.critical, removeToast]);

  return (
    <div
      role="alert"
      className="pointer-events-auto flex max-w-xs items-start gap-2 rounded-[var(--radius)] border border-[var(--danger)] bg-[var(--bg)] px-3 py-2.5 shadow-lg"
    >
      {/* Error icon */}
      <svg
        className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--danger)]"
        viewBox="0 0 16 16"
        fill="currentColor"
      >
        <path d="M8 1a7 7 0 100 14A7 7 0 008 1zm-.75 4a.75.75 0 011.5 0v3.5a.75.75 0 01-1.5 0V5zm.75 6.25a.75.75 0 110-1.5.75.75 0 010 1.5z" />
      </svg>

      <span className="flex-1 text-xs text-[var(--text)]">{toast.message}</span>

      <button
        onClick={() => removeToast(toast.id)}
        aria-label="Dismiss"
        className="shrink-0 text-text-dim transition-colors hover:text-[var(--text)]"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
        >
          <line x1="0" y1="0" x2="10" y2="10" />
          <line x1="10" y1="0" x2="0" y2="10" />
        </svg>
      </button>
    </div>
  );
}
