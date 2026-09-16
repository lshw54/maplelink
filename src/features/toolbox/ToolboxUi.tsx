import type { ReactNode } from "react";

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
      {/* min-w keeps a long right-side value (a wrapping path) from crushing
          the label into a one-character-per-line column. */}
      <div className="min-w-[76px] flex-1">
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

/** A value shown on a row's right side (paths, read-only info). Wraps rather
 *  than truncates — a game path is only useful when all of it is readable. */
export function RowValue({ children, mono }: { children: ReactNode; mono?: boolean }) {
  return (
    <span
      title={typeof children === "string" ? children : undefined}
      className={`max-w-[300px] min-w-0 text-right text-[11px] leading-snug break-all text-text-dim ${
        mono ? "font-mono" : ""
      }`}
    >
      {children}
    </span>
  );
}
