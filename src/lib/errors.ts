/**
 * Turn whatever a rejected promise carried into something worth showing.
 *
 * Tauri commands reject with an `ErrorDto` — a plain object, not an `Error` —
 * so `String(e)` on one yields `[object Object]`. That is what players were
 * seeing in the client manager: a red box with no information in it, and
 * nothing anyone could act on or report.
 *
 * The code is appended when there is one. It is noise to read, but it is the
 * part that makes a screenshot useful: it says which command failed without
 * anyone having to reproduce it.
 */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") {
    const text = err.trim();
    if (text) return text;
  } else if (err instanceof Error) {
    if (err.message) return err.message;
  } else if (typeof err === "object" && err !== null) {
    const shape = err as Record<string, unknown>;
    const message = typeof shape.message === "string" ? shape.message.trim() : "";
    const code = typeof shape.code === "string" ? shape.code.trim() : "";
    const details = typeof shape.details === "string" ? shape.details.trim() : "";

    if (message && code) return `${message} (${code})`;
    if (message) return message;
    if (code) return details ? `${code}: ${details}` : code;

    // Some other object. Its JSON is ugly but it is information, which
    // `[object Object]` is not.
    try {
      const json = JSON.stringify(err);
      // `{}` included: an empty object says nothing, but it says it honestly,
      // and it is distinguishable from the `[object Object]` this replaces.
      if (json) return json;
    } catch {
      /* circular, or something that will not serialise */
    }
  }
  return String(err);
}
