import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { GameDownloadDto } from "../../lib/types";

function openExternal(url: string) {
  commands.openExternal(url).catch(() => {});
}

function Row({ item }: { item: GameDownloadDto }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  function copy() {
    navigator.clipboard.writeText(item.url);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="flex flex-col gap-1 border-b border-[var(--tb-border)] px-3.5 py-2.5 last:border-b-0">
      <div className="flex items-center gap-3">
        <span className="min-w-0 flex-1 text-[12px] leading-snug font-semibold break-words text-[var(--text)]">
          {item.name}
          {item.manager && (
            <span className="ml-2 rounded bg-[rgba(234,179,8,0.14)] px-1.5 py-0.5 text-[10px] font-bold text-yellow-500">
              {t("toolbox.download.manager_badge")}
            </span>
          )}
        </span>
        <span className="w-16 shrink-0 text-right font-mono text-[11px] text-text-dim">
          {item.size}
        </span>
        <button
          onClick={copy}
          className="shrink-0 rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
        >
          {copied ? t("toolbox.download.copied") : t("toolbox.download.copy")}
        </button>
        <button
          onClick={() => openExternal(item.url)}
          className="shrink-0 rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-2.5 py-1 text-[11px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95"
        >
          {t("toolbox.download.download")}
        </button>
      </div>
      {item.manager && (
        <p className="text-[11px] leading-relaxed text-text-dim">
          {t("toolbox.download.manager_why")}
        </p>
      )}
    </div>
  );
}

/**
 * Everything that gets a client onto the machine in the first place: the
 * torrent the game manager itself uses, plus beanfun's own installer links.
 * MapleLink hands over links and a torrent file, and downloads nothing here.
 */
