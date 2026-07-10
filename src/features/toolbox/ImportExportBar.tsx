import { useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { Modal } from "../shared/Modal";
import { PasswordInput } from "../shared/PasswordInput";

/** Export / import saved accounts + display overrides (plaintext or passphrase-
 *  encrypted). `onImported` re-fetches the account list after a successful import. */
export function ImportExportBar({ onImported }: { onImported: () => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  // Export modal
  const [exportOpen, setExportOpen] = useState(false);
  const [encrypt, setEncrypt] = useState(false);
  const [exportPass, setExportPass] = useState("");

  // Disposal choice (shown after picking a file, before importing)
  const [disposalPath, setDisposalPath] = useState<string | null>(null);
  const [disposal, setDisposal] = useState<"recycle" | "delete" | "keep">("recycle");

  // Import passphrase modal (opened only when the file is encrypted)
  const [importPath, setImportPath] = useState<string | null>(null);
  const [importPass, setImportPass] = useState("");
  const [importErr, setImportErr] = useState<string | null>(null);

  const flash = (ok: boolean, text: string) => {
    setMsg({ ok, text });
    setTimeout(() => setMsg(null), 4000);
  };

  async function doExport() {
    if (encrypt && exportPass.trim().length < 4) {
      flash(false, t("data.pass_too_short"));
      return;
    }
    setBusy(true);
    try {
      const saved = await commands.exportData(encrypt ? exportPass : undefined);
      setExportOpen(false);
      setExportPass("");
      setEncrypt(false);
      if (saved) flash(true, t("data.export_done"));
    } catch {
      flash(false, t("data.export_failed"));
    } finally {
      setBusy(false);
    }
  }

  async function startImport() {
    const path = await commands.openImportDialog().catch(() => null);
    if (path) {
      setDisposal("recycle");
      setDisposalPath(path); // open the disposal-choice modal first
    }
  }

  async function runImport(path: string, disp: "recycle" | "delete" | "keep", passphrase?: string) {
    setBusy(true);
    try {
      const n = await commands.importData(path, disp, passphrase);
      setDisposalPath(null);
      setImportPath(null);
      setImportPass("");
      setImportErr(null);
      onImported();
      flash(true, t("data.import_done", { count: String(n) }));
    } catch (e) {
      const code = (e as { code?: string })?.code;
      if (code === "IMPORT_PASSPHRASE_REQUIRED") {
        setDisposalPath(null); // encrypted → move on to the passphrase prompt
        setImportPath(path);
        setImportErr(null);
      } else if (code === "IMPORT_WRONG_PASSPHRASE") {
        setImportErr(t("data.import_wrong_pass"));
      } else {
        setDisposalPath(null);
        setImportPath(null);
        flash(false, t("data.import_failed"));
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex items-center gap-1.5">
      <button
        onClick={() => setExportOpen(true)}
        disabled={busy}
        title={t("data.export")}
        className="flex items-center gap-1 rounded-lg border border-[var(--tb-border)] bg-[var(--tb-card)] px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent disabled:opacity-50"
      >
        <svg
          width="11"
          height="11"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M6 8V2M6 2L3.5 4.5M6 2l2.5 2.5M2.5 9.5h7" />
        </svg>
        {t("data.export")}
      </button>
      <button
        onClick={startImport}
        disabled={busy}
        title={t("data.import")}
        className="flex items-center gap-1 rounded-lg border border-[var(--tb-border)] bg-[var(--tb-card)] px-2.5 py-1 text-[11px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent disabled:opacity-50"
      >
        <svg
          width="11"
          height="11"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M6 2v6M6 8L3.5 5.5M6 8l2.5-2.5M2.5 9.5h7" />
        </svg>
        {t("data.import")}
      </button>
      {msg && (
        <div
          className={`fixed bottom-4 left-1/2 z-[130] -translate-x-1/2 rounded-lg px-3 py-1.5 text-[12px] font-semibold shadow-lg ${
            msg.ok
              ? "bg-[rgba(34,197,94,0.15)] text-green-500"
              : "bg-[rgba(239,68,68,0.15)] text-red-400"
          }`}
        >
          {msg.text}
        </div>
      )}

      {/* Export options */}
      <Modal
        isOpen={exportOpen}
        onClose={() => setExportOpen(false)}
        title={t("data.export_title")}
      >
        <div className="flex flex-col gap-3">
          <p className="rounded-[10px] border border-[rgba(234,179,8,0.3)] bg-[rgba(234,179,8,0.06)] px-3 py-2 text-[11px] leading-relaxed text-yellow-500">
            {t("data.export_warn")}
          </p>
          <label className="flex cursor-pointer items-center gap-2 text-[12px] text-[var(--text)] select-none">
            <input
              type="checkbox"
              checked={encrypt}
              onChange={(e) => setEncrypt(e.target.checked)}
              className="h-3.5 w-3.5 accent-[var(--accent)]"
            />
            {t("data.export_encrypt")}
          </label>
          {encrypt && (
            <PasswordInput
              value={exportPass}
              onChange={(e) => setExportPass(e.target.value)}
              placeholder={t("data.pass_placeholder")}
              autoComplete="new-password"
              className="w-full rounded-lg border border-border bg-[var(--surface)] py-2 pr-9 pl-3 text-xs text-[var(--text)] outline-none focus:border-accent"
            />
          )}
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setExportOpen(false)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={doExport}
              disabled={busy}
              className="rounded-lg bg-accent px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              {t("data.export_btn")}
            </button>
          </div>
        </div>
      </Modal>

      {/* Disposal choice — how to handle the source file after importing */}
      <Modal
        isOpen={disposalPath !== null}
        onClose={() => setDisposalPath(null)}
        title={t("data.import_title")}
      >
        <div className="flex flex-col gap-3">
          <p className="text-[12px] leading-relaxed text-text-dim">
            {t("data.import_disposal_hint")}
          </p>
          <div className="flex flex-col gap-1.5">
            {(["recycle", "delete", "keep"] as const).map((opt) => (
              <label
                key={opt}
                className={`flex cursor-pointer items-start gap-2 rounded-lg border px-3 py-2 text-[12px] transition-colors select-none ${
                  disposal === opt
                    ? "border-accent bg-[var(--surface-hover)] text-[var(--text)]"
                    : "border-border text-text-dim hover:border-accent/50"
                }`}
              >
                <input
                  type="radio"
                  name="disposal"
                  checked={disposal === opt}
                  onChange={() => setDisposal(opt)}
                  className="mt-0.5 accent-[var(--accent)]"
                />
                <span>{t(`data.disposal_${opt}`)}</span>
              </label>
            ))}
          </div>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setDisposalPath(null)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={() => disposalPath && runImport(disposalPath, disposal)}
              disabled={busy}
              className="rounded-lg bg-accent px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              {t("data.import_btn")}
            </button>
          </div>
        </div>
      </Modal>

      {/* Import passphrase */}
      <Modal
        isOpen={importPath !== null}
        onClose={() => setImportPath(null)}
        title={t("data.import_pass_title")}
      >
        <div className="flex flex-col gap-3">
          <p className="text-[12px] leading-relaxed text-text-dim">{t("data.import_pass_hint")}</p>
          <PasswordInput
            value={importPass}
            onChange={(e) => setImportPass(e.target.value)}
            placeholder={t("data.pass_placeholder")}
            autoComplete="off"
            onKeyDown={(e) => {
              if (e.key === "Enter" && importPath) runImport(importPath, disposal, importPass);
            }}
            autoFocus
            className="w-full rounded-lg border border-border bg-[var(--surface)] py-2 pr-9 pl-3 text-xs text-[var(--text)] outline-none focus:border-accent"
          />
          {importErr && <p className="text-[11px] text-red-400">{importErr}</p>}
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setImportPath(null)}
              className="rounded-lg px-3 py-1.5 text-[12px] text-text-dim transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={() => importPath && runImport(importPath, disposal, importPass)}
              disabled={busy || !importPass}
              className="rounded-lg bg-accent px-4 py-1.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              {t("data.import_btn")}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
