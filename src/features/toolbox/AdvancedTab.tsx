import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { commands } from "../../lib/tauri";
import { Toggle } from "../../components/Toggle";
import { Section, Row, Dropdown } from "./ToolboxUi";

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

  const flip = (key: string, current: boolean) =>
    setConfig.mutate({ key, value: String(!current) });

  return (
    <div className="flex flex-col gap-4">
      {/* Launch */}
      <Section title={t("settings.section.launch")}>
        <Row label={t("settings.skip_play_confirm")}>
          <Toggle
            checked={config?.skipPlayConfirm ?? false}
            onChange={() => config && flip("skip_play_confirm", config.skipPlayConfirm)}
          />
        </Row>
        <Row label={t("settings.auto_launch_game")}>
          <Toggle
            checked={config?.autoLaunchGame ?? false}
            onChange={() => config && flip("auto_launch_game", config.autoLaunchGame)}
          />
        </Row>
        <Row label={t("settings.auto_kill_patcher")} hint={t("settings.auto_kill_patcher_desc")}>
          <Toggle
            checked={config?.autoKillPatcher ?? true}
            onChange={() => config && flip("auto_kill_patcher", config.autoKillPatcher)}
          />
        </Row>
        <Row label={t("settings.traditional_login")} hint={t("settings.traditional_login_desc")}>
          <Toggle
            checked={config?.traditionalLogin ?? false}
            onChange={() => config && flip("traditional_login", config.traditionalLogin)}
          />
        </Row>
      </Section>

      {/* Privacy */}
      <Section title={t("settings.section.privacy")}>
        <Row label={t("settings.hide_account_names")} hint={t("settings.hide_account_names_desc")}>
          <Toggle
            checked={config?.hideAccountNames ?? false}
            onChange={() => config && flip("hide_account_names", config.hideAccountNames)}
          />
        </Row>
        <Row label={t("settings.gamepass_incognito")}>
          <Toggle
            checked={config?.gamepassIncognito ?? true}
            onChange={() => config && flip("gamepass_incognito", config.gamepassIncognito)}
          />
        </Row>
      </Section>

      {/* Window */}
      <Section title={t("settings.section.window")}>
        <Row label={t("settings.close_behavior")} hint={t("settings.close_behavior_desc")}>
          <Dropdown
            value={config?.closeBehavior ?? "ask"}
            options={[
              { value: "ask", label: t("settings.close_ask") },
              { value: "quit", label: t("settings.close_quit") },
              { value: "tray", label: t("settings.close_tray") },
            ]}
            onChange={(v) => setConfig.mutate({ key: "close_behavior", value: v })}
          />
        </Row>
      </Section>

      {/* Debugging */}
      <Section title={t("settings.section.debug")}>
        <Row label={t("settings.debug_console")}>
          <Toggle
            checked={config?.debugLogging ?? false}
            onChange={() => {
              if (!config) return;
              const newVal = !config.debugLogging;
              setConfig.mutate({ key: "debug_logging", value: String(newVal) });
              commands.toggleDebugWindow(newVal).catch(() => {});
            }}
          />
        </Row>
      </Section>
    </div>
  );
}
