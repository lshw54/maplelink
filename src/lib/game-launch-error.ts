import { errorMessage } from "./errors";
import { getTranslation } from "./i18n";
import { useErrorToastStore } from "./stores/error-toast-store";
import { useUiStore } from "./stores/ui-store";
import { commands } from "./tauri";

const VC_RUNTIME_DOWNLOAD_URL =
  "https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/";

/** Keep the runtime error visible so the player can open Microsoft's download page. */
export function reportGameLaunchError(error: unknown, runtimeOnly = false) {
  const missingRuntime =
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "PROC_MISSING_VC_RUNTIME";
  const toasts = useErrorToastStore.getState();
  if (!missingRuntime) {
    if (!runtimeOnly) {
      toasts.addToast({ message: errorMessage(error), category: "process", critical: false });
    }
    return;
  }

  const language = useUiStore.getState().language;
  toasts.clearCategory("vc-runtime");
  toasts.addToast({
    message: getTranslation(language, "errors.PROC_MISSING_VC_RUNTIME"),
    category: "vc-runtime",
    critical: true,
    action: {
      label: getTranslation(language, "errors.vc_runtime_download"),
      onClick: () => commands.openExternal(VC_RUNTIME_DOWNLOAD_URL).catch(() => {}),
    },
  });
}
