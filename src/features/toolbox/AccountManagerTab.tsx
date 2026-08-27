import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { ImportExportBar } from "./ImportExportBar";
import { Segmented } from "./ToolboxUi";
import type { SavedAccountDto } from "../../lib/types";

type RegionFilter = "" | "HK" | "TW";

export function AccountManagerTab() {
  const { t } = useTranslation();
  const [allAccounts, setAllAccounts] = useState<SavedAccountDto[]>([]);
  const [filter, setFilter] = useState<RegionFilter>("");
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const refresh = useCallback(() => {
    commands
      .getAllSavedAccounts()
      .then(setAllAccounts)
      .catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const filtered = filter ? allAccounts.filter((a) => a.region === filter) : allAccounts;

  async function handleDelete(acct: SavedAccountDto) {
    try {
      await commands.deleteSavedAccount(acct.account, acct.region);
      setAllAccounts((prev) =>
        prev.filter((a) => !(a.account === acct.account && a.region === acct.region)),
      );
      if (expandedId === acct.account + acct.region) setExpandedId(null);
    } catch {
      /* non-critical */
    }
  }

  const FILTERS: { value: RegionFilter; label: string }[] = [
    { value: "", label: t("toolbox.account_manager.filter_all") },
    { value: "HK", label: "HK" },
    { value: "TW", label: "TW" },
  ];

  return (
    <div className="flex flex-col gap-4">
      {/* Header line: caption · region filter · export / import */}
      <div className="flex items-center gap-3">
        <span className="px-1 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
          {t("toolbox.account_manager.saved")}
        </span>
        <Segmented options={FILTERS} value={filter} onChange={setFilter} />
        <div className="flex-1" />
        <ImportExportBar onImported={refresh} />
      </div>

      {/* Account list */}
      {filtered.length === 0 ? (
        <div className="flex items-center justify-center rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] py-10">
          <span className="text-[11.5px] text-text-dim">
            {t("toolbox.account_manager.no_saved")}
          </span>
        </div>
      ) : (
        <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] [&>*+*]:border-t [&>*+*]:border-[var(--tb-border)]">
          {filtered.map((a) => {
            const key = a.account + a.region;
            const isExpanded = expandedId === key;
            return (
              <div key={key}>
                <button
                  onClick={() => setExpandedId(isExpanded ? null : key)}
                  className="flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-[var(--surface-hover)]"
                >
                  <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[var(--accent-dark)] text-[11px] font-bold text-[var(--on-accent)]">
                    {a.account.charAt(0).toUpperCase()}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[11.5px] font-medium text-[var(--text)]">
                      {a.account}
                    </div>
                  </div>
                  <span className="shrink-0 rounded bg-[var(--surface-hover)] px-1.5 py-0.5 text-[10.5px] font-semibold text-text-dim">
                    {a.region}
                  </span>
                  {a.hasPassword && (
                    <span className="shrink-0 rounded bg-[rgba(74,222,128,0.1)] px-1.5 py-0.5 text-[10.5px] font-semibold text-[#4ade80]">
                      🔑
                    </span>
                  )}
                  {a.rememberPassword && (
                    <span className="shrink-0 rounded bg-[rgba(var(--accent-rgb),0.1)] px-1.5 py-0.5 text-[10.5px] font-semibold text-accent">
                      💾
                    </span>
                  )}
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 12 12"
                    fill="none"
                    className={`shrink-0 text-text-faint transition-transform ${isExpanded ? "rotate-180" : ""}`}
                  >
                    <path
                      d="M3 5L6 8L9 5"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </button>

                {isExpanded && (
                  <div className="border-t border-[var(--tb-border)] px-3.5 py-2.5">
                    <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-[11px]">
                      <div className="text-text-dim">{t("toolbox.account_manager.region")}</div>
                      <div className="text-right font-semibold text-[var(--text)]">{a.region}</div>
                      <div className="text-text-dim">
                        {t("toolbox.account_manager.password_saved")}
                      </div>
                      <div className="text-right font-semibold text-[var(--text)]">
                        {a.hasPassword ? "●●●●●●●●" : "—"}
                      </div>
                      <div className="text-text-dim">{t("toolbox.account_manager.remember")}</div>
                      <div className="text-right font-semibold text-[var(--text)]">
                        {a.rememberPassword ? "✓" : "—"}
                      </div>
                    </div>
                    <div className="mt-3 flex justify-end">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDelete(a);
                        }}
                        className="rounded-md border border-[var(--danger,#ef4444)] px-2.5 py-1 text-[11px] font-semibold text-[var(--danger,#ef4444)] transition-colors hover:bg-[var(--danger,#ef4444)] hover:text-white"
                      >
                        {t("toolbox.account_manager.delete")}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
