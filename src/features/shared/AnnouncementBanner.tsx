import { useTranslation } from "../../lib/i18n";

/**
 * Permanent announcement banner (always shown, ~28px — its height is baked into
 * the window size in `resize_window`). It cannot be closed/hidden; it's the
 * persistent entry point to reopen the announcement. Clicking it opens the
 * announcement overlay.
 */
export function AnnouncementBanner({ onOpen }: { onOpen: () => void }) {
  const { t } = useTranslation();
  return (
    <button
      onClick={onOpen}
      className="flex h-[28px] shrink-0 items-center gap-2 border-b border-[rgba(232,162,58,0.2)] bg-[rgba(232,162,58,0.1)] px-3 text-left transition-colors hover:bg-[rgba(232,162,58,0.16)]"
    >
      <span className="shrink-0 text-[12px]">📢</span>
      <span className="flex-1 truncate text-[11px] font-semibold text-accent">
        {t("announcement.title")}
      </span>
      <span className="shrink-0 text-[11px] font-semibold text-accent/80">
        {t("announcement.reopen")} ›
      </span>
    </button>
  );
}
