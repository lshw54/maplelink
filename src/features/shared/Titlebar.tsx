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
  const classicMode = useUiStore((s) => s.classicMode);
  const config = useConfigStore((s) => s.config);
  const setConfig = useSetConfig();

  const region = config?.region ?? "HK";
  const regionFlag = region === "TW" ? "🇹🇼" : "🇭🇰";

  function handleDragStart(e: React.MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    appWindow.startDragging();
  }

  // Classic (懷舊服) is a distinct mode, kept separate from the HK/TW region
  // toggle. Enabling it forces the region to HK (phase 1 is HK id-pass only).
  function toggleClassic() {
    if (classicMode) {
      useUiStore.setState({ classicMode: false });
    } else {
      useUiStore.setState({ classicMode: true });
      if (region !== "HK") setConfig.mutate({ key: "region", value: "HK" });
    }
  }

  // Choosing a region is a regular-login action, so it also leaves classic mode.
  function handleRegionToggle() {
    if (classicMode) useUiStore.setState({ classicMode: false });
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
      {/* App name */}
      <div className="pointer-events-none flex flex-1 items-center pl-4 text-[11px] font-bold tracking-[3px] text-text-dim uppercase">
        MAPLELINK
      </div>

      {/* Actions — no-drag */}
      <div
        className="flex items-center"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        {/* Classic (懷舊服) toggle + region toggle — same style, side by side */}
        {currentPage === "login" ? (
          <>
            <button
              onClick={toggleClassic}
              title={t("login.mode_classic")}
              className={`relative flex h-[34px] w-[34px] items-center justify-center text-[13px] transition-all hover:bg-[var(--surface-hover)] active:scale-[0.92] ${
                classicMode ? "text-accent" : "text-text-dim hover:text-accent"
              }`}
            >
              🍁
              {classicMode && (
                <span className="absolute bottom-[5px] left-1/2 h-0.5 w-3 -translate-x-1/2 rounded-sm bg-accent opacity-60" />
              )}
            </button>
            <button
              onClick={handleRegionToggle}
              title={t("shared.titlebar.region_toggle")}
              className={`relative flex h-[34px] w-[34px] items-center justify-center text-[12px] transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-[0.92] ${
                classicMode ? "text-text-faint" : "text-text-dim"
              }`}
            >
              {regionFlag}
              {!classicMode && (
                <span className="absolute bottom-[5px] left-1/2 h-0.5 w-3 -translate-x-1/2 rounded-sm bg-accent opacity-60" />
              )}
            </button>
          </>
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
