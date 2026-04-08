import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { Modal } from "../shared/Modal";

const WEEKDAYS_ZH = ["日", "一", "二", "三", "四", "五", "六"];

function getMaintenanceInfo() {
  const now = new Date();
  const day = now.getDay();
  const weekday = WEEKDAYS_ZH[day];
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  const date = `${yyyy}/${mm}/${dd}`;
  // MapleStory version maintenance: Wednesday 00:00 – 12:00
  const isMaintenanceDay = day === 3;
  return { weekday, date, isMaintenanceDay };
}

interface ToolCard {
  icon: string;
  iconBg: string;
  name: string;
  desc: string;
  onClick?: () => void;
  disabled?: boolean;
  loading?: boolean;
}

function ToolCardItem({ card }: { card: ToolCard }) {
  return (
    <button
      onClick={card.onClick}
      disabled={card.disabled || card.loading}
      className="hover:border-accent/30 flex w-[160px] flex-col items-center gap-2 rounded-[12px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-5 transition-all hover:translate-y-[-2px] active:scale-95 disabled:opacity-50 disabled:hover:translate-y-0"
    >
      <div
        className={`flex h-10 w-10 items-center justify-center rounded-xl text-lg ${card.iconBg}`}
      >
        {card.loading ? "⏳" : card.icon}
      </div>
      <span className="text-xs font-semibold text-[var(--text)]">{card.name}</span>
      <span className="text-center text-[11px] leading-tight text-text-dim">{card.desc}</span>
    </button>
  );
}

export function ToolsTab() {
  const { t } = useTranslation();
  const { weekday, date, isMaintenanceDay } = getMaintenanceInfo();
  const [cleaning, setCleaning] = useState(false);
  const [cleanResult, setCleanResult] = useState<string | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);

  async function doCleanup() {
    setShowConfirm(false);
    setCleaning(true);
    setCleanResult(null);
    try {
      const result = await commands.cleanupGameCache();
      setCleanResult(
        result === "nothing to clean"
          ? String(t("toolbox.tools.cleanup_nothing"))
          : `✅ ${String(t("toolbox.tools.cleanup_done"))}`,
      );
    } catch {
      setCleanResult(`❌ ${String(t("toolbox.tools.cleanup_error"))}`);
    } finally {
      setCleaning(false);
      setTimeout(() => setCleanResult(null), 3000);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      {/* Maintenance banner */}
      <div className="flex items-center gap-3 rounded-[12px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
        <span className="text-xl">🔧</span>
        <div className="flex flex-col">
          <span className="text-xs font-semibold text-[var(--text)]">
            {t("toolbox.tools.weekday_prefix")}
            {weekday} · {date}
          </span>
          <span className="text-[11px] text-text-dim">
            {isMaintenanceDay ? (
              <>
                <span className="mr-1.5 rounded bg-[rgba(234,179,8,0.15)] px-1.5 py-0.5 text-[10px] font-bold text-yellow-500">
                  ⚠ MAINTENANCE
                </span>
                {t("toolbox.tools.maintenance_time")} ·{" "}
                {t("toolbox.tools.version_maintenance_time")}
              </>
            ) : (
              t("toolbox.tools.no_maintenance")
            )}
          </span>
        </div>
      </div>

      {/* System tools */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[2px] text-text-faint">
          {t("toolbox.tools.section_system")}
        </div>
        <div className="flex flex-wrap gap-3">
          <ToolCardItem
            card={{
              icon: "🗑",
              iconBg: "bg-[rgba(239,68,68,0.1)]",
              name: t("toolbox.tools.cleanup"),
              desc: cleanResult ?? String(t("toolbox.tools.cleanup_desc")),
              onClick: () => setShowConfirm(true),
              loading: cleaning,
            }}
          />
        </div>
      </div>

      {/* Report center */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[2px] text-text-faint">
          {t("toolbox.tools.section_report")}
        </div>
        <div className="flex flex-wrap gap-3">
          <ToolCardItem
            card={{
              icon: "⚠️",
              iconBg: "bg-[rgba(234,179,8,0.1)]",
              name: t("toolbox.tools.report_hack"),
              desc: t("toolbox.tools.report_hack_desc"),
              disabled: true,
            }}
          />
          <ToolCardItem
            card={{
              icon: "👑",
              iconBg: "bg-[rgba(168,85,247,0.1)]",
              name: t("toolbox.tools.report_team"),
              desc: t("toolbox.tools.report_team_desc"),
              disabled: true,
            }}
          />
        </div>
      </div>

      {/* Calculators */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[2px] text-text-faint">
          {t("toolbox.tools.section_calc")}
        </div>
        <div className="flex flex-wrap gap-3">
          <ToolCardItem
            card={{
              icon: "⭐",
              iconBg: "bg-[rgba(234,179,8,0.1)]",
              name: t("toolbox.tools.starforce"),
              desc: t("toolbox.tools.starforce_desc"),
              disabled: true,
            }}
          />
          <ToolCardItem
            card={{
              icon: "💎",
              iconBg: "bg-[rgba(59,130,246,0.1)]",
              name: t("toolbox.tools.core_calc"),
              desc: t("toolbox.tools.core_calc_desc"),
              disabled: true,
            }}
          />
        </div>
      </div>

      {/* Cleanup confirm modal */}
      <Modal
        isOpen={showConfirm}
        onClose={() => setShowConfirm(false)}
        title={t("toolbox.tools.cleanup")}
      >
        <div className="flex flex-col gap-4">
          <p className="text-xs text-text-dim">{t("toolbox.tools.cleanup_confirm")}</p>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowConfirm(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={doCleanup}
              className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("common.confirm")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
