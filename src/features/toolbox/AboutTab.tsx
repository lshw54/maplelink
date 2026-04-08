import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { UpdateInfoDto } from "../../lib/types";

export function AboutTab() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState("0.1.0");
  const [checking, setChecking] = useState(false);
  const [updateResult, setUpdateResult] = useState<UpdateInfoDto | null | undefined>(undefined);

  useEffect(() => {
    commands
      .getAppVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  async function handleCheckUpdate() {
    setChecking(true);
    setUpdateResult(undefined);
    try {
      const info = await commands.checkUpdate();
      setUpdateResult(info);
    } catch {
      setUpdateResult(null);
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="flex flex-col items-center gap-4 py-6">
      {/* App icon */}
      <img src="/app-icon.png" alt="MapleLink" className="h-16 w-16 rounded-[16px] shadow-lg" />

      <div className="flex flex-col items-center gap-1">
        <span className="text-xs font-semibold text-[var(--text)]">{t("app.name")}</span>
        <span className="text-[12px] text-text-dim">
          {t("toolbox.about.version")} {appVersion}
        </span>
      </div>

      <p className="max-w-xs text-center text-[12px] text-text-dim">
        {t("toolbox.about.description")}
      </p>

      {/* Check for Updates */}
      <button
        onClick={handleCheckUpdate}
        disabled={checking}
        className="rounded-lg border border-border px-4 py-1.5 text-[12px] font-semibold text-text-dim transition-all hover:bg-[var(--surface-hover)] hover:text-accent active:scale-95 disabled:opacity-50"
      >
        {checking ? t("toolbox.about.checking_update") : t("toolbox.about.check_update")}
      </button>
      {updateResult !== undefined && (
        <span className={`text-[12px] ${updateResult ? "text-accent" : "text-text-faint"}`}>
          {updateResult
            ? t("toolbox.about.update_available").replace("{{version}}", updateResult.version)
            : t("toolbox.about.no_update")}
        </span>
      )}

      <div className="flex flex-col items-center gap-1 text-[12px] text-text-dim">
        <a
          href="https://github.com/maplelink"
          target="_blank"
          rel="noopener noreferrer"
          className="text-accent hover:underline"
        >
          {t("toolbox.about.github")}
        </a>
        <span>{t("toolbox.about.license")}</span>
      </div>
    </div>
  );
}
