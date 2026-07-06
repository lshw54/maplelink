import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { useConfigStore } from "../../lib/stores/config-store";
import { useSetConfig } from "../../lib/hooks/use-config";
import { commands } from "../../lib/tauri";
import { DnsCheck } from "./DnsCheck";
import type { WebLaunchStatus, WebLaunchTestCode } from "../../lib/types";

type CheckState = "ok" | "bad";

function StatusDot({ state }: { state: CheckState }) {
  return (
    <span
      className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[13px] ${
        state === "ok"
          ? "bg-[rgba(34,197,94,0.12)] text-green-500"
          : "bg-[rgba(239,68,68,0.12)] text-red-500"
      }`}
    >
      {state === "ok" ? "✓" : "✕"}
    </span>
  );
}

function CheckRow({
  state,
  title,
  detail,
  action,
}: {
  state: CheckState;
  title: string;
  detail: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-3 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
      <StatusDot state={state} />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="text-xs font-semibold text-[var(--text)]">{title}</span>
        <span className="truncate text-[11px] text-text-dim">{detail}</span>
      </div>
      {action}
    </div>
  );
}

/** One live-test row: shows a spinner while running, ✓/⚠/✕ + message once done. */
function TestRow({
  label,
  running,
  code,
}: {
  label: string;
  running: boolean;
  code?: WebLaunchTestCode;
}) {
  const { t } = useTranslation();
  const tone = !code ? "idle" : code === "ok" ? "ok" : code === "skipped_running" ? "warn" : "bad";
  const color =
    tone === "ok"
      ? "text-green-500"
      : tone === "warn"
        ? "text-yellow-500"
        : tone === "bad"
          ? "text-red-500"
          : "text-text-faint";
  return (
    <div className="flex items-center gap-2 text-[11px]">
      <span className="w-4 shrink-0 text-center">
        {running ? (
          <span className="inline-block h-3 w-3 animate-spin rounded-full border border-text-faint border-t-accent align-middle" />
        ) : (
          <span className={`font-bold ${color}`}>
            {tone === "ok" ? "✓" : tone === "warn" ? "⚠" : tone === "bad" ? "✕" : "•"}
          </span>
        )}
      </span>
      <span className="shrink-0 font-semibold text-[var(--text)]">{label}</span>
      <span className="truncate text-text-dim">
        {running
          ? t("web_launch.testing_now")
          : code
            ? t(`web_launch.code_${code}`)
            : t("web_launch.test_not_run")}
      </span>
    </div>
  );
}

export function WebLaunchTab() {
  const { t } = useTranslation();
  const setConfig = useSetConfig();
  const gamePath = useConfigStore((s) => s.config?.gamePath ?? "");
  const [status, setStatus] = useState<WebLaunchStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [toggling, setToggling] = useState(false);
  const [toggleMsg, setToggleMsg] = useState<string | null>(null);
  const [testing, setTesting] = useState<null | "game" | "gamania">(null);
  const [gameCode, setGameCode] = useState<WebLaunchTestCode | undefined>();
  const [gamaniaCode, setGamaniaCode] = useState<WebLaunchTestCode | undefined>();

  const refresh = useCallback(async () => {
    setChecking(true);
    try {
      setStatus(await commands.getWebLaunchStatus());
    } catch {
      /* keep previous */
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    let alive = true;
    commands
      .getWebLaunchStatus()
      .then((s) => {
        if (alive) setStatus(s);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [gamePath]);

  async function handleToggle() {
    if (!status) return;
    const next = !status.registered;
    setToggling(true);
    setToggleMsg(null);
    try {
      await commands.setWebLaunchIntercept(next);
      setToggleMsg(next ? t("web_launch.enabled_msg") : t("web_launch.disabled_msg"));
    } catch (e) {
      setToggleMsg(
        `${t("web_launch.toggle_failed")}: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setToggling(false);
      await refresh();
    }
  }

  async function handleTest() {
    setGameCode(undefined);
    setGamaniaCode(undefined);
    try {
      setTesting("game");
      setGameCode(await commands.webLaunchTestGame());
      setTesting("gamania");
      setGamaniaCode(await commands.webLaunchTestGamania());
    } catch {
      /* ignore */
    } finally {
      setTesting(null);
      await refresh();
    }
  }

  async function handleDetect() {
    try {
      const path = await commands.detectGamePath();
      if (path) setConfig.mutate({ key: "gamePath", value: path });
    } catch {
      /* ignore */
    }
  }
  async function handleBrowse() {
    const path = await commands.openFileDialog();
    if (path) setConfig.mutate({ key: "gamePath", value: path });
  }

  const envReady =
    !!status && status.gamePathOk && status.lrReady && status.gamaniaInstalled && status.exeNameOk;
  const enabled = !!status?.registered;

  // Banner: three distinct states so an all-green env with the switch OFF no
  // longer wrongly says "fix the red items below".
  const banner = !envReady
    ? { tone: "warn", icon: "⚠️", text: t("web_launch.not_ready") }
    : enabled
      ? { tone: "ok", icon: "✅", text: t("web_launch.ready_enabled") }
      : { tone: "info", icon: "🔹", text: t("web_launch.ready_disabled") };

  return (
    <div className="flex flex-col gap-5">
      {/* Intro */}
      <p className="text-[11px] leading-relaxed text-text-dim">{t("web_launch.intro")}</p>

      {/* Overall status banner */}
      <div
        className={`flex items-center gap-2 rounded-[10px] border px-4 py-2.5 text-[12px] font-semibold ${
          banner.tone === "ok"
            ? "border-[rgba(34,197,94,0.3)] bg-[rgba(34,197,94,0.06)] text-green-500"
            : banner.tone === "warn"
              ? "border-[rgba(234,179,8,0.3)] bg-[rgba(234,179,8,0.06)] text-yellow-500"
              : "border-[rgba(59,130,246,0.3)] bg-[rgba(59,130,246,0.06)] text-blue-400"
        }`}
      >
        <span>{banner.icon}</span>
        {banner.text}
      </div>

      {/* Enable toggle — the primary action, kept up top */}
      <div
        className={`flex items-center justify-between rounded-[12px] border px-4 py-3.5 transition-colors ${
          enabled
            ? "border-[rgba(232,162,58,0.4)] bg-[rgba(232,162,58,0.06)]"
            : "border-[var(--tb-border)] bg-[var(--tb-card)]"
        }`}
      >
        <div className="flex min-w-0 flex-col pr-3">
          <span className="text-[13px] font-semibold text-[var(--text)]">
            {t("web_launch.enable")}
          </span>
          <span className="text-[11px] leading-relaxed text-text-dim">
            {enabled ? t("web_launch.enable_on_hint") : t("web_launch.enable_off_hint")}
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          disabled={toggling || !status}
          onClick={handleToggle}
          className={`relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-50 ${
            enabled ? "bg-accent" : "bg-[var(--border)]"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
              enabled ? "translate-x-5" : "translate-x-0"
            }`}
          />
        </button>
      </div>
      {toggleMsg && <p className="-mt-3 px-1 text-[11px] text-text-dim">{toggleMsg}</p>}

      {/* Self-check list */}
      <div>
        <div className="mb-2 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
          {t("web_launch.section_check")}
        </div>
        <div className="flex flex-col gap-2">
          {/* App exe name */}
          <CheckRow
            state={status?.exeNameOk ? "ok" : "bad"}
            title={t("web_launch.check_exe")}
            detail={
              status?.exeNameOk
                ? status.exeName
                : `${status?.exeName ?? "?"} — ${t("web_launch.check_exe_bad")}`
            }
          />

          {/* Game path */}
          <CheckRow
            state={status?.gamePathOk ? "ok" : "bad"}
            title={t("web_launch.check_game_path")}
            detail={
              status?.gamePathOk
                ? status.gamePath
                : gamePath
                  ? t("web_launch.check_game_path_bad")
                  : t("web_launch.check_game_path_missing")
            }
            action={
              !status?.gamePathOk && (
                <div className="flex shrink-0 gap-1.5">
                  <button
                    onClick={handleDetect}
                    className="rounded-lg border border-border px-2.5 py-1 text-[11px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
                  >
                    {t("web_launch.detect")}
                  </button>
                  <button
                    onClick={handleBrowse}
                    className="rounded-lg border border-border px-2.5 py-1 text-[11px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
                  >
                    {t("settings.browse")}
                  </button>
                </div>
              )
            }
          />

          {/* LR */}
          <CheckRow
            state={status?.lrReady ? "ok" : "bad"}
            title={t("web_launch.check_lr")}
            detail={status?.lrReady ? t("web_launch.check_lr_ok") : t("web_launch.check_lr_bad")}
          />

          {/* Gamania launcher */}
          <CheckRow
            state={status?.gamaniaInstalled ? "ok" : "bad"}
            title={t("web_launch.check_gamania")}
            detail={
              status?.gamaniaInstalled
                ? t("web_launch.check_gamania_ok")
                : t("web_launch.check_gamania_bad")
            }
          />
        </div>
      </div>

      {/* Network / DNS */}
      <DnsCheck />

      {/* Live launch test */}
      <div>
        <div className="mb-2 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
          {t("web_launch.section_test")}
        </div>
        <div className="flex flex-col gap-2.5 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
          <p className="text-[11px] leading-relaxed text-text-dim">{t("web_launch.test_desc")}</p>
          <div className="flex flex-col gap-1.5 border-t border-[var(--tb-border)] pt-2.5">
            <TestRow
              label={t("web_launch.test_game_label")}
              running={testing === "game"}
              code={gameCode}
            />
            <TestRow
              label={t("web_launch.test_gamania_label")}
              running={testing === "gamania"}
              code={gamaniaCode}
            />
          </div>
          <button
            onClick={handleTest}
            disabled={testing !== null}
            className="mt-0.5 self-start rounded-lg bg-gradient-to-br from-accent to-[#c47a1a] px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 active:scale-95 disabled:opacity-50"
          >
            {testing !== null
              ? testing === "game"
                ? t("web_launch.testing_game")
                : t("web_launch.testing_gamania")
              : t("web_launch.test_run")}
          </button>
        </div>
      </div>

      {/* Re-check + hint */}
      <div className="flex items-center justify-between">
        <p className="max-w-[70%] text-[11px] leading-relaxed text-text-faint">
          {t("web_launch.hint")}
        </p>
        <button
          onClick={refresh}
          disabled={checking}
          className="shrink-0 rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-50"
        >
          {checking ? t("web_launch.rechecking") : t("web_launch.recheck")}
        </button>
      </div>
    </div>
  );
}
