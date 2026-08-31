import type { BrowserConnectionInfo } from "../../lib/types";

/**
 * What the padlock opens: who answered for this page, and with what
 * certificate.
 *
 * It is drawn inside the toolbar webview, which grows to make room — see
 * `CHROME_HEIGHT` in `services/browser_window.rs` for why a panel cannot simply
 * hang below the bar.
 */
export function ConnectionPanel({
  info,
  t,
  onClose,
}: {
  /** Null while the handshake is still in flight. */
  info: BrowserConnectionInfo | null;
  t: (key: string) => string;
  onClose: () => void;
}) {
  const cert = info?.certificate ?? null;

  return (
    <div className="min-h-0 flex-1 overflow-y-auto border-b border-[var(--tb-border)] bg-[var(--tb-card)] px-4 py-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[12px] font-semibold text-[var(--text)]">
          {info
            ? t(info.encrypted ? "browser.connection.secure" : "browser.connection.not_secure")
            : t("browser.connection.checking")}
        </span>
        <button
          onClick={onClose}
          className="rounded px-2 py-0.5 text-[11px] text-[var(--text-dim)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
        >
          {t("browser.connection.close")}
        </button>
      </div>

      {info && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[11px]">
          <Row label={t("browser.connection.host")} value={`${info.host}:${info.port}`} />
          {cert && (
            <>
              <Row label={t("browser.connection.issued_to")} value={cert.subject} />
              <Row label={t("browser.connection.issued_by")} value={cert.issuer} />
              <Row
                label={t("browser.connection.valid")}
                value={`${formatDate(cert.validFrom)} — ${formatDate(cert.validTo)}`}
              />
              <Row label={t("browser.connection.serial")} value={cert.serial} mono />
              <Row label={t("browser.connection.fingerprint")} value={cert.fingerprint} mono />
            </>
          )}
          {info.error && <Row label={t("browser.connection.failed")} value={info.error} />}
        </dl>
      )}

      <p className="mt-3 text-[10px] leading-relaxed text-[var(--text-faint)]">
        {t("browser.connection.note")}
      </p>
    </div>
  );
}

function Row({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <>
      <dt className="whitespace-nowrap text-[var(--text-dim)]">{label}</dt>
      <dd
        className={`break-all text-[var(--text)] ${mono ? "font-mono text-[10px] tracking-tight" : ""}`}
      >
        {value}
      </dd>
    </>
  );
}

/** The backend sends RFC 2822; show it however the user's machine writes dates. */
function formatDate(value: string): string {
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? value : new Date(parsed).toLocaleDateString();
}
