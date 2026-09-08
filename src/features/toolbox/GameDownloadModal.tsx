import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { GameDownloadDto } from "../../lib/types";
import { Modal } from "../../components/Modal";

function openExternal(url: string) {
  commands.openExternal(url).catch(() => {});
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
      <span className="text-xs leading-snug font-semibold break-words text-[var(--text)]">
        {item.name}
      </span>
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] text-text-dim">{item.size}</span>
        <div className="flex shrink-0 gap-2">
          <button
            onClick={copy}
            className="rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
          >
            {copied ? t("toolbox.download.copied") : t("toolbox.download.copy")}
          </button>
          <button
            onClick={() => openExternal(item.url)}
            className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-2.5 py-1 text-[11px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95"
          >
            {t("toolbox.download.download")}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * The torrent the Gamania Games Manager itself fetches for the full client.
 * Still links-only: the player feeds it to their own BitTorrent client.
 */
function FullClientSection({ isOpen }: { isOpen: boolean }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const {
    data: info,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ["gameFullClientInfo"],
    queryFn: () => commands.getGameFullClientInfo(),
    enabled: isOpen,
    staleTime: 5 * 60 * 1000,
  });

  function copy() {
    if (!info) return;
    navigator.clipboard.writeText(info.torrentUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  async function save() {
    setSaveState("saving");
    let next: "idle" | "saved" | "error";
    try {
      next = (await commands.saveGameFullClientTorrent()) ? "saved" : "idle";
    } catch {
      next = "error";
    }
    setSaveState(next);
    if (next === "saved") setTimeout(() => setSaveState("idle"), 2000);
  }

  return (
    <div className="flex flex-col gap-2">
      <span className="text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
        {t("toolbox.download.full.title")}
      </span>
      <p className="text-[11px] leading-relaxed text-text-dim">
        {t("toolbox.download.full.intro")}
      </p>
      <details className="rounded-[10px] border border-[var(--tb-border)] px-3 py-2 text-[11px] leading-relaxed text-text-dim">
        <summary className="cursor-pointer font-semibold text-[var(--text)] select-none">
          {t("toolbox.download.full.why_title")}
        </summary>
        <p className="mt-1.5">{t("toolbox.download.full.why_body")}</p>
      </details>

      {isLoading && (
        <div className="flex items-center gap-2 py-2 text-[12px] text-text-dim">
          <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-text-faint border-t-accent" />
          {t("toolbox.download.full.loading")}
        </div>
      )}

      {isError && !isLoading && (
        <div className="flex items-center gap-3 py-1">
          <p className="text-[12px] text-red-400">{t("toolbox.download.full.error")}</p>
          <button
            onClick={() => refetch()}
            className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
          >
            {t("toolbox.download.retry")}
          </button>
        </div>
      )}

      {info && (
        <div className="flex flex-col gap-2 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-3.5 py-3">
          <span className="text-xs leading-snug font-semibold break-words text-[var(--text)]">
            {info.productName} · {t("toolbox.download.full.version", { version: info.version })}
          </span>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-[11px] text-text-dim">
              {t("toolbox.download.full.size", {
                gb: (info.sizeBytes / 1024 ** 3).toFixed(1),
                count: String(info.fileCount),
              })}
              {info.publishDate &&
                ` · ${t("toolbox.download.full.published", { date: info.publishDate })}`}
            </span>
            <div className="flex shrink-0 gap-2">
              <button
                onClick={copy}
                className="rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
              >
                {copied ? t("toolbox.download.copied") : t("toolbox.download.full.copy_official")}
              </button>
              <button
                onClick={save}
                disabled={saveState === "saving"}
                className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-2.5 py-1 text-[11px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95 disabled:opacity-60"
              >
                {saveState === "saving"
                  ? t("toolbox.download.full.saving")
                  : saveState === "saved"
                    ? t("toolbox.download.full.saved")
                    : t("toolbox.download.full.save_torrent")}
              </button>
            </div>
          </div>
          {saveState === "error" && (
            <p className="text-[11px] text-red-400">{t("toolbox.download.full.save_error")}</p>
          )}
          <code className="block truncate rounded-md bg-[var(--surface-hover)] px-2 py-1 text-[10px] text-text-dim select-all">
            {info.torrentUrl}
          </code>
          <details className="mt-1 text-[11px] leading-relaxed text-text-dim">
            <summary className="cursor-pointer font-semibold text-[var(--text)] select-none">
              {t("toolbox.download.full.steps_title")}
            </summary>
            <ol className="mt-1.5 list-decimal space-y-1 pl-4">
              <li>{t("toolbox.download.full.step_1")}</li>
              <li>{t("toolbox.download.full.step_2")}</li>
              <li>
                {t("toolbox.download.full.step_3", {
                  folder: info.folderName,
                  version: info.version,
                })}
              </li>
              <li>{t("toolbox.download.full.step_4", { exe: info.exeName })}</li>
            </ol>
          </details>
        </div>
      )}
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
    <Modal isOpen={isOpen} onClose={onClose} title={t("toolbox.download.title")} size="lg">
      <div className="flex max-h-[65vh] flex-col gap-4 overflow-y-auto">
        {/* Security note: official links only, we never touch client files */}
        <p className="rounded-[10px] border border-[rgba(59,130,246,0.3)] bg-[rgba(59,130,246,0.06)] px-3 py-2 text-[11px] leading-relaxed text-blue-400">
          {t("toolbox.download.intro")}
        </p>

        <FullClientSection isOpen={isOpen} />

        {isLoading && (
          <div className="flex items-center justify-center gap-2 py-6 text-[12px] text-text-dim">
            <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-text-faint border-t-accent" />
            {t("toolbox.download.loading")}
          </div>
        )}

        {isError && !isLoading && (
          <div className="flex flex-col items-center gap-2 py-4">
            <p className="text-[12px] text-red-400">{t("toolbox.download.error")}</p>
            <button
              onClick={() => refetch()}
              className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("toolbox.download.retry")}
            </button>
          </div>
        )}

        {!isLoading && !isError && items && (
          <div className="flex flex-col gap-4">
            {games.length > 0 && (
              <div className="flex flex-col gap-2">
                <span className="text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
                  {t("toolbox.download.group_game")}
                </span>
                {games.map((it) => (
                  <DownloadRow key={it.id} item={it} />
                ))}
              </div>
            )}
            {patches.length > 0 && (
              <div className="flex flex-col gap-2">
                <span className="text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
                  {t("toolbox.download.group_patch")}
                </span>
                {patches.map((it) => (
                  <DownloadRow key={it.id} item={it} />
                ))}
              </div>
            )}
            {items.length === 0 && (
              <p className="py-4 text-center text-[12px] text-text-dim">
                {t("toolbox.download.empty")}
              </p>
            )}
          </div>
        )}
      </div>
    </Modal>
  );
}
