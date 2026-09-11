import { useEffect } from "react";
import { create } from "zustand";

/**
 * How many full-window overlays are on screen.
 *
 * The app draws its own title bar, so dragging the window means clicking a
 * strip of the page. Every modal here covers the whole viewport, which used to
 * cover that strip too — with the announcement, which opens by itself and
 * locks its button for ten seconds, the window simply could not be moved. If
 * it had also opened part-way off the screen there was no way back.
 *
 * So the title bar stays above the overlays and keeps working. It must not
 * become a way to click past a modal, though, so while anything is open it
 * offers only what belongs to the window: dragging, minimise and close.
 */
interface OverlayState {
  count: number;
  push: () => void;
  pop: () => void;
}

const useOverlayStore = create<OverlayState>((set) => ({
  count: 0,
  push: () => set((s) => ({ count: s.count + 1 })),
  pop: () => set((s) => ({ count: Math.max(0, s.count - 1) })),
}));

/**
 * Call from a component that covers the window while it is mounted.
 *
 * `open` lets a component that renders nothing when closed still call the hook
 * unconditionally.
 */
export function useOverlayLayer(open = true): void {
  useEffect(() => {
    if (!open) return;
    const { push, pop } = useOverlayStore.getState();
    push();
    return pop;
  }, [open]);
}

/** True while any overlay is on screen. */
export function useOverlayOpen(): boolean {
  return useOverlayStore((s) => s.count > 0);
}

/** Test seam: the count, without a React subscription. */
export function overlayCount(): number {
  return useOverlayStore.getState().count;
}
