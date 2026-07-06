import { useTranslation } from "../../lib/i18n";
import { useUiStore } from "../../lib/stores/ui-store";
import { WebLaunchTab } from "./WebLaunchTab";

/** Standalone one-click web-launch page (reached from the login page). */
export function WebLaunchPage() {
  const { t } = useTranslation();
  const goBack = useUiStore((s) => s.goBack);

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[var(--tb-bg)]">
      {/* Header */}
      <div className="flex shrink-0 items-center gap-2 border-b border-[var(--tb-border)] px-4 py-3">
        <span className="text-base">🌐</span>
        <span className="flex-1 text-[13px] font-semibold text-[var(--text)]">
          {t("web_launch.title")}
        </span>
        <button
          onClick={() => goBack()}
          className="rounded-lg border border-[var(--tb-border)] bg-transparent px-3 py-1.5 text-[12px] font-semibold tracking-[1px] text-text-dim uppercase transition-all hover:border-accent hover:text-accent active:scale-95"
        >
          {t("shared.titlebar.back")}
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        <WebLaunchTab />
      </div>
    </div>
  );
}
