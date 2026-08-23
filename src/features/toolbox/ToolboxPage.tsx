import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { useUiStore } from "../../lib/stores/ui-store";
import { useConfigStore } from "../../lib/stores/config-store";
import { ToolsTab } from "./ToolsTab";
import { AccountManagerTab } from "./AccountManagerTab";
import { SettingsTab } from "./SettingsTab";
import { AdvancedTab } from "./AdvancedTab";
import { AboutTab } from "./AboutTab";
import { AnnouncementsTab } from "./AnnouncementsTab";

type ToolboxTab = "tools" | "account_manager" | "settings" | "advanced" | "announcements" | "about";

const TABS: { key: ToolboxTab; labelKey: string; icon: string }[] = [
  { key: "tools", labelKey: "toolbox.tabs.tools", icon: "🛠" },
  { key: "announcements", labelKey: "toolbox.tabs.announcements", icon: "📢" },
  { key: "account_manager", labelKey: "toolbox.tabs.account_manager", icon: "👤" },
  { key: "settings", labelKey: "toolbox.tabs.settings", icon: "⚙" },
  { key: "advanced", labelKey: "toolbox.tabs.advanced", icon: "🔧" },
  { key: "about", labelKey: "toolbox.tabs.about", icon: "ℹ" },
];

export function ToolboxPage() {
  const { t } = useTranslation();
  const goBack = useUiStore((s) => s.goBack);
  const [activeTab, setActiveTab] = useState<ToolboxTab>("tools");
  // Compact UI: the nav collapses to an icon rail (icon + tiny label) so the
  // content column keeps its width in the smaller window.
  const compact = useConfigStore((s) => s.config?.compactUi ?? false);

  return (
    <div className="flex h-full overflow-hidden bg-[var(--tb-bg)]">
      {/* Left nav sidebar */}
      <nav
        className={`flex shrink-0 flex-col border-r border-[var(--tb-border)] bg-[var(--tb-nav-bg)] ${
          compact ? "w-[64px] py-2" : "w-[150px] py-4"
        }`}
      >
        {TABS.map((tab) =>
          compact ? (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              title={t(tab.labelKey)}
              className={`mx-1.5 my-0.5 flex flex-col items-center gap-0.5 rounded-lg px-1 py-1.5 text-[9px] font-semibold tracking-[0.3px] transition-all hover:bg-[var(--surface)] hover:text-[var(--text)] ${
                activeTab === tab.key
                  ? "bg-[rgba(232,162,58,0.1)] text-accent shadow-[inset_0_0_0_1px_rgba(232,162,58,0.25)]"
                  : "text-text-dim"
              }`}
            >
              <span className="text-[15px] leading-none">{tab.icon}</span>
              <span className="w-full truncate text-center leading-tight">{t(tab.labelKey)}</span>
            </button>
          ) : (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`flex items-center gap-2 border-l-[3px] px-[18px] py-2.5 text-left text-[12px] font-semibold tracking-[0.5px] transition-all hover:translate-y-[-1px] hover:bg-[var(--surface)] hover:text-[var(--text)] ${
                activeTab === tab.key
                  ? "border-l-accent bg-[rgba(232,162,58,0.05)] text-accent"
                  : "border-l-transparent text-text-dim"
              }`}
            >
              <span className="w-5 text-center text-sm">{tab.icon}</span>
              {t(tab.labelKey)}
            </button>
          ),
        )}

        <div className="flex-1" />

        {/* Back button */}
        <button
          onClick={() => goBack()}
          title={t("shared.titlebar.back")}
          className={`rounded-lg border border-[var(--tb-border)] bg-transparent text-center font-semibold text-text-dim uppercase transition-all hover:translate-y-[-2px] hover:border-accent hover:text-accent active:scale-95 ${
            compact
              ? "mx-1.5 px-1 py-1.5 text-[11px] tracking-[0px]"
              : "mx-3 px-3 py-2 text-[12px] tracking-[1px]"
          }`}
        >
          {compact ? "←" : t("shared.titlebar.back")}
        </button>
      </nav>

      {/* Right content area */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === "tools" && <ToolsTab />}
        {activeTab === "account_manager" && <AccountManagerTab />}
        {activeTab === "settings" && <SettingsTab />}
        {activeTab === "advanced" && <AdvancedTab />}
        {activeTab === "announcements" && <AnnouncementsTab />}
        {activeTab === "about" && <AboutTab />}
      </div>
    </div>
  );
}
