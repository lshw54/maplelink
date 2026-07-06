import { useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import type { DnsStatus, DnsTestResult } from "../../lib/types";

/** Network / DNS self-check + one-click switch to Alibaba public DNS. */
export function DnsCheck() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<DnsStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [test, setTest] = useState<DnsTestResult | null>(null);

  async function load() {
    setLoading(true);
    try {
      setStatus(await commands.getDnsStatus());
    } catch {
      /* keep previous */
    } finally {
      setLoading(false);
    }
  }

  // Initial load — set state only inside the promise (no synchronous setState
  // in the effect body).
  useEffect(() => {
    let alive = true;
    commands
      .getDnsStatus()
      .then((s) => {
        if (alive) setStatus(s);
      })
      .catch(() => {})
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  async function change(op: "set" | "reset") {
    setWorking(true);
    setMsg(null);
    try {
      if (op === "set") await commands.setRecommendedDns();
      else await commands.resetDnsAuto();
      setMsg(t("web_launch.dns_changed"));
    } catch (e) {
      const s = e instanceof Error ? e.message : String(e);
      setMsg(s.includes("cancelled") ? t("web_launch.dns_cancelled") : t("web_launch.dns_failed"));
    } finally {
      setWorking(false);
      await load();
    }
  }

  async function runTest() {
    setTesting(true);
    setTest(null);
    try {
      setTest(await commands.testDns());
    } catch {
      /* ignore */
    } finally {
      setTesting(false);
    }
  }

  const region =
    status && (status.publicIp || status.countryCode)
      ? `${status.publicIp || "?"} · ${status.countryCode || "?"}`
      : t("web_launch.dns_unknown");
  const recommend = !!status && status.isChina && !status.usingRecommended;

  return (
    <div>
      <div className="mb-2 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
        {t("web_launch.section_dns")}
      </div>
      <div className="flex flex-col gap-2.5 rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
        {/* Info rows */}
        <div className="flex items-center justify-between text-[11px]">
          <span className="text-text-dim">{t("web_launch.dns_ip")}</span>
          <span className="font-mono text-[var(--text)]">{loading ? "…" : region}</span>
        </div>
        <div className="flex items-center justify-between text-[11px]">
          <span className="text-text-dim">{t("web_launch.dns_current")}</span>
          <span className="font-mono text-[var(--text)]">
            {loading ? "…" : status?.currentDns.join(", ") || t("web_launch.dns_auto")}
          </span>
        </div>

        {/* Recommendation for China IPs */}
        {recommend && (
          <p className="rounded-md bg-[rgba(234,179,8,0.08)] px-2.5 py-1.5 text-[11px] leading-relaxed text-yellow-500">
            {t("web_launch.dns_recommend")}
          </p>
        )}

        {/* Test result */}
        {test && (
          <div className="flex flex-col gap-1 border-t border-[var(--tb-border)] pt-2 text-[11px]">
            <TestLine ok={test.beanfunOk} label="login.beanfun.com" />
            <TestLine ok={test.googleOk} label="www.google.com (reCAPTCHA)" />
          </div>
        )}

        {/* Actions */}
        <div className="flex flex-wrap items-center gap-2 pt-0.5">
          {status?.usingRecommended ? (
            <button
              onClick={() => change("reset")}
              disabled={working}
              className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-50"
            >
              {working ? t("web_launch.dns_working") : t("web_launch.dns_reset")}
            </button>
          ) : (
            <button
              onClick={() => change("set")}
              disabled={working}
              className={`rounded-lg px-3 py-1.5 text-[11px] font-semibold text-white transition-opacity hover:opacity-90 active:scale-95 disabled:opacity-50 ${
                recommend
                  ? "bg-gradient-to-br from-accent to-[#c47a1a]"
                  : "bg-[var(--border)] text-text-dim"
              }`}
            >
              {working ? t("web_launch.dns_working") : t("web_launch.dns_switch")}
            </button>
          )}
          <button
            onClick={runTest}
            disabled={testing}
            className="rounded-lg border border-border px-3 py-1.5 text-[11px] font-semibold text-text-dim transition-colors hover:bg-[var(--surface-hover)] disabled:opacity-50"
          >
            {testing ? t("web_launch.dns_testing") : t("web_launch.dns_test")}
          </button>
          {msg && <span className="text-[11px] text-text-dim">{msg}</span>}
        </div>
      </div>
    </div>
  );
}

function TestLine({ ok, label }: { ok: boolean; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <span
        className={`w-3 shrink-0 text-center font-bold ${ok ? "text-green-500" : "text-red-500"}`}
      >
        {ok ? "✓" : "✕"}
      </span>
      <span className="font-mono text-text-dim">{label}</span>
    </div>
  );
}
