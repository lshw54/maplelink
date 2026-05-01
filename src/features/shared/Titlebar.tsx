import { useTranslation } from "../../lib/i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUiStore } from "../../lib/stores/ui-store";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";

export function Titlebar() {
  const { t } = useTranslation();
  const appWindow = getCurrentWindow();
  const currentPage = useUiStore((s) => s.currentPage);
  const setPage = useUiStore((s) => s.setPage);
  const config = useConfigStore((s) => s.config);
  const setConfig = useSetConfig();

  const region = config?.region ?? "HK";
  const regionFlag = region === "TW" ? "🇹🇼" : "🇭🇰";

  function handleDragStart(e: React.MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    appWindow.startDragging();
  }

  function handleRegionToggle() {
    const next = region === "TW" ? "HK" : "TW";
    setConfig.mutate({ key: "region", value: next });
  }

  function handleToolbox() {
    if (currentPage !== "toolbox") {
      setPage("toolbox");
    }
  }

  return (
    <div
      data-tauri-drag-region
      onMouseDown={handleDragStart}
      className="flex h-[34px] shrink-0 items-center"
      style={{ zIndex: 10, position: "relative" }}
    >
      {/* Drag region — app name */}
      <div className="pointer-events-none flex flex-1 items-center pl-4 text-[11px] font-bold tracking-[3px] text-text-dim uppercase">
        MAPLELINK
      </div>

      {/* Actions — no-drag */}
      <div
        className="flex items-center"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        {/* Region: clickable on login, read-only on main/toolbox */}
        {currentPage === "login" ? (
          <button
            onClick={handleRegionToggle}
            title={t("shared.titlebar.region_toggle")}
            className="relative flex h-[34px] w-[34px] items-center justify-center text-[12px] text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-[0.92]"
          >
            {regionFlag}
            <span className="absolute bottom-[5px] left-1/2 h-0.5 w-3 -translate-x-1/2 rounded-sm bg-accent opacity-60" />
          </button>
        ) : (
          <span className="flex h-[34px] w-[34px] items-center justify-center text-[12px] text-text-faint">
            {region}
          </span>
        )}

        {/* Toolbox */}
        <button
          onClick={handleToolbox}
          title={t("shared.titlebar.toolbox")}
          className="flex h-[34px] w-[34px] items-center justify-center text-[12px] text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-[0.92]"
        >
          🛠
        </button>

        {/* Minimize */}
        <button
          onClick={() => appWindow.minimize()}
          aria-label={t("shared.titlebar.minimize")}
          className="flex h-[34px] w-[34px] items-center justify-center text-[14px] text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
        >
          −
        </button>

        {/* Close */}
        <button
          onClick={() => appWindow.close()}
          aria-label={t("shared.titlebar.close")}
          className="flex h-[34px] w-[34px] items-center justify-center rounded-tr-[var(--radius)] text-[16px] text-text-dim transition-all hover:bg-[var(--danger)] hover:text-white"
        >
          ×
        </button>
      </div>
    </div>
  );
}
