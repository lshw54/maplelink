import { useTranslation } from "../../lib/i18n";
import { Modal } from "../../components/Modal";

export type EnterChoice = "copy" | "otp";

interface EnterActionPromptProps {
  isOpen: boolean;
  onChoose: (choice: EnterChoice) => void;
  onClose: () => void;
}

/**
 * Asked the first time Enter is pressed on a selected game account: should it
 * copy the account ID, as it always used to, or fetch the one-time password?
 * The answer is kept, and the settings page can change it later.
 */
export function EnterActionPrompt({ isOpen, onChoose, onClose }: EnterActionPromptProps) {
  const { t } = useTranslation();
  return (
    <Modal isOpen={isOpen} onClose={onClose} title={t("launcher.enter_prompt.title")}>
      <div className="flex flex-col gap-3">
        <p className="text-xs leading-relaxed text-text-dim">{t("launcher.enter_prompt.body")}</p>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => onChoose("otp")}
            className="rounded-lg bg-gradient-to-br from-accent to-[var(--accent-dark)] px-4 py-2 text-[12px] font-semibold text-[var(--on-accent)] transition-opacity hover:opacity-90"
          >
            {t("launcher.enter_prompt.otp")}
          </button>
          <button
            type="button"
            onClick={() => onChoose("copy")}
            className="rounded-lg border border-border px-4 py-2 text-[12px] font-semibold text-text-dim transition-colors hover:border-accent hover:text-accent"
          >
            {t("launcher.enter_prompt.copy")}
          </button>
        </div>
        <p className="text-[11px] text-text-faint">{t("launcher.enter_prompt.hint")}</p>
      </div>
    </Modal>
  );
}
