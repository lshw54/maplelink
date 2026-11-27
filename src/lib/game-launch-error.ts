import { errorMessage } from "./errors";
import { writeConfig } from "./hooks/use-config";
import { getTranslation } from "./i18n";
import { useErrorToastStore } from "./stores/error-toast-store";
import { useUiStore } from "./stores/ui-store";
import { commands } from "./tauri";

const VC_RUNTIME_DOWNLOAD_URL =
  "https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/";

/** Keep launch prerequisites visible with an action the player can take. */
export function reportGameLaunchError(error: unknown, prerequisitesOnly = false) {
  const code =
    typeof error === "object" && error !== null && "code" in error ? error.code : undefined;
  const toasts = useErrorToastStore.getState();
  const language = useUiStore.getState().language;

  if (code === "PROC_GAME_PATH_NOT_FOUND") {
    toasts.clearCategory("game-path");
    const toastId = toasts.addToast({
      message: getTranslation(language, "errors.PROC_GAME_PATH_NOT_FOUND"),
      category: "game-path",
      critical: true,
      action: {
        label: getTranslation(language, "errors.choose_game_path"),
        onClick: async () => {
          try {
            const path = await commands.openFileDialog();
            if (path) {
              await writeConfig("gamePath", path);
              toasts.removeToast(toastId);
            }
          } catch (err) {
            toasts.addToast({ message: errorMessage(err), category: "process", critical: false });
          }
        },
      },
    });
    return;
  }

  if (code !== "PROC_MISSING_VC_RUNTIME") {
    if (!prerequisitesOnly) {
      toasts.addToast({ message: errorMessage(error), category: "process", critical: false });
    }
    return;
  }

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
