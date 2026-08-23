import { useEffect, useRef, useState, type ReactNode } from "react";

/**
 * Shared building blocks for the toolbox tabs — the grouped-list look the
 * About tab established: a small caption, then a rounded container whose rows
 * are separated by hairlines. One set of sizes for the full and compact UI.
 */

/** A titled group of rows. */
export function Section({
  title,
  children,
  hint,
}: {
  title?: string;
  children: ReactNode;
  /** Optional caption text under the whole group. */
  hint?: string;
}) {
  return (
    <section className="flex flex-col gap-1.5">
      {title && (
        <h3 className="px-1 text-[10px] font-semibold tracking-[2px] text-text-faint uppercase">
          {title}
        </h3>
      )}
      <div className="overflow-hidden rounded-[10px] border border-[var(--tb-border)] bg-[var(--tb-card)] [&>*+*]:border-t [&>*+*]:border-[var(--tb-border)]">
        {children}
      </div>
      {hint && <p className="px-1 text-[10.5px] leading-relaxed text-text-faint">{hint}</p>}
    </section>
  );
}

/** One row: label (with an optional one-line hint under it) and a control. */
export function Row({
  label,
  hint,
  children,
  onClick,
}: {
  label: ReactNode;
  hint?: ReactNode;
  children?: ReactNode;
  /** Makes the whole row a button (used for link-like rows). */
  onClick?: () => void;
}) {
  const inner = (
    <>
      <div className="min-w-0 flex-1">
        <div className="text-[11.5px] font-medium text-[var(--text)]">{label}</div>
        {hint && <div className="mt-0.5 text-[10.5px] leading-snug text-text-faint">{hint}</div>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </>
  );
  const cls = "flex w-full items-center gap-4 px-3.5 py-2.5 text-left";
  return onClick ? (
    <button
      onClick={onClick}
      className={`${cls} transition-colors hover:bg-[var(--surface-hover)]`}
    >
      {inner}
    </button>
  ) : (
    <div className={cls}>{inner}</div>
  );
}

/** Small segmented control (theme / language / channel …). */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div className="flex overflow-hidden rounded-md border border-[var(--tb-border)]">
      {options.map((o, i) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={`px-2.5 py-1 text-[11px] font-semibold whitespace-nowrap transition-all outline-none active:scale-95 ${
            i < options.length - 1 ? "border-r border-[var(--tb-border)]" : ""
          } ${
            value === o.value
              ? "bg-gradient-to-br from-accent to-[var(--accent-dark)] text-[var(--on-accent)]"
              : "bg-transparent text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Small outlined action button for a row's right side. */
export function RowButton({
  children,
  onClick,
  danger,
  title,
}: {
  children: ReactNode;
  onClick: () => void;
  danger?: boolean;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`shrink-0 rounded-md border px-2.5 py-1 text-[11px] font-medium transition-colors ${
        danger
          ? "border-[var(--tb-border)] text-text-dim hover:border-[var(--danger)] hover:text-[var(--danger)]"
          : "border-[var(--tb-border)] text-text-dim hover:bg-[var(--surface-hover)] hover:text-[var(--text)]"
      }`}
    >
      {children}
    </button>
  );
}

/** A value shown on a row's right side (paths, read-only info). */
export function RowValue({ children, mono }: { children: ReactNode; mono?: boolean }) {
  return (
    <span className={`max-w-[220px] truncate text-[11px] text-text-dim ${mono ? "font-mono" : ""}`}>
      {children}
    </span>
  );
}

/** Theme-styled dropdown (the native <select> popup ignores the dark theme). */
export function Dropdown({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const current = options.find((o) => o.value === value);

  return (
    <div ref={ref} className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1.5 rounded-md border border-[var(--tb-border)] px-2.5 py-1 text-[11px] text-[var(--text)] transition-colors hover:border-accent"
      >
        {current?.label ?? value}
        <svg width="10" height="10" viewBox="0 0 12 12" fill="none" className="text-text-dim">
          <path
            d="M3 4.5L6 7.5L9 4.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {open && (
        <div className="absolute right-0 z-20 mt-1 min-w-[150px] overflow-hidden rounded-lg border border-[var(--tb-border)] bg-[var(--tb-card)] shadow-[0_10px_30px_rgba(0,0,0,0.35)]">
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
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
        </div>
      )}
    </div>
  );
}
