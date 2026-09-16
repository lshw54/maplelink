import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import type {
  ClientCheckedFileDto,
  ClientFileState,
  ClientIssueKind,
  ClientScanReportDto,
} from "../../lib/types";
import { allDirPaths, buildTree, type DirNode } from "./file-tree";

export type Filter = "all" | "issues" | "ok" | "failed" | "extra";

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

function FolderRow<T>({
  node,
  depth,
  open,
  onToggle,
}: {
  node: DirNode<T>;
  depth: number;
  open: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      onClick={onToggle}
      style={{ paddingLeft: 10 + depth * 16 }}
      // Tinted and bold, so a folder reads as a heading over the rows it owns
      // rather than as one more line in the list.
      className="flex h-[30px] w-full items-center gap-2 border-b border-[var(--tb-border)] bg-[rgba(255,255,255,0.025)] pr-3 text-left last:border-b-0 hover:bg-[var(--surface-hover)]"
    >
      <span className="w-3 shrink-0 text-center text-[9px] text-text-dim">{open ? "▾" : "▸"}</span>
      <span className="shrink-0 text-[11px] text-text-dim">{open ? "📂" : "📁"}</span>
      <span className="min-w-0 flex-1 truncate text-[11px] font-bold text-[var(--text)]">
        {node.name}
      </span>
      {node.issues > 0 && (
        <span className="shrink-0 text-[10px] font-bold text-yellow-500">
          {t("client.folder_issues", { count: String(node.issues) })}
        </span>
      )}
      <span className="w-16 shrink-0 text-right font-mono text-[10px] text-text-faint">
        {t("client.folder_files", { count: String(node.total) })}
      </span>
    </button>
  );
}

/**
 * One level of the tree: its folders (each recursing when open), then its own
 * files. The leaf is drawn by the caller, so the same walk serves the manifest
 * rows and the bare paths of the extra-files list.
 */
function TreeLevel<T>({
  node,
  depth,
  expanded,
  toggle,
  renderFile,
}: {
  node: DirNode<T>;
  depth: number;
  expanded: Set<string>;
  toggle: (path: string) => void;
  renderFile: (item: T, depth: number) => React.ReactNode;
}) {
  return (
    <>
      {[...node.dirs.values()].map((child) => {
        const open = expanded.has(child.path);
        return (
          <div key={child.path}>
            <FolderRow node={child} depth={depth} open={open} onToggle={() => toggle(child.path)} />
            {open && (
              <TreeLevel
                node={child}
                depth={depth + 1}
                expanded={expanded}
                toggle={toggle}
                renderFile={renderFile}
              />
            )}
          </div>
        );
      })}
      {node.files.map((item) => renderFile(item, depth))}
    </>
  );
}

/**
 * A file the manifest does not mention: path only, nothing to compare. It can
 * be picked for the Recycle Bin, and starts unpicked — these are as likely to
 * be the player's own settings and screenshots as leftovers.
 */
