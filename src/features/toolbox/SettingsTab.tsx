import { useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { useUiStore, announcementBarShown } from "../../lib/stores/ui-store";
import { commands } from "../../lib/tauri";
import { Toggle } from "../../components/Toggle";
import { Section, Row, RowButton, RowValue, Segmented } from "./ToolboxUi";
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

type DefaultLoginView = "normal" | "qr";

const DEFAULT_LOGIN_VIEWS: { value: DefaultLoginView; labelKey: string }[] = [
  { value: "normal", labelKey: "settings.default_login_view.normal" },
  { value: "qr", labelKey: "settings.default_login_view.qr" },
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

  async function handleBrowseNgmPath() {
    const path = await commands.openFileDialog();
    if (path) {
      setConfig.mutate({ key: "classicNgmPath", value: path });
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

  function handleDefaultLoginViewChange(view: DefaultLoginView) {
    useConfigStore.getState().updateConfigField("defaultLoginView", view);
    setConfig.mutate({ key: "defaultLoginView", value: view });
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Game */}
      <Section title={t("settings.section.game")}>
        <Row label={t("settings.game_path")}>
          <RowValue mono>{config?.gamePath || "—"}</RowValue>
          <RowButton onClick={handleBrowseGamePath}>{t("settings.browse")}</RowButton>
        </Row>
        <Row label={t("settings.classic_ngm_path")}>
          <RowValue mono>{config?.classicNgmPath || t("settings.classic_ngm_auto")}</RowValue>
          <RowButton onClick={handleBrowseNgmPath}>{t("settings.browse")}</RowButton>
          {config?.classicNgmPath && (
            <RowButton
              danger
              title={t("common.close")}
              onClick={() => setConfig.mutate({ key: "classicNgmPath", value: "" })}
            >
              ✕
            </RowButton>
          )}
        </Row>
      </Section>

      {/* Appearance */}
      <Section title={t("settings.section.appearance")}>
        <Row label={t("settings.theme")}>
          <Segmented
            options={THEMES.map((th) => ({ value: th.value, label: t(th.labelKey) }))}
            value={config?.theme ?? "system"}
            onChange={handleThemeChange}
          />
        </Row>
        <Row label={t("settings.language")}>
          <Segmented
            options={LANGUAGES}
            value={config?.language ?? "zh-TW"}
            onChange={handleLanguageChange}
          />
        </Row>
        <Row label={t("settings.compact_ui")} hint={t("settings.compact_ui_desc")}>
          <Toggle
            checked={config?.compactUi ?? false}
            onChange={async () => {
              if (!config) return;
              useConfigStore.getState().updateConfigField("compactUi", !config.compactUi);
              await setConfig
                .mutateAsync({ key: "compactUi", value: String(!config.compactUi) })
                .catch(() => {});
              // This page is open right now — take the new size at once.
              commands.resizeWindow("toolbox", announcementBarShown()).catch(() => {});
            }}
          />
        </Row>
      </Section>

      {/* Updates */}
      <Section title={t("settings.section.updates")}>
        <Row label={t("settings.auto_update")}>
          <Toggle checked={config?.autoUpdate ?? true} onChange={handleToggleAutoUpdate} />
        </Row>
        <Row label={t("settings.update_channel")}>
          <Segmented
            options={UPDATE_CHANNELS.map((ch) => ({ value: ch.value, label: t(ch.labelKey) }))}
            value={config?.updateChannel ?? "release"}
            onChange={handleUpdateChannelChange}
          />
        </Row>
        {/* GitHub hosts override — only consulted when a direct connection to
            GitHub fails, i.e. in practice only for mainland-China users. */}
        <Row label={t("settings.github_hosts")} hint={t("settings.github_hosts_desc")}>
          <Toggle
            checked={config?.githubHosts ?? true}
            onChange={() => {
              if (!config) return;
              setConfig.mutate({ key: "githubHosts", value: String(!config.githubHosts) });
            }}
          />
        </Row>
      </Section>

      {/* Connection */}
      <Section title={t("settings.section.network")}>
        {/* Route webview traffic through this process — for accelerator users,
            switched on by itself when the IP says mainland China. */}
        <Row label={t("settings.webview_via_proxy")} hint={t("settings.webview_via_proxy_desc")}>
          <Toggle
            checked={config?.webviewViaProxy ?? false}
            onChange={() => {
              if (!config) return;
              setConfig.mutate({
                key: "webview_via_proxy",
                value: String(!config.webviewViaProxy),
              });
            }}
          />
        </Row>
      </Section>

      {/* Login — default view is TW only; HK has no QR login */}
      {config?.region === "TW" && (
        <Section title={t("settings.section.login")}>
          <Row label={t("settings.default_login_view")}>
            <Segmented
              options={DEFAULT_LOGIN_VIEWS.map((v) => ({ value: v.value, label: t(v.labelKey) }))}
              value={config?.defaultLoginView ?? "normal"}
              onChange={handleDefaultLoginViewChange}
            />
          </Row>
        </Section>
      )}
    </div>
  );
}
