import { useTranslation } from "../../lib/i18n";
import { useUiStore } from "../../lib/stores/ui-store";
import { commands } from "../../lib/tauri";

export function QrViewerPage() {
  const { t } = useTranslation();
  const qrImage = useUiStore((s) => s.qrViewerImage);

  function handleClose() {
    useUiStore.getState().qrViewerImage = null;
    commands.resizeWindow("login").catch(() => {});
    useUiStore.setState({ currentPage: "login" });
  }

  if (!qrImage) {
    handleClose();
    return null;
  }

  return (
    <div className="flex h-full flex-col items-center justify-center bg-[var(--bg)] p-6">
      {/* Close button */}
      <button
        onClick={handleClose}
        className="absolute right-4 top-10 flex h-8 w-8 items-center justify-center rounded-full bg-[var(--surface)] text-text-dim transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <path d="M3 3L9 9M9 3L3 9" />
        </svg>
      </button>

      <div className="mb-4 text-[12px] uppercase tracking-[4px] text-text-dim">
        {t("login.qr.title")}
      </div>

      <div className="rounded-2xl bg-white p-6 shadow-[0_4px_24px_rgba(0,0,0,0.3)]">
        <img
          src={qrImage}
          alt="QR Code"
          style={{ width: 350, height: 350, imageRendering: "pixelated" }}
          draggable={false}
        />
      </div>

      <div className="mt-3 text-[11px] text-text-faint">{t("login.qr.instruction")}</div>
    </div>
  );
}
