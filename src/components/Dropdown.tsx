import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** Height of one option row, for choosing which side of the button to open on. */
const ROW = 28;

interface Place {
  top?: number;
  bottom?: number;
  right: number;
  minWidth: number;
}

/**
 * A select drawn by the app instead of by the webview.
 *
 * Use this rather than `<select>`. WebView2 draws a native select's open list
 * itself, in light colours, whatever the page says — `color-scheme: dark` on
 * the document changes the closed control and nothing else — so on the dark
 * theme the list comes up white with the page's pale text on it.
 *
 * The menu is drawn into `document.body` at a fixed position, so neither a
 * card that clips its content for rounded corners nor a scrolling pane can cut
 * it off. It closes on an outside click, Escape, scrolling or resizing.
 */
export function Dropdown<T extends string>({
  value,
  options,
  onChange,
  disabled = false,
  size = "md",
  title,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  disabled?: boolean;
  /** `sm` fits a line of 10px text, `md` a settings row. */
  size?: "sm" | "md";
  title?: string;
}) {
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<Place>({ right: 0, minWidth: 0 });
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const current = options.find((o) => o.value === value);

  function toggle() {
    if (open) {
      setOpen(false);
      return;
    }
    const r = buttonRef.current?.getBoundingClientRect();
    if (!r) return;
    const right = window.innerWidth - r.right;
    const minWidth = Math.max(r.width, size === "sm" ? 80 : 150);
    const needed = options.length * ROW + 8;
    const below = window.innerHeight - r.bottom;
    // Below unless it does not fit there and there is more room above.
    setPlace(
      below >= needed + 8 || below >= r.top
        ? { top: r.bottom + 4, right, minWidth }
        : { bottom: window.innerHeight - r.top + 4, right, minWidth },
    );
    setOpen(true);
  }

  // A control disabled while its menu is open (a job starting) closes it.
  const shown = open && !disabled;

  useEffect(() => {
    if (!shown) return;
    const close = () => setOpen(false);
    // The menu is not inside the button's subtree, so "outside" has to mean
    // outside both.
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (buttonRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    // A fixed menu stays where it opened while its button scrolls away, so any
    // scroll or resize closes it instead of leaving it floating.
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [shown]);

  const button =
    size === "sm"
      ? "gap-1 rounded border bg-[var(--surface)] px-1.5 py-px text-[10px] font-semibold text-text-dim"
      : "gap-1.5 rounded-md border px-2.5 py-1 text-[11px] text-[var(--text)]";

  return (
    <div className="relative shrink-0">
      <button
        ref={buttonRef}
        type="button"
        onClick={toggle}
        disabled={disabled}
        title={title}
        aria-haspopup="listbox"
        aria-expanded={shown}
        className={`flex items-center border-[var(--tb-border)] transition-colors hover:border-accent disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:border-[var(--tb-border)] ${button}`}
      >
        {current?.label ?? value}
        <svg
          width={size === "sm" ? 8 : 10}
          height={size === "sm" ? 8 : 10}
          viewBox="0 0 12 12"
          fill="none"
          className="text-text-dim"
        >
          <path
            d="M3 4.5L6 7.5L9 4.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {shown &&
        createPortal(
          <div
            ref={menuRef}
            role="listbox"
            style={{ position: "fixed", ...place }}
            // Under the main window's title bar (z-200), which stays on top.
            className="z-[190] overflow-hidden rounded-lg border border-[var(--tb-border)] bg-[var(--tb-card)] py-1 shadow-[0_10px_30px_rgba(0,0,0,0.35)]"
          >
            {options.map((o) => (
              <button
                key={o.value}
                type="button"
                role="option"
                aria-selected={o.value === value}
                onClick={() => {
                  onChange(o.value);
                  setOpen(false);
                }}
                className={`block w-full px-3 py-1.5 text-left text-[11px] transition-colors hover:bg-[var(--surface-hover)] ${
                  o.value === value ? "font-semibold text-accent" : "text-[var(--text)]"
                }`}
              >
                {o.label}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
