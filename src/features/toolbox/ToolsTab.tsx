import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { Modal } from "../../components/Modal";
import { GameDownloadModal } from "./GameDownloadModal";
import { Section } from "./ToolboxUi";

function getMaintenanceInfo(t: (key: string) => string) {
  const now = new Date();
  const day = now.getDay();
  const weekday = t(`toolbox.tools.weekday_${day}`);
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  const date = `${yyyy}/${mm}/${dd}`;
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

/** One tool, as a list row: tinted icon, name, one-line description, ›. */
function ToolRow({ card }: { card: ToolCard }) {
  return (
    <button
      onClick={card.onClick}
      disabled={card.disabled || card.loading}
      className="flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-50"
    >
      <div
        className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[15px] ${card.iconBg}`}
      >
        {card.loading ? "⏳" : card.icon}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[11.5px] font-medium text-[var(--text)]">{card.name}</div>
        <div className="mt-0.5 line-clamp-2 text-[10.5px] leading-snug text-text-faint">
          {card.desc}
        </div>
      </div>
      <span className="shrink-0 text-[12px] text-text-faint">›</span>
    </button>
  );
}

export function ToolsTab() {
  const { t } = useTranslation();
  const { weekday, date, isMaintenanceDay } = getMaintenanceInfo(t);
  const [cleaning, setCleaning] = useState(false);
  const [cleanResult, setCleanResult] = useState<string | null>(null);
  const [showConfirm, setShowConfirm] = useState(false);
  const [showWebviewConfirm, setShowWebviewConfirm] = useState(false);
  const [showDownload, setShowDownload] = useState(false);

  async function doResetWebview() {
    setShowWebviewConfirm(false);
    try {
      await commands.resetWebviewData();
      await commands.restartApp();
    } catch {
      /* restart failure is non-critical */
    }
  }

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
    <div className="flex flex-col gap-4">
      {/* Maintenance notice */}
      <div className="flex items-center gap-3 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-3.5 py-2.5">
        <span className="text-[15px]">🔧</span>
        <div className="min-w-0 flex-1">
          <div className="text-[11.5px] font-medium text-[var(--text)]">
            {weekday} · {date}
          </div>
          <div className="mt-0.5 text-[10.5px] text-text-faint">
            {isMaintenanceDay ? (
              <>
                {t("toolbox.tools.maintenance_time")} ·{" "}
                {t("toolbox.tools.version_maintenance_time")}
              </>
            ) : (
              t("toolbox.tools.no_maintenance")
            )}
          </div>
        </div>
        {isMaintenanceDay && (
          <span className="shrink-0 rounded bg-[rgba(234,179,8,0.15)] px-1.5 py-0.5 text-[10px] font-bold text-yellow-500">
            ⚠ MAINTENANCE
          </span>
        )}
      </div>

      {/* Game client */}
      <Section title={t("toolbox.tools.section_client")}>
        <ToolRow
          card={{
            icon: "⬇️",
            iconBg: "bg-[rgba(34,197,94,0.1)]",
            name: t("toolbox.tools.download_client"),
            desc: t("toolbox.tools.download_client_desc"),
            onClick: () => setShowDownload(true),
          }}
        />
        <ToolRow
          card={{
            icon: "📂",
            iconBg: "bg-[rgba(99,102,241,0.1)]",
            name: t("toolbox.tools.data_folder"),
            desc: t("toolbox.tools.data_folder_desc"),
            onClick: () => {
              commands.openDataFolder().catch(() => {});
            },
          }}
        />
      </Section>

      {/* System tools */}
      <Section title={t("toolbox.tools.section_system")}>
        <ToolRow
          card={{
            icon: "🗑",
            iconBg: "bg-[rgba(239,68,68,0.1)]",
            name: t("toolbox.tools.cleanup"),
            desc: cleanResult ?? String(t("toolbox.tools.cleanup_desc")),
            onClick: () => setShowConfirm(true),
            loading: cleaning,
          }}
        />
        <ToolRow
          card={{
            icon: "🌐",
            iconBg: "bg-[rgba(59,130,246,0.1)]",
            name: t("toolbox.tools.reset_webview"),
            desc: t("toolbox.tools.reset_webview_desc"),
            onClick: () => setShowWebviewConfirm(true),
          }}
        />
      </Section>

      {/* Report center */}
      <Section title={t("toolbox.tools.section_report")}>
        <ToolRow
          card={{
            icon: "⚠️",
            iconBg: "bg-[rgba(234,179,8,0.1)]",
            name: t("toolbox.tools.report_hack"),
            desc: t("toolbox.tools.report_hack_desc"),
            onClick: () =>
              commands
                .openWebPopup(
                  "https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx",
                  t("toolbox.tools.report_hack"),
                )
                .catch(() => {}),
          }}
        />
        <ToolRow
          card={{
            icon: "👑",
            iconBg: "bg-[rgba(168,85,247,0.1)]",
            name: t("toolbox.tools.report_team"),
            desc: t("toolbox.tools.report_team_desc"),
            onClick: () =>
              commands
                .openWebPopup(
                  "https://beanfun-event.beanfun.com/EventAD_Mobile/EventAD?eventAdId=3453",
                  t("toolbox.tools.report_team"),
                )
                .catch(() => {}),
          }}
        />
      </Section>

      {/* Calculators */}
      <Section title={t("toolbox.tools.section_calc")}>
        <ToolRow
          card={{
            icon: "⭐",
            iconBg: "bg-[rgba(234,179,8,0.1)]",
            name: t("toolbox.tools.starforce"),
            desc: t("toolbox.tools.starforce_desc"),
            onClick: () =>
              commands
                .openWebPopup(
                  "https://brendonmay.github.io/starforceCalculator/",
                  t("toolbox.tools.starforce"),
                )
                .catch(() => {}),
          }}
        />
        <ToolRow
          card={{
            icon: "💎",
            iconBg: "bg-[rgba(59,130,246,0.1)]",
            name: t("toolbox.tools.core_calc"),
            desc: t("toolbox.tools.core_calc_desc"),
            onClick: () =>
              commands
                .openWebPopup(
                  "https://phantasmicsky.github.io/NodestoneBuilder/",
                  t("toolbox.tools.core_calc"),
                )
                .catch(() => {}),
          }}
        />
      </Section>

      {/* Official client download list */}
      <GameDownloadModal isOpen={showDownload} onClose={() => setShowDownload(false)} />

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

      {/* WebView2 reset confirm modal */}
      <Modal
        isOpen={showWebviewConfirm}
        onClose={() => setShowWebviewConfirm(false)}
        title={t("toolbox.tools.reset_webview")}
      >
        <div className="flex flex-col gap-4">
          <p className="text-xs text-text-dim">{t("toolbox.tools.reset_webview_confirm")}</p>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowWebviewConfirm(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={doResetWebview}
              className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
            >
              {t("toolbox.tools.reset_webview_restart")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
