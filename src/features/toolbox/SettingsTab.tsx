import { useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { useUiStore } from "../../lib/stores/ui-store";
import { commands } from "../../lib/tauri";
import type { ThemeMode, Language } from "../../lib/stores/ui-store";

const THEMES: { value: ThemeMode; labelKey: string }[] = [
  { value: "system", labelKey: "settings.theme.system" },
  { value: "dark", labelKey: "settings.theme.dark" },
  { value: "light", labelKey: "settings.theme.light" },
];

const LANGUAGES: { value: Language; label: string }[] = [
  { value: "en-US", label: "English" },
  { value: "zh-TW", label: "繁體中文" },
  { value: "zh-CN", label: "简体中文" },
];

type UpdateChannel = "release" | "pre-release";

const UPDATE_CHANNELS: { value: UpdateChannel; labelKey: string }[] = [
  { value: "release", labelKey: "settings.update_channel.release" },
  { value: "pre-release", labelKey: "settings.update_channel.pre_release" },
];

export function SettingsTab() {
  const { t } = useTranslation();
  const config = useConfigStore((s) => s.config);
  const setTheme = useUiStore((s) => s.setTheme);
  const setLanguage = useUiStore((s) => s.setLanguage);
  const setConfig = useSetConfig();

  // Auto-detect game path from registry if not set
  useEffect(() => {
    if (!config?.gamePath) {
      commands
        .detectGamePath()
        .then((path) => {
          if (path) {
            setConfig.mutate({ key: "gamePath", value: path });
          }
        })
        .catch(() => {});
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function handleBrowseGamePath() {
    const path = await commands.openFileDialog();
    if (path) {
      setConfig.mutate({ key: "gamePath", value: path });
    }
  }

  function handleThemeChange(theme: ThemeMode) {
    setTheme(theme);
    setConfig.mutate({ key: "theme", value: theme });
  }

  function handleLanguageChange(lang: Language) {
    setLanguage(lang);
    setConfig.mutate({ key: "language", value: lang });
  }

  function handleToggleAutoUpdate() {
    if (!config) return;
    setConfig.mutate({
      key: "autoUpdate",
      value: String(!config.autoUpdate),
    });
  }

  function handleUpdateChannelChange(channel: UpdateChannel) {
    useConfigStore.getState().updateConfigField("updateChannel", channel);
    setConfig.mutate({ key: "updateChannel", value: channel });
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Game path */}
      <SettingRow label={t("settings.game_path")}>
        <div className="flex items-center gap-2">
          <span className="max-w-[280px] truncate text-xs text-[var(--text)]">
            {config?.gamePath || "—"}
          </span>
          <button
            onClick={handleBrowseGamePath}
            className="shrink-0 rounded-[var(--radius)] border border-border px-3 py-1 text-xs text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
          >
            {t("settings.browse")}
          </button>
        </div>
      </SettingRow>

      {/* Theme picker — segmented control */}
      <SettingRow label={t("settings.theme")}>
        <div className="flex overflow-hidden rounded-lg border border-[var(--tb-border)]">
          {THEMES.map((theme, i) => (
            <button
              key={theme.value}
              onClick={() => handleThemeChange(theme.value)}
              className={`px-3.5 py-1.5 text-[12px] font-semibold tracking-[0.5px] outline-none transition-all active:scale-95 ${
                i < THEMES.length - 1 ? "border-r border-[var(--tb-border)]" : ""
              } ${
                config?.theme === theme.value
                  ? "bg-gradient-to-br from-accent to-[#c47a1a] text-white shadow-[0_2px_8px_var(--accent-glow)]"
                  : "bg-[var(--tb-card)] text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
              }`}
            >
              {t(theme.labelKey)}
            </button>
          ))}
        </div>
      </SettingRow>

      {/* Language picker — segmented control */}
      <SettingRow label={t("settings.language")}>
        <div className="flex overflow-hidden rounded-lg border border-[var(--tb-border)]">
          {LANGUAGES.map((lang, i) => (
            <button
              key={lang.value}
              onClick={() => handleLanguageChange(lang.value)}
              className={`px-3.5 py-1.5 text-[12px] font-semibold tracking-[0.5px] outline-none transition-all active:scale-95 ${
                i < LANGUAGES.length - 1 ? "border-r border-[var(--tb-border)]" : ""
              } ${
                config?.language === lang.value
                  ? "bg-gradient-to-br from-accent to-[#c47a1a] text-white shadow-[0_2px_8px_var(--accent-glow)]"
                  : "bg-[var(--tb-card)] text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
              }`}
            >
              {lang.label}
            </button>
          ))}
        </div>
      </SettingRow>

      {/* Auto-update toggle */}
      <SettingRow label={t("settings.auto_update")}>
        <Toggle checked={config?.autoUpdate ?? true} onChange={handleToggleAutoUpdate} />
      </SettingRow>

      {/* Update channel */}
      <SettingRow label={t("settings.update_channel")}>
        <div className="flex overflow-hidden rounded-lg border border-[var(--tb-border)]">
          {UPDATE_CHANNELS.map((ch, i) => (
            <button
              key={ch.value}
              onClick={() => handleUpdateChannelChange(ch.value)}
              className={`px-3.5 py-1.5 text-[12px] font-semibold tracking-[0.5px] outline-none transition-colors ${
                i < UPDATE_CHANNELS.length - 1 ? "border-r border-[var(--tb-border)]" : ""
              } ${
                (config?.updateChannel ?? "release") === ch.value
                  ? "bg-gradient-to-br from-accent to-[#c47a1a] text-white shadow-[0_2px_8px_var(--accent-glow)]"
                  : "bg-[var(--tb-card)] text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
              }`}
            >
              {t(ch.labelKey)}
            </button>
          ))}
        </div>
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
    </div>
  );
}

function SettingRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3 transition-all hover:translate-y-[-1px] hover:border-[var(--tb-border)]">
      <span className="text-xs font-semibold text-[var(--text)]">{label}</span>
      {children}
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      onClick={onChange}
      role="switch"
      aria-checked={checked}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
        checked ? "bg-accent" : "bg-[var(--border)]"
      }`}
    >
      <span
        className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
          checked ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </button>
  );
}
