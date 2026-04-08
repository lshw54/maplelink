import { useCallback } from "react";
import { useTranslation } from "../i18n";
import { useErrorToastStore } from "../stores/error-toast-store";
import type { ErrorDto } from "../types";

const CRITICAL_CATEGORIES = new Set(["authentication", "process"]);

/**
 * Returns a handler that translates an ErrorDto into a localized toast.
 * Critical errors (auth, process) persist; others auto-dismiss after 5s.
 */
export function useErrorHandler() {
  const { t } = useTranslation();
  const addToast = useErrorToastStore((s) => s.addToast);

  return useCallback(
    (error: ErrorDto) => {
      const message = t(`errors.${error.code}`, {
        path: error.details ?? "",
      });
      const fallback = message === `errors.${error.code}` ? error.message : message;
      const critical = CRITICAL_CATEGORIES.has(error.category);

      addToast({
        message: fallback,
        category: error.category,
        critical,
      });
    },
    [t, addToast],
  );
}
