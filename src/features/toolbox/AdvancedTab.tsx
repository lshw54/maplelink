import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { commands } from "../../lib/tauri";

export function AdvancedTab() {
  const { t } = useTranslation();
  const config = useConfigStore((s) => s.config);
  const setConfig = useSetConfig();

  // Sync toggle when debug window is closed via its own × button
  useEffect(() => {
    const unlisten = listen("debug-window-closed", () => {
      useConfigStore.getState().updateConfigField("debugLogging", false);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="flex flex-col gap-3">
      {/* Debug console */}
      <SettingRow label={t("settings.debug_console")}>
        <Toggle
          checked={config?.debugLogging ?? false}
          onChange={() => {
            if (!config) return;
            const newVal = !config.debugLogging;
            setConfig.mutate({ key: "debug_logging", value: String(newVal) });
            commands.toggleDebugWindow(newVal).catch(() => {});
          }}
        />
      </SettingRow>
      {/* GamePass incognito mode */}
      <SettingRow label={t("settings.gamepass_incognito")}>
        <Toggle
          checked={config?.gamepassIncognito ?? true}
          onChange={() => {
            if (!config) return;
            setConfig.mutate({
              key: "gamepass_incognito",
              value: String(!config.gamepassIncognito),
            });
          }}
        />
      </SettingRow>

      {/* Skip play confirmation */}
      <SettingRow label={t("settings.skip_play_confirm")}>
        <Toggle
          checked={config?.skipPlayConfirm ?? false}
          onChange={() => {
            if (!config) return;
            setConfig.mutate({
              key: "skip_play_confirm",
              value: String(!config.skipPlayConfirm),
            });
          }}
        />
      </SettingRow>

      {/* Auto-launch game after login */}
      <SettingRow label={t("settings.auto_launch_game")}>
        <Toggle
          checked={config?.autoLaunchGame ?? false}
          onChange={() => {
            if (!config) return;
            setConfig.mutate({
              key: "auto_launch_game",
              value: String(!config.autoLaunchGame),
            });
          }}
        />
      </SettingRow>

      {/* Auto-kill Patcher.exe */}
      <SettingRow label={t("settings.auto_kill_patcher")}>
        <Toggle
          checked={config?.autoKillPatcher ?? true}
          onChange={() => {
            if (!config) return;
            setConfig.mutate({
              key: "auto_kill_patcher",
              value: String(!config.autoKillPatcher),
            });
          }}
        />
      </SettingRow>
      <p className="px-1 text-[11px] leading-relaxed text-text-faint">
        {t("settings.auto_kill_patcher_desc")}
      </p>

      {/* Traditional login mode */}
      <SettingRow label={t("settings.traditional_login")}>
        <Toggle
          checked={config?.traditionalLogin ?? false}
          onChange={() => {
            if (!config) return;
            setConfig.mutate({
              key: "traditional_login",
              value: String(!config.traditionalLogin),
            });
          }}
        />
      </SettingRow>
      <p className="px-1 text-[11px] leading-relaxed text-text-faint">
        {t("settings.traditional_login_desc")}
      </p>
    </div>
  );
}

function SettingRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3 transition-all hover:translate-y-[-1px]">
      <span className="text-xs font-semibold text-[var(--text)]">{label}</span>
      {children}
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={onChange}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
        checked ? "bg-accent" : "bg-[var(--border)]"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
          checked ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </button>
  );
}
