import { useMemo, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import type { ClientCheckedFileDto, ClientIssueKind, ClientScanReportDto } from "../../lib/types";

export type Filter = "all" | "issues" | "ok" | "extra";

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = n / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

/**
 * What to call the problem, in the words a player would use.
 *
 * A content mismatch has two very different causes and the file alone cannot
 * tell them apart: the client is a version behind, or the file is damaged. The
 * version marker already answers that for the whole folder, so it decides the
 * wording here — "out of date" when the client is behind, "damaged" when it
 * claims to be the official version and still does not match.
 */
function issueLabel(kind: ClientIssueKind, outdated: boolean): string {
  switch (kind) {
    case "missing":
      return "client.issue_missing";
    case "unreadable":
      return "client.issue_unreadable";
    default:
      return outdated ? "client.issue_outdated" : "client.issue_damaged";
  }
}

const KIND_DETAIL: Record<ClientIssueKind, string> = {
  missing: "client.issue_why_missing",
  sizeMismatch: "client.issue_why_size",
  hashMismatch: "client.issue_why_hash",
  unreadable: "client.issue_why_unreadable",
};

function Chip({
  active,
  onClick,
  label,
  count,
  tone,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  count: number;
  tone?: "ok" | "warn";
}) {
  const colour =
    tone === "ok" ? "text-green-500" : tone === "warn" ? "text-yellow-500" : "text-[var(--text)]";
  return (
    <button
      onClick={onClick}
      className={`rounded-lg border px-2.5 py-1 text-[11px] transition-colors ${
        active
          ? "border-accent bg-[var(--surface-hover)]"
          : "border-[var(--tb-border)] hover:bg-[var(--surface-hover)]"
      }`}
    >
      <span className="text-text-dim">{label}</span>{" "}
      <span className={`font-bold ${colour}`}>{count}</span>
    </button>
  );
}

function FileRow({
  file,
  checked,
  outdated,
  onToggle,
}: {
  file: ClientCheckedFileDto;
  checked: boolean;
  outdated: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const issue = file.kind !== null;
  const kind = file.kind as ClientIssueKind;
  // The row is one line, so the full reason and what will happen to the file
  // live in the tooltip rather than being cut off in the status column.
  const why = issue
    ? `${t(KIND_DETAIL[kind], {
        expected: formatBytes(file.expectedSize),
        local: file.localSize === null ? "—" : formatBytes(file.localSize),
      })} ${t("client.issue_will_replace")}`
    : t("client.issue_why_ok");

  return (
    <div
      // 1263 rows: let the engine skip what is scrolled out of view.
      style={{ contentVisibility: "auto", containIntrinsicSize: "0 30px" }}
      className="flex h-[30px] items-center gap-2.5 border-b border-[var(--tb-border)] px-3 last:border-b-0 hover:bg-[var(--surface-hover)]"
    >
      {issue ? (
        <input
          type="checkbox"
          checked={checked}
          onChange={onToggle}
          className="h-3.5 w-3.5 shrink-0 accent-[var(--accent)]"
        />
      ) : (
        <span className="w-3.5 shrink-0 text-center text-[11px] text-green-500">✓</span>
      )}
      <span
        title={file.path}
        className={`min-w-0 flex-1 truncate font-mono text-[11px] ${
          issue ? "text-[var(--text)]" : "text-text-dim"
        }`}
      >
        {file.path}
      </span>
      <span
        title={why}
        className={`w-24 shrink-0 cursor-help text-right text-[10px] font-semibold ${
          issue ? "text-yellow-500" : "text-text-faint"
        }`}
      >
        {issue ? t(issueLabel(kind, outdated)) : t("client.status_ok")}
      </span>
      <span className="w-16 shrink-0 text-right font-mono text-[10px] text-text-faint">
        {formatBytes(file.expectedSize)}
      </span>
    </div>
  );
}

/** The result of a check: what needs work, and what was left alone. */
export function VerifyPanel({
  report,
  selected,
  setSelected,
  outdated = false,
}: {
  report: ClientScanReportDto | null;
  selected: Set<string>;
  setSelected: (next: Set<string>) => void;
  /** The installed client is behind the official version, per the version marker. */
  outdated?: boolean;
}) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");

  const issues = useMemo(() => report?.files.filter((f) => f.kind !== null) ?? [], [report]);

  const rows = useMemo(() => {
    if (!report) return [];
    const base =
      filter === "issues"
        ? issues
        : filter === "ok"
          ? report.files.filter((f) => f.kind === null)
          : report.files;
    const needle = query.trim().toLowerCase();
    return needle ? base.filter((f) => f.path.toLowerCase().includes(needle)) : base;
  }, [report, issues, filter, query]);

  const extras = useMemo(() => {
    if (!report) return [];
    const needle = query.trim().toLowerCase();
    return needle
      ? report.extraFiles.filter((p) => p.toLowerCase().includes(needle))
      : report.extraFiles;
  }, [report, query]);

  if (!report) {
    return (
      <div className="flex flex-1 items-center justify-center px-6">
        <p className="text-center text-[12px] leading-relaxed text-text-faint">
          {t("client.ready_hint")}
        </p>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 px-6 pb-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Chip
          active={filter === "all"}
          onClick={() => setFilter("all")}
          label={t("client.filter_all")}
          count={report.files.length}
        />
        <Chip
          active={filter === "issues"}
          onClick={() => setFilter("issues")}
          label={t("client.filter_issues")}
          count={issues.length}
          tone="warn"
        />
        <Chip
          active={filter === "ok"}
          onClick={() => setFilter("ok")}
          label={t("client.filter_ok")}
          count={report.okFiles}
          tone="ok"
        />
        <Chip
          active={filter === "extra"}
          onClick={() => setFilter("extra")}
          label={t("client.filter_extra")}
          count={report.extraFiles.length}
        />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("client.search")}
          spellCheck={false}
          className="ml-auto w-48 rounded-lg border border-[var(--tb-border)] bg-[var(--surface)] px-2.5 py-1 font-mono text-[11px] text-[var(--text)] outline-none focus:border-accent"
        />
        {filter === "issues" && issues.length > 0 && (
          <button
            onClick={() =>
              setSelected(
                selected.size === issues.length ? new Set() : new Set(issues.map((f) => f.path)),
              )
            }
            className="rounded-lg border border-[var(--tb-border)] px-2.5 py-1 text-[11px] font-semibold text-text-dim hover:bg-[var(--surface-hover)]"
          >
            {t("client.select_all")}
          </button>
        )}
      </div>

      {filter === "extra" ? (
        <>
          <p className="shrink-0 text-[11px] text-text-dim">{t("client.extra_hint")}</p>
          <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
            {extras.map((p) => (
              <div
                key={p}
                style={{ contentVisibility: "auto", containIntrinsicSize: "0 30px" }}
                className="flex h-[30px] items-center border-b border-[var(--tb-border)] px-3 font-mono text-[11px] text-text-dim last:border-b-0"
              >
                <span className="truncate">{p}</span>
              </div>
            ))}
            {extras.length === 0 && (
              <p className="p-3 text-[11px] text-text-faint">{t("client.nothing_here")}</p>
            )}
          </div>
        </>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
          {rows.map((f) => (
            <FileRow
              key={f.path}
              file={f}
              checked={selected.has(f.path)}
              outdated={outdated}
              onToggle={() => {
                const next = new Set(selected);
                if (next.has(f.path)) next.delete(f.path);
                else next.add(f.path);
                setSelected(next);
              }}
            />
          ))}
          {rows.length === 0 && (
            <p className="p-3 text-[11px] text-text-faint">{t("client.nothing_here")}</p>
          )}
        </div>
      )}
    </div>
  );
}
