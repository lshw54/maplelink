import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Modal } from "../../components/Modal";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { useAuthStore } from "../../lib/stores/auth-store";
import { useErrorToastStore } from "../../lib/stores/error-toast-store";
import { errorMessage } from "../../lib/errors";

/** beanfun's handler returns the contract as newline-separated plain text.
 *  Tags are stripped anyway in case the HK handler ever wraps it in markup,
 *  and runs of blank lines are collapsed so it reads as a document. */
function contractToText(html: string): string {
  const withBreaks = html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/(p|div|li|h\d|tr)>/gi, "\n")
    .replace(/<\/?[^>]+>/g, "");
  const doc = new DOMParser().parseFromString(withBreaks, "text/html");
  return (doc.body.textContent ?? "")
    .split("\n")
    .map((l) => l.trim())
    .filter((l, i, arr) => l !== "" || (i > 0 && arr[i - 1] !== ""))
    .join("\n")
    .trim();
}

type View = "form" | "contract";

/**
 * "Add game account" — the button the original launcher had next to the
 * account list. Asks for a display name and agreement to the game's terms,
 * then creates the sub-account through beanfun's gamezone handler.
 */
export function AddServiceAccountDialog({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const addToast = useErrorToastStore((s) => s.addToast);

  const [view, setView] = useState<View>("form");
  const [name, setName] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [contract, setContract] = useState<string | null>(null);
  const [contractError, setContractError] = useState<string | null>(null);
  const [contractLoading, setContractLoading] = useState(false);

  function reset() {
    setView("form");
    setName("");
    setAgreed(false);
    setWorking(false);
    setError(null);
  }

  function handleClose() {
    if (working) return;
    reset();
    onClose();
  }

  async function showContract() {
    setView("contract");
    if (contract || contractLoading) return;
    setContractError(null);
    setContractLoading(true);
    try {
      const text = await commands.getServiceContract(useAuthStore.getState().activeSessionId ?? "");
      setContract(text ? contractToText(text) : "");
    } catch (e) {
      // Surface beanfun's / the bridge's own error so a refusal is diagnosable.
      setContractError(errorMessage(e));
      setContract("");
    } finally {
      setContractLoading(false);
    }
  }

  async function handleCreate() {
    const displayName = name.trim();
    if (!displayName) {
      setError(t("launcher.add_account.need_name"));
      return;
    }
    if (!agreed) {
      setError(t("launcher.add_account.need_agree"));
      return;
    }
    setError(null);
    setWorking(true);
    try {
      const result = await commands.addServiceAccount(
        useAuthStore.getState().activeSessionId ?? "",
        displayName,
      );
      if (!result.success) {
        setError(result.message || t("launcher.add_account.failed"));
        return;
      }
      // The backend already re-fetched the list; invalidating re-reads it.
      await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
      await queryClient.invalidateQueries({ queryKey: ["accountLimit"] });
      addToast({
        message: t("launcher.add_account.success"),
        category: "success",
        critical: false,
      });
      reset();
      onClose();
    } catch {
      setError(t("launcher.add_account.failed"));
    } finally {
      setWorking(false);
    }
  }

  const title =
    view === "contract" ? t("launcher.add_account.terms") : t("launcher.add_account.title");

  return (
    <Modal isOpen={isOpen} onClose={handleClose} title={title}>
      {view === "contract" ? (
        <div className="flex flex-col gap-3">
          <div className="max-h-[50vh] overflow-y-auto rounded-lg border border-[var(--tb-border)] bg-[var(--bg)] px-3 py-2 text-[11px] leading-relaxed whitespace-pre-wrap text-[var(--text-dim)]">
            {contractLoading
              ? t("app.loading")
              : contract
                ? contract
                : t("launcher.add_account.terms_unavailable")}
          </div>
          {contractError && (
            <div className="text-[11px] break-all text-red-400">{contractError}</div>
          )}
          <div className="flex justify-end">
            <button
              onClick={() => setView("form")}
              className="rounded-lg border border-border px-3 py-1.5 text-[12px] text-[var(--text)] transition-colors hover:bg-[var(--surface-hover)]"
            >
              {t("launcher.add_account.back")}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          <label className="text-[12px] text-[var(--text-dim)]">
            {t("launcher.add_account.prompt")}
          </label>
          <input
            autoFocus
            name="new-display-name"
            autoComplete="off"
            data-form-type="other"
            value={name}
            disabled={working}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreate();
            }}
            className="rounded-lg border border-[var(--tb-border)] bg-[var(--bg)] px-3 py-2 text-xs text-[var(--text)] transition-colors outline-none focus:border-accent disabled:opacity-50"
          />
          <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-text-dim select-none">
            <input
              type="checkbox"
              name="agree-terms"
              checked={agreed}
              disabled={working}
              onChange={(e) => setAgreed(e.target.checked)}
              className="h-3.5 w-3.5 accent-accent"
            />
            <span>
              {t("launcher.add_account.agree")}{" "}
              <button
                type="button"
                onClick={(e) => {
                  e.preventDefault();
                  showContract();
                }}
                className="text-accent hover:underline"
              >
                {t("launcher.add_account.terms")}
              </button>
            </span>
          </label>
          {error && <div className="text-center text-[12px] text-red-400">{error}</div>}
          <div className="flex justify-end gap-2">
            <button
              onClick={handleClose}
              disabled={working}
              className="rounded-lg px-3 py-1.5 text-[12px] text-[var(--text-dim)] transition-colors hover:bg-[rgba(255,255,255,0.05)] disabled:opacity-50"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={handleCreate}
              disabled={working}
              className="rounded-lg bg-accent px-3 py-1.5 text-[12px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90 disabled:opacity-50"
            >
              {working ? t("launcher.add_account.creating") : t("launcher.add_account.title")}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
