import { useTranslation } from "../../lib/i18n";

export function AdvancedTab() {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-center rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-8">
        <span className="text-[12px] text-text-dim">{t("toolbox.advanced.no_options")}</span>
      </div>
    </div>
  );
}
