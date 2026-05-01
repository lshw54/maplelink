import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { commands } from "../../lib/tauri";
import { useUpdateStore } from "../../lib/stores/update-store";
import { UpdateDialog } from "../shared/UpdateDialog";
import type { UpdateInfoDto } from "../../lib/types";

export function AboutTab() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState("...");
  const [platform, setPlatform] = useState("...");
  const [checking, setChecking] = useState(false);
  const [updateResult, setUpdateResult] = useState<UpdateInfoDto | null | undefined>(undefined);
  const [showUpdateDialog, setShowUpdateDialog] = useState(false);
  const cachedUpdate = useUpdateStore((s) => s.availableUpdate);
  const config = useConfigStore((s) => s.config);

  const effectiveResult = updateResult !== undefined ? updateResult : cachedUpdate;
  const channelLabel =
    config?.updateChannel === "pre-release"
      ? t("toolbox.about.channel_prerelease")
      : t("toolbox.about.channel_stable");

  const versionStatus = checking
    ? t("toolbox.about.checking_update")
    : effectiveResult
      ? t("toolbox.about.update_available_short")
      : updateResult === null
        ? t("toolbox.about.no_update")
        : null;

  useEffect(() => {
    commands
      .getAppVersion()
      .then(setAppVersion)
      .catch(() => {});
    commands
      .getPlatformInfo()
      .then(setPlatform)
      .catch(() => setPlatform("Windows"));
  }, []);

  async function handleCheckUpdate() {
    setChecking(true);
    setUpdateResult(undefined);
    try {
      const info = await commands.checkUpdate();
      setUpdateResult(info);
      if (info) {
        useUpdateStore.getState().setAvailableUpdate(info);
        setShowUpdateDialog(true);
      }
    } catch {
      setUpdateResult(null);
    } finally {
      setChecking(false);
    }
  }

  function openExternal(url: string) {
    import("@tauri-apps/plugin-shell").then(({ open }) => open(url));
  }

  return (
    <div className="flex flex-col gap-3 py-2">
      {/* Header */}
      <div className="flex items-center gap-3">
        <img src="/app-icon.png" alt="MapleLink" className="h-11 w-11 rounded-[10px] shadow-lg" />
        <div>
          <div className="text-sm font-bold text-[var(--text)]">{t("app.name")}</div>
          <div className="flex items-center gap-1.5">
            <span className="font-mono text-[11px] text-text-dim">v{appVersion}</span>
            {versionStatus && (
              <>
                <span className="text-[10px] text-text-faint">·</span>
                {effectiveResult && !showUpdateDialog ? (
                  <button
                    onClick={() => setShowUpdateDialog(true)}
                    className="text-[11px] text-accent hover:underline"
                  >
                    {versionStatus}
                  </button>
                ) : (
                  <span className={`text-[11px] ${checking ? "text-text-faint" : "text-text-dim"}`}>
                    {versionStatus}
                  </span>
                )}
              </>
            )}
          </div>
        </div>
        <div className="flex-1" />
        <button
          onClick={handleCheckUpdate}
          disabled={checking}
          className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-95 disabled:opacity-50"
        >
          {checking ? t("toolbox.about.checking_update") : t("toolbox.about.check_update")}
        </button>
      </div>

      {/* Info table + disclaimer merged */}
      <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]">
        {/* Disclaimer row */}
        <div className="border-b border-[var(--tb-border)] bg-yellow-500/5 px-4 py-2">
          <p className="text-[10px] leading-normal text-text-dim">
            ⚠️ {t("toolbox.about.disclaimer")}
          </p>
        </div>
        <InfoRow label={t("toolbox.about.update_channel")} value={channelLabel} />
        <InfoRow label={t("toolbox.about.frontend")} value="React 19 + TypeScript" />
        <InfoRow label={t("toolbox.about.backend")} value="Rust + Tauri v2" />
        <InfoRow label={t("toolbox.about.platform")} value={platform} />
        <InfoRow label={t("toolbox.about.license_label")} value="MIT License" last />
      </div>

      {/* Links */}
      <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)]">
        <LinkRow
          icon="🔗"
          label={t("toolbox.about.github_project")}
          onClick={() => openExternal("https://github.com/lshw54/maplelink")}
        />
        <LinkRow
          icon="🐛"
          label={t("toolbox.about.issues")}
          onClick={() => openExternal("https://github.com/lshw54/maplelink/issues")}
          last
        />
      </div>

      {/* Copyright */}
      <p className="text-center text-[10px] text-text-faint">
        © 2025 MapleLink Contributors · MIT License
      </p>

      {showUpdateDialog && effectiveResult && (
        <UpdateDialog update={effectiveResult} onClose={() => setShowUpdateDialog(false)} />
      )}
    </div>
  );
}

function InfoRow({ label, value, last }: { label: string; value: string; last?: boolean }) {
  return (
    <div
      className={`flex items-center justify-between bg-[var(--tb-card)] px-4 py-2 ${
        last ? "" : "border-b border-[var(--tb-border)]"
      }`}
    >
      <span className="text-[11px] font-medium text-text-dim">{label}</span>
      <span className="text-[11px] font-semibold text-[var(--text)]">{value}</span>
    </div>
  );
}

function LinkRow({
  icon,
  label,
  onClick,
  last,
}: {
  icon: string;
  label: string;
  onClick: () => void;
  last?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex w-full items-center gap-2.5 bg-[var(--tb-card)] px-4 py-2 text-left transition-colors hover:bg-[var(--surface-hover)] ${
        last ? "" : "border-b border-[var(--tb-border)]"
      }`}
    >
      <span className="w-4 text-center text-xs">{icon}</span>
      <span className="flex-1 text-[11px] font-medium text-[var(--text)]">{label}</span>
      <span className="text-[10px] text-text-faint">↗</span>
    </button>
  );
}
