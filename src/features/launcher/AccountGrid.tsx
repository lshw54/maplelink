import { useState, useCallback } from "react";
import { useTranslation } from "../../lib/i18n";
import { useGameAccounts, useRefreshAccounts } from "../../lib/hooks/use-accounts";
import { AccountContextMenu } from "./AccountContextMenu";
import type { GameAccountDto } from "../../lib/types";

interface AccountGridProps {
  selectedAccountId: string | null;
  onSelectAccount: (account: GameAccountDto) => void;
}

interface ContextState {
  position: { x: number; y: number };
  accountId: string;
}

export function AccountGrid({ selectedAccountId, onSelectAccount }: AccountGridProps) {
  const { t } = useTranslation();
  const { data: accounts, isLoading } = useGameAccounts();
  const refreshAccounts = useRefreshAccounts();
  const [contextMenu, setContextMenu] = useState<ContextState | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent, accountId: string) => {
    e.preventDefault();
    setContextMenu({ position: { x: e.clientX, y: e.clientY }, accountId });
  }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  function handleCopyAccount(e: React.MouseEvent, accountId: string) {
    e.stopPropagation();
    navigator.clipboard.writeText(accountId);
    setCopiedId(accountId);
    setTimeout(() => setCopiedId(null), 1500);
  }

  return (
    <div className="flex flex-1 flex-col gap-2 overflow-hidden">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-text-dim">{t("launcher.accounts")}</span>
        <button onClick={refreshAccounts} className="text-[12px] text-accent hover:underline">
          {t("launcher.refresh")}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <p className="py-4 text-center text-xs text-text-dim">{t("app.loading")}</p>
        ) : !accounts?.length ? (
          <p className="py-4 text-center text-xs text-text-dim">{t("launcher.no_accounts")}</p>
        ) : (
          <div className="grid grid-cols-2 gap-2">
            {accounts.map((account) => {
              const isSelected = selectedAccountId === account.id;
              const initial = account.displayName.charAt(0).toUpperCase();
              const isCopied = copiedId === account.id;
              return (
                <button
                  key={account.id}
                  onClick={() => onSelectAccount(account)}
                  onContextMenu={(e) => handleContextMenu(e, account.id)}
                  className={`group relative flex flex-col items-center gap-1.5 rounded-xl border p-3 text-center backdrop-blur-sm transition-colors duration-150 ${
                    isSelected
                      ? "border-accent bg-[rgba(232,162,58,0.05)] shadow-[0_0_20px_rgba(232,162,58,0.15)]"
                      : "border-border bg-[var(--surface)] hover:border-[var(--border)] hover:bg-[var(--surface-hover)]"
                  }`}
                >
                  {/* Copy button — top-right, visible on hover */}
                  <span
                    role="button"
                    tabIndex={-1}
                    onClick={(e) => handleCopyAccount(e, account.id)}
                    title={t("launcher.context.copy_account")}
                    className={`absolute top-1.5 right-1.5 rounded p-0.5 transition-all ${
                      isCopied
                        ? "text-green-400 opacity-100"
                        : "text-text-faint opacity-0 group-hover:opacity-100 hover:text-accent"
                    }`}
                  >
                    {isCopied ? (
                      <svg
                        width="12"
                        height="12"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    ) : (
                      <svg
                        width="12"
                        height="12"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <rect x="9" y="9" width="13" height="13" rx="2" />
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                      </svg>
                    )}
                  </span>
                  <div
                    className={`flex h-[38px] w-[38px] items-center justify-center rounded-full border-2 text-sm font-bold transition-colors duration-150 ${
                      isSelected
                        ? "border-accent bg-[var(--surface-hover)] text-accent"
                        : "border-border bg-[var(--surface-hover)] text-text-dim group-hover:border-accent group-hover:text-accent"
                    }`}
                  >
                    {initial}
                  </div>
                  <span className="truncate text-xs font-medium text-[var(--text)]">
                    {account.displayName}
                  </span>
                  <span className="truncate font-mono text-[12px] text-text-faint">
                    #{account.sn}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      <AccountContextMenu
        position={contextMenu?.position ?? null}
        account={accounts?.find((a) => a.id === contextMenu?.accountId) ?? null}
        onClose={closeContextMenu}
      />
    </div>
  );
}
