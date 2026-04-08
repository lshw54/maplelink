import { useState, useEffect } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { SavedAccountDto } from "../../lib/types";

type RegionFilter = "" | "HK" | "TW";

export function AccountManagerTab() {
  const { t } = useTranslation();
  const [allAccounts, setAllAccounts] = useState<SavedAccountDto[]>([]);
  const [filter, setFilter] = useState<RegionFilter>("");
  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    commands
      .getAllSavedAccounts()
      .then(setAllAccounts)
      .catch(() => {});
  }, []);

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
    <div className="flex flex-col gap-3">
      {/* Header + filter */}
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-[var(--text)]">
          {t("toolbox.account_manager.saved")}
        </span>
        <div className="flex overflow-hidden rounded-lg border border-[var(--tb-border)]">
          {FILTERS.map((f, i) => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={`px-2.5 py-1 text-[12px] font-semibold tracking-[0.3px] transition-all ${
                i < FILTERS.length - 1 ? "border-r border-[var(--tb-border)]" : ""
              } ${
                filter === f.value
                  ? "bg-gradient-to-br from-accent to-[#c47a1a] text-white"
                  : "bg-[var(--tb-card)] text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {/* Account list */}
      {filtered.length === 0 ? (
        <div className="flex items-center justify-center rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] py-10">
          <span className="text-[12px] text-text-dim">{t("toolbox.account_manager.no_saved")}</span>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map((a) => {
            const key = a.account + a.region;
            const isExpanded = expandedId === key;
            return (
              <div
                key={key}
                className="overflow-hidden rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] transition-all"
              >
                <button
                  onClick={() => setExpandedId(isExpanded ? null : key)}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--surface-hover)]"
                >
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-[#c47a1a] text-[12px] font-bold text-white">
                    {a.account.charAt(0).toUpperCase()}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[12px] font-semibold text-[var(--text)]">
                      {a.account}
                    </div>
                  </div>
                  <span className="shrink-0 rounded bg-[var(--surface-hover)] px-1.5 py-0.5 text-[12px] font-semibold text-text-dim">
                    {a.region}
                  </span>
                  {a.hasPassword && (
                    <span className="shrink-0 rounded bg-[rgba(74,222,128,0.1)] px-1.5 py-0.5 text-[12px] font-semibold text-[#4ade80]">
                      🔑
                    </span>
                  )}
                  {a.rememberPassword && (
                    <span className="shrink-0 rounded bg-[rgba(232,162,58,0.1)] px-1.5 py-0.5 text-[12px] font-semibold text-accent">
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
                  <div className="border-t border-[var(--tb-border)] px-4 py-3">
                    <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-[12px]">
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
                        className="rounded-lg border border-[var(--danger,#ef4444)] px-3 py-1 text-[12px] font-semibold text-[var(--danger,#ef4444)] transition-colors hover:bg-[var(--danger,#ef4444)] hover:text-white"
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
