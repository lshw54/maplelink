import { useEffect, type RefObject } from "react";

/**
 * Call `onClose` on any mousedown outside `ref`.
 *
 * `deferred` registers the listener a tick late, so the click that opened a
 * menu does not immediately close it. `active: false` skips the listener
 * entirely (for a dropdown that is not open).
 */
export function useClickOutside(
  ref: RefObject<HTMLElement | null>,
  onClose: () => void,
  { deferred = false, active = true }: { deferred?: boolean; active?: boolean } = {},
) {
  useEffect(() => {
    if (!active) return;
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    }
    const timer = deferred
      ? setTimeout(() => document.addEventListener("mousedown", handleClickOutside), 16)
      : (document.addEventListener("mousedown", handleClickOutside), null);
    return () => {
      if (timer !== null) clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [ref, onClose, deferred, active]);
}