export function DownloadPanel({ onAutoInstall }: { onAutoInstall: () => void }) {
  const { t } = useTranslation();
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");

  const full = useQuery({
    queryKey: ["gameFullClientInfo"],
    queryFn: () => commands.getGameFullClientInfo(),
    staleTime: 5 * 60 * 1000,
  });
  const manifestUrl = full.data?.manifestUrl ?? "";
  const list = useQuery({
    queryKey: ["gameDownloadList"],
    queryFn: () => commands.getGameDownloadList(),
    staleTime: 5 * 60 * 1000,
  });

  async function saveTorrent() {
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

  const games = list.data?.filter((i) => i.kind === "game") ?? [];
  const patches = list.data?.filter((i) => i.kind !== "game") ?? [];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pt-4 pb-4">
      {/* Full client over BitTorrent. */}
      <section className="flex flex-col gap-2.5 rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)] p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <h2 className="text-[14px] font-bold">{t("toolbox.download.full.title")}</h2>
            {full.data && (
              <p className="mt-0.5 text-[11px] text-text-dim">
                {full.data.productName} · {full.data.version} ·{" "}
                {(full.data.sizeBytes / 1024 ** 3).toFixed(1)} GB ·{" "}
                {t("toolbox.download.full.size", {
                  gb: (full.data.sizeBytes / 1024 ** 3).toFixed(1),
                  count: String(full.data.fileCount),
                })}
              </p>
            )}
          </div>
          <div className="flex shrink-0 gap-2">
            {/* The quiet option first, the one most players want on the right. */}
            <button
              onClick={saveTorrent}
              disabled={saveState === "saving" || !full.data}
              className="rounded-lg border border-border px-4 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent disabled:opacity-50"
            >
              {saveState === "saving"
                ? t("toolbox.download.full.saving")
                : saveState === "saved"
                  ? t("toolbox.download.full.saved")
                  : t("client.manual_install")}
            </button>
            <button
              onClick={onAutoInstall}
              className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-1.5 text-[11px] font-bold text-[var(--on-accent)] transition-opacity hover:opacity-90 active:scale-95"
            >
              {t("client.auto_install")}
            </button>
          </div>
        </div>

        {full.isLoading && (
          <p className="text-[11px] text-text-dim">{t("toolbox.download.full.loading")}</p>
        )}
        {full.isError && (
          <div className="flex items-center gap-3">
            <p className="text-[11px] text-red-400">{t("toolbox.download.full.error")}</p>
            <button
              onClick={() => full.refetch()}
              className="rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim hover:bg-[var(--surface-hover)]"
            >
              {t("toolbox.download.retry")}
            </button>
          </div>
        )}
        {saveState === "error" && (
          <p className="text-[11px] text-red-400">{t("toolbox.download.full.save_error")}</p>
        )}

        <p className="text-[11px] leading-relaxed text-text-dim">
          {t("toolbox.download.full.intro")}
        </p>
        <ul className="flex flex-col gap-1 text-[11px] leading-relaxed text-text-dim">
          <li>
            <span className="font-semibold text-[var(--text)]">{t("client.manual_install")}</span>{" "}
            {t("client.manual_install_hint")}
          </li>
          <li>
            <span className="font-semibold text-[var(--text)]">{t("client.auto_install")}</span>{" "}
            {t("client.auto_install_hint")}
          </li>
        </ul>

        <details className="text-[11px] leading-relaxed text-text-dim">
          <summary className="cursor-pointer font-semibold text-[var(--text)] select-none">
            {t("toolbox.download.full.steps_title")}
          </summary>
          <ol className="mt-1.5 list-decimal space-y-1 pl-4">
            <li>{t("toolbox.download.full.step_1")}</li>
            <li>{t("toolbox.download.full.step_2")}</li>
            <li>{t("toolbox.download.full.step_3", { version: full.data?.version ?? "" })}</li>
            <li>{t("toolbox.download.full.step_4", { exe: full.data?.exeName ?? "" })}</li>
          </ol>
        </details>

        <details className="text-[11px] leading-relaxed text-text-dim">
          <summary className="cursor-pointer font-semibold text-[var(--text)] select-none">
            {t("toolbox.download.full.why_title")}
          </summary>
          <p className="mt-1.5">{t("toolbox.download.full.why_body")}</p>
        </details>

        {/* The manifest is public, so the comparison is checkable rather than
            something a player has to take our word for. */}
        {manifestUrl && (
          <div className="flex items-center gap-3">
            <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-text-dim">
              {t("client.manifest_hint")}
            </p>
            <button
              onClick={() => openExternal(manifestUrl)}
              className="shrink-0 rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
            >
              {t("client.manifest_open")}
            </button>
          </div>
        )}
      </section>

      {/* beanfun's own installer and patch links. */}
      <section className="flex flex-col gap-2">
        <h2 className="text-[14px] font-bold">{t("toolbox.download.title")}</h2>
        <p className="text-[11px] leading-relaxed text-blue-400">{t("toolbox.download.intro")}</p>

        {list.isLoading && (
          <p className="py-2 text-[11px] text-text-dim">{t("toolbox.download.loading")}</p>
        )}
        {list.isError && (
          <div className="flex items-center gap-3 py-1">
            <p className="text-[11px] text-red-400">{t("toolbox.download.error")}</p>
            <button
              onClick={() => list.refetch()}
              className="rounded-lg border border-border px-2.5 py-1 text-[11px] font-semibold text-text-dim hover:bg-[var(--surface-hover)]"
            >
              {t("toolbox.download.retry")}
            </button>
          </div>
        )}

        {games.length > 0 && (
          <>
            <span className="text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
              {t("toolbox.download.group_game")}
            </span>
            <div className="rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
              {games.map((it) => (
                <Row key={it.id} item={it} />
              ))}
            </div>
          </>
        )}
        {patches.length > 0 && (
          <>
            <span className="mt-1 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
              {t("toolbox.download.group_patch")}
            </span>
            <div className="rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
              {patches.map((it) => (
                <Row key={it.id} item={it} />
              ))}
            </div>
          </>
        )}
        {list.data?.length === 0 && (
          <p className="py-2 text-[11px] text-text-dim">{t("toolbox.download.empty")}</p>
        )}
      </section>
    </div>
  );
}
