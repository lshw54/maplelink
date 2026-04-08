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

  const handleContextMenu = useCallback((e: React.MouseEvent, accountId: string) => {
    e.preventDefault();
    setContextMenu({ position: { x: e.clientX, y: e.clientY }, accountId });
  }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

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
              return (
                <button
                  key={account.id}
                  onClick={() => onSelectAccount(account)}
                  onContextMenu={(e) => handleContextMenu(e, account.id)}
                  className={`group flex flex-col items-center gap-1.5 rounded-xl border p-3 text-center backdrop-blur-sm transition-colors duration-150 ${
                    isSelected
                      ? "border-accent bg-[rgba(232,162,58,0.05)] shadow-[0_0_20px_rgba(232,162,58,0.15)]"
                      : "border-border bg-[var(--surface)] hover:border-[var(--border)] hover:bg-[var(--surface-hover)]"
                  }`}
                >
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
