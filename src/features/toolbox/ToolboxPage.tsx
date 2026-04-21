import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { useUiStore } from "../../lib/stores/ui-store";
import { ToolsTab } from "./ToolsTab";
import { AccountManagerTab } from "./AccountManagerTab";
import { SettingsTab } from "./SettingsTab";
import { AdvancedTab } from "./AdvancedTab";
import { AboutTab } from "./AboutTab";

type ToolboxTab = "tools" | "account_manager" | "settings" | "advanced" | "about";

const TABS: { key: ToolboxTab; labelKey: string; icon: string }[] = [
  { key: "tools", labelKey: "toolbox.tabs.tools", icon: "🛠" },
  { key: "account_manager", labelKey: "toolbox.tabs.account_manager", icon: "👤" },
  { key: "settings", labelKey: "toolbox.tabs.settings", icon: "⚙" },
  { key: "advanced", labelKey: "toolbox.tabs.advanced", icon: "🔧" },
  { key: "about", labelKey: "toolbox.tabs.about", icon: "ℹ" },
];

export function ToolboxPage() {
  const { t } = useTranslation();
  const goBack = useUiStore((s) => s.goBack);
  const [activeTab, setActiveTab] = useState<ToolboxTab>("tools");

  return (
    <div className="flex h-full overflow-hidden bg-[var(--tb-bg)]">
      {/* Left nav sidebar */}
      <nav className="flex w-[150px] shrink-0 flex-col border-r border-[var(--tb-border)] bg-[var(--tb-nav-bg)] py-4">
        {TABS.map((tab) => (
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
        ))}

        <div className="flex-1" />

        {/* Back button */}
        <button
          onClick={() => goBack()}
          className="mx-3 rounded-lg border border-[var(--tb-border)] bg-transparent px-3 py-2 text-center text-[12px] font-semibold tracking-[1px] text-text-dim uppercase transition-all hover:translate-y-[-2px] hover:border-accent hover:text-accent active:scale-95"
        >
          {t("shared.titlebar.back")}
        </button>
      </nav>

      {/* Right content area */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === "tools" && <ToolsTab />}
        {activeTab === "account_manager" && <AccountManagerTab />}
        {activeTab === "settings" && <SettingsTab />}
        {activeTab === "advanced" && <AdvancedTab />}
        {activeTab === "about" && <AboutTab />}
      </div>
    </div>
  );
}
