import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";

export function AdvancedTab() {
  const { t } = useTranslation();
  const config = useConfigStore((s) => s.config);
  const setConfig = useSetConfig();

  return (
    <div className="flex flex-col gap-3">
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
    <div className="flex items-center justify-between rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
      <span className="text-[12px] text-[var(--text)]">{label}</span>
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
      className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors ${
        checked ? "bg-accent" : "bg-[var(--surface-hover)]"
      }`}
    >
      <span
        className={`inline-block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-[18px]" : "translate-x-[3px]"
        }`}
      />
    </button>
  );
}