function ExtraRow({
  path,
  label,
  indent = 12,
  checked,
  onToggle,
  disabled,
}: {
  path: string;
  label?: string;
  indent?: number;
  checked: boolean;
  onToggle: () => void;
  disabled: boolean;
}) {
  return (
    <label
      style={{ contentVisibility: "auto", containIntrinsicSize: "0 30px", paddingLeft: indent }}
      className="flex h-[30px] cursor-pointer items-center gap-2.5 border-b border-[var(--tb-border)] pr-3 font-mono text-[11px] text-text-dim last:border-b-0 hover:bg-[var(--surface-hover)]"
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        disabled={disabled}
        className="h-3.5 w-3.5 shrink-0 accent-[var(--accent)]"
      />
      <span title={path} className="truncate">
        {label ?? path}
      </span>
    </label>
  );
}

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
  tone?: "ok" | "warn" | "bad";
}) {
  const colour =
    tone === "ok"
      ? "text-green-500"
      : tone === "warn"
        ? "text-yellow-500"
        : tone === "bad"
          ? "text-red-400"
          : "text-[var(--text)]";
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

/** How a row reads while a download is running, and after it. */
const LIVE: Record<
  ClientFileState,
  { label: string; colour: string; mark: string; pulse?: boolean }
> = {
  downloading: {
    label: "client.status_downloading",
    colour: "text-accent",
    mark: "↓",
    pulse: true,
  },
  done: { label: "client.status_repaired", colour: "text-green-500", mark: "✓" },
  failed: { label: "client.status_failed", colour: "text-red-400", mark: "!" },
};

function FileRow({
  file,
  checked,
  outdated,
  onToggle,
  label,
  indent = 12,
  state,
  fraction,
}: {
  file: ClientCheckedFileDto;
  checked: boolean;
  outdated: boolean;
  onToggle: () => void;
  /** What to show in the path column; defaults to the full manifest path. */
  label?: string;
  /** Left padding in pixels, so a nested row lines up under its folder. */
  indent?: number;
  /** What the running download has done with this file, if anything yet. */
  state?: ClientFileState;
  /** How much of this file has arrived, 0–1, while it is in flight. */
  fraction?: number;
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
  // A file being fetched right now is no longer described by the scan that
  // found it: what it is doing matters more than what was wrong with it.
  const live = state ? LIVE[state] : null;

  return (
    <div
      // 1263 rows: let the engine skip what is scrolled out of view.
      style={{ contentVisibility: "auto", containIntrinsicSize: "0 30px", paddingLeft: indent }}
      className="relative flex h-[30px] items-center gap-2.5 border-b border-[var(--tb-border)] pr-3 last:border-b-0 hover:bg-[var(--surface-hover)]"
    >
      {/* This file's own transfer, along the bottom edge of its row. A column
          of its own would cost width on every one of 1,263 rows; a rule that
          fills as the file arrives costs none, and reads at a glance down the
          handful of rows that are moving. */}
      {fraction !== undefined && (
        <span
          aria-hidden
          className="absolute bottom-0 left-0 h-[2px] rounded-full bg-accent transition-[width] duration-300"
          style={{ width: `${Math.min(100, Math.max(0, fraction * 100))}%` }}
        />
      )}
      {live ? (
        <span
          className={`w-3.5 shrink-0 text-center text-[11px] ${live.colour} ${
            live.pulse ? "animate-pulse" : ""
          }`}
        >
          {live.mark}
        </span>
      ) : issue ? (
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
        {label ?? file.path}
      </span>
      <span
        title={why}
        className={`w-24 shrink-0 cursor-help text-right text-[10px] font-semibold ${
          live ? live.colour : issue ? "text-yellow-500" : "text-text-faint"
        }`}
      >
        {live
          ? `${t(live.label)}${fraction === undefined ? "" : ` ${Math.round(fraction * 100)}%`}`
          : issue
            ? t(issueLabel(kind, outdated))
            : t("client.status_ok")}
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
  live,
  inFlight,
  filter,
  setFilter,
  extraSelected,
  setExtraSelected,
  onRemoveExtra,
  extraBusy,
  extraNotice,
}: {
  report: ClientScanReportDto | null;
  selected: Set<string>;
  setSelected: (next: Set<string>) => void;
  /** The installed client is behind the official version, per the version marker. */
  outdated?: boolean;
  /** What the running download has reached so far, path by path. */
  live?: ReadonlyMap<string, ClientFileState>;
  /** How far each file still in flight has got, 0–1. */
  inFlight?: ReadonlyMap<string, number>;
  /** Owned by the window, which points it at the work when a repair starts. */
  filter: Filter;
  setFilter: (next: Filter) => void;
  /** Extra files picked for the Recycle Bin. */
  extraSelected: Set<string>;
  setExtraSelected: (next: Set<string>) => void;
  /** Ask to move the picked extra files; the window confirms first. */
  onRemoveExtra: () => void;
  /** A clean-up or another job is running. */
  extraBusy: boolean;
  /** What the last clean-up did, when there was one. */
  extraNotice: string | null;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  // The flat list is the default: it is what the scan produces, and it reads
  // straight down. The tree is for answering "is this folder alright".
  const [view, setView] = useState<"list" | "folder">("list");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  /**
   * The scan said what was wrong; the repair since then says what still is.
   *
   * A file that has just been written is no longer a problem, so it leaves
   * "needs work" and joins "fine" as it lands. That is what makes the list
   * useful while a repair runs: it counts down to nothing, and whatever is
   * being fetched right now sits at the top of what is left.
   */
  const { issues, fine, failed } = useMemo(() => {
    const issues: ClientCheckedFileDto[] = [];
    const fine: ClientCheckedFileDto[] = [];
    const failed: ClientCheckedFileDto[] = [];
    for (const f of report?.files ?? []) {
      const state = live?.get(f.path);
      if (state === "failed") failed.push(f);
      if (state === "done" || (f.kind === null && state !== "failed")) fine.push(f);
      else if (f.kind !== null) issues.push(f);
    }
    return { issues, fine, failed };
  }, [report, live]);

  const rows = useMemo(() => {
    if (!report) return [];
    const base =
      filter === "issues"
        ? issues
        : filter === "ok"
          ? fine
          : filter === "failed"
            ? failed
            : report.files;
    const needle = query.trim().toLowerCase();
    return needle ? base.filter((f) => f.path.toLowerCase().includes(needle)) : base;
  }, [report, issues, fine, failed, filter, query]);

  const extras = useMemo(() => {
    if (!report) return [];
    const needle = query.trim().toLowerCase();
    return needle
      ? report.extraFiles.filter((p) => p.toLowerCase().includes(needle))
      : report.extraFiles;
  }, [report, query]);

  const tree = useMemo(
    () =>
      buildTree(
        rows,
        (f) => f.path,
        (f) => f.kind !== null,
      ),
    [rows],
  );
  // The extra files are a flat list of paths too, and a folder that is entirely
  // leftovers is exactly what someone wants to see grouped.
  const extraTree = useMemo(() => buildTree(extras, (p) => p), [extras]);
  const shownTree = filter === "extra" ? extraTree : tree;

  // A search or an issues-only filter narrows things to a handful, and leaving
  // those collapsed would hide the very rows the player asked for.
  const narrowed = query.trim() !== "" || filter === "issues" || filter === "failed";
  const openDirs = useMemo(
    () => (narrowed ? allDirPaths(shownTree) : expanded),
    [narrowed, shownTree, expanded],
  );
  const allOpen = useMemo(
    () => allDirPaths(shownTree).size > 0 && allDirPaths(shownTree).size === openDirs.size,
    [shownTree, openDirs],
  );

  const toggleDir = useCallback((path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

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
          count={fine.length}
          tone="ok"
        />
        {/* Only worth a chip when something did fail; an always-present zero
            just makes the row longer. */}
        {failed.length > 0 && (
          <Chip
            active={filter === "failed"}
            onClick={() => setFilter("failed")}
            label={t("client.filter_failed")}
            count={failed.length}
            tone="bad"
          />
        )}
        <Chip
          active={filter === "extra"}
          onClick={() => setFilter("extra")}
          label={t("client.filter_extra")}
          count={report.extraFiles.length}
        />
        {/* List or folders. Two segments rather than a dropdown: there are
            only ever two, and the current one has to be readable at a glance. */}
        <div className="ml-auto flex shrink-0 overflow-hidden rounded-lg border border-[var(--tb-border)]">
          {(["list", "folder"] as const).map((id) => (
            <button
              key={id}
              onClick={() => setView(id)}
              className={`px-2.5 py-1 text-[11px] font-semibold transition-colors ${
                view === id
                  ? "bg-[var(--surface-hover)] text-[var(--text)]"
                  : "text-text-dim hover:bg-[var(--surface-hover)]"
              }`}
            >
              {t(`client.view_${id}`)}
            </button>
          ))}
        </div>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("client.search")}
          spellCheck={false}
          className="w-44 rounded-lg border border-[var(--tb-border)] bg-[var(--surface)] px-2.5 py-1 font-mono text-[11px] text-[var(--text)] outline-none focus:border-accent"
        />
        {view === "folder" && !narrowed && (
          <button
            onClick={() => setExpanded(allOpen ? new Set() : allDirPaths(shownTree))}
            className="rounded-lg border border-[var(--tb-border)] px-2.5 py-1 text-[11px] font-semibold text-text-dim hover:bg-[var(--surface-hover)]"
          >
            {t(allOpen ? "client.collapse_all" : "client.expand_all")}
          </button>
        )}
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
          <div className="flex shrink-0 items-center gap-2">
            <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-text-dim">
              {t("client.extra_hint")}
            </p>
            {extras.length > 0 && (
              <button
                onClick={() => {
                  // Over what is shown: a search narrows what "all" means.
                  const shown = extras.every((p) => extraSelected.has(p));
                  const next = new Set(extraSelected);
                  for (const p of extras) {
                    if (shown) next.delete(p);
                    else next.add(p);
                  }
                  setExtraSelected(next);
                }}
                disabled={extraBusy}
                className="shrink-0 rounded-lg border border-[var(--tb-border)] px-2.5 py-1 text-[11px] font-semibold text-text-dim hover:bg-[var(--surface-hover)] disabled:opacity-50"
              >
                {t("client.select_all")}
              </button>
            )}
            <button
              onClick={onRemoveExtra}
              disabled={extraBusy || extraSelected.size === 0}
              className="shrink-0 rounded-lg border border-[rgba(239,68,68,0.4)] px-2.5 py-1 text-[11px] font-semibold text-red-400 transition-colors hover:bg-[rgba(239,68,68,0.1)] disabled:border-[var(--tb-border)] disabled:text-text-faint disabled:hover:bg-transparent"
            >
              {t("client.extra_remove", { count: String(extraSelected.size) })}
            </button>
          </div>
          {extraNotice && <p className="shrink-0 text-[11px] text-green-500">{extraNotice}</p>}
          <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
            {view === "folder" ? (
              <TreeLevel
                node={extraTree}
                depth={0}
                expanded={openDirs}
                toggle={toggleDir}
                renderFile={(path, depth) => (
                  <ExtraRow
                    key={path}
                    path={path}
                    label={path.split("/").pop() ?? path}
                    indent={10 + (depth + 1) * 16}
                    checked={extraSelected.has(path)}
                    onToggle={() => {
                      const next = new Set(extraSelected);
                      if (next.has(path)) next.delete(path);
                      else next.add(path);
                      setExtraSelected(next);
                    }}
                    disabled={extraBusy}
                  />
                )}
              />
            ) : (
              extras.map((p) => (
                <ExtraRow
                  key={p}
                  path={p}
                  checked={extraSelected.has(p)}
                  onToggle={() => {
                    const next = new Set(extraSelected);
                    if (next.has(p)) next.delete(p);
                    else next.add(p);
                    setExtraSelected(next);
                  }}
                  disabled={extraBusy}
                />
              ))
            )}
            {extras.length === 0 && (
              <p className="p-3 text-[11px] text-text-faint">{t("client.nothing_here")}</p>
            )}
          </div>
        </>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-[var(--tb-border)] bg-[var(--tb-card)]">
          {view === "folder" ? (
            <TreeLevel
              node={tree}
              depth={0}
              expanded={openDirs}
              toggle={toggleDir}
              renderFile={(f, depth) => (
                <FileRow
                  key={f.path}
                  file={f}
                  // Nested rows show the file name; the full path is in the tooltip.
                  label={f.path.split("/").pop() ?? f.path}
                  indent={10 + (depth + 1) * 16}
                  checked={selected.has(f.path)}
                  outdated={outdated}
                  state={live?.get(f.path)}
                  fraction={inFlight?.get(f.path)}
                  onToggle={() => {
                    const next = new Set(selected);
                    if (next.has(f.path)) next.delete(f.path);
                    else next.add(f.path);
                    setSelected(next);
                  }}
                />
              )}
            />
          ) : (
            rows.map((f) => (
              <FileRow
                key={f.path}
                file={f}
                checked={selected.has(f.path)}
                outdated={outdated}
                state={live?.get(f.path)}
                fraction={inFlight?.get(f.path)}
                onToggle={() => {
                  const next = new Set(selected);
                  if (next.has(f.path)) next.delete(f.path);
                  else next.add(f.path);
                  setSelected(next);
                }}
              />
            ))
          )}
          {rows.length === 0 && (
            <p className="p-3 text-[11px] text-text-faint">{t("client.nothing_here")}</p>
          )}
        </div>
      )}
    </div>
  );
}
