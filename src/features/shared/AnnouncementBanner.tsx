import { useTranslation } from "../../lib/i18n";

/**
 * Permanent announcement banner (always shown, ~28px — its height is baked into
 * the window size in `resize_window`). It cannot be closed/hidden; it's the
 * persistent entry point to reopen the announcement. Clicking it opens the
 * announcement overlay.
 */
export function AnnouncementBanner({
  onOpen,
  onClose,
}: {
  onOpen: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex h-[28px] shrink-0 items-center gap-2 border-b border-[rgba(var(--accent-rgb),0.2)] bg-[rgba(var(--accent-rgb),0.1)] pr-2 pl-3">
      <button
        onClick={onOpen}
        className="flex min-w-0 flex-1 items-center gap-2 text-left transition-opacity hover:opacity-80"
      >
        <span className="shrink-0 text-[12px]">📢</span>
        <span className="min-w-0 flex-1 truncate text-[11px] font-semibold text-accent">
          {t("announcement.title")}
        </span>
        <span className="shrink-0 text-[11px] font-semibold text-accent/80">
          {t("announcement.reopen")} ›
        </span>
      </button>
      <button
        onClick={onClose}
        title={t("announcement.hide_banner")}
        aria-label={t("announcement.hide_banner")}
        className="shrink-0 px-1 text-[13px] leading-none text-accent/60 transition-colors hover:text-accent"
      >
        ×
      </button>
    </div>
  );
}
