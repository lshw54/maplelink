import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { GameDownloadDto } from "../../lib/types";
import { Modal } from "../shared/Modal";

function openExternal(url: string) {
  import("@tauri-apps/plugin-shell").then(({ open }) => open(url));
}

function DownloadRow({ item }: { item: GameDownloadDto }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  function copy() {
    navigator.clipboard.writeText(item.url);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="flex flex-col gap-2 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-3.5 py-3">
      {/* Full-width name so version numbers are never truncated */}
      <span className="text-sm leading-snug font-semibold break-words text-[var(--text)]">
        {item.name}
      </span>
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-text-dim">{item.size}</span>
        <div className="flex shrink-0 gap-2">
          <button
            onClick={copy}
            className="rounded-lg border border-border px-3 py-1.5 text-xs font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
          >
            {copied ? t("toolbox.download.copied") : t("toolbox.download.copy")}
          </button>
          <button
            onClick={() => openExternal(item.url)}
            className="rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-3 py-1.5 text-xs font-semibold text-white transition-opacity hover:opacity-90 active:scale-95"
          >
            {t("toolbox.download.download")}
          </button>
        </div>
      </div>
    </div>
  );
}

export function GameDownloadModal({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const {
    data: items,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ["gameDownloadList"],
    queryFn: () => commands.getGameDownloadList(),
    enabled: isOpen,
    staleTime: 5 * 60 * 1000,
  });

  const games = items?.filter((i) => i.kind === "game") ?? [];
  const patches = items?.filter((i) => i.kind !== "game") ?? [];

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={t("toolbox.download.title")}>
      <div className="flex flex-col gap-4">
        {/* Security note: official links only, we never touch client files */}
        <p className="rounded-[10px] border border-[rgba(59,130,246,0.3)] bg-[rgba(59,130,246,0.06)] px-3 py-2.5 text-xs leading-relaxed text-blue-400">
          {t("toolbox.download.intro")}
        </p>

        {isLoading && (
          <div className="flex items-center justify-center gap-2 py-6 text-[13px] text-text-dim">
            <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-text-faint border-t-accent" />
            {t("toolbox.download.loading")}
          </div>
        )}

        {isError && !isLoading && (
          <div className="flex flex-col items-center gap-2 py-4">
            <p className="text-[13px] text-red-400">{t("toolbox.download.error")}</p>
            <button
              onClick={() => refetch()}
              className="rounded-lg border border-border px-3 py-1.5 text-xs font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("toolbox.download.retry")}
            </button>
          </div>
        )}

        {!isLoading && !isError && items && (
          <div className="flex max-h-[50vh] flex-col gap-4 overflow-y-auto">
            {games.length > 0 && (
              <div className="flex flex-col gap-2">
                <span className="text-[11px] font-semibold tracking-[2px] text-text-faint uppercase">
                  {t("toolbox.download.group_game")}
                </span>
                {games.map((it) => (
                  <DownloadRow key={it.id} item={it} />
                ))}
              </div>
            )}
            {patches.length > 0 && (
              <div className="flex flex-col gap-2">
                <span className="text-[11px] font-semibold tracking-[2px] text-text-faint uppercase">
                  {t("toolbox.download.group_patch")}
                </span>
                {patches.map((it) => (
                  <DownloadRow key={it.id} item={it} />
                ))}
              </div>
            )}
            {items.length === 0 && (
              <p className="py-4 text-center text-[13px] text-text-dim">
                {t("toolbox.download.empty")}
              </p>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
