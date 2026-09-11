/**
 * A directory node built from flat, forward-slash paths.
 *
 * Both lists the scan produces are flat — the manifest's files and the extra
 * files found on disk — which is the right shape for comparing and a poor one
 * for reading: `Data/Map/Map001.wz` tells you nothing about how much of
 * `Data/Map` is fine. The tree is derived on demand so the flat lists stay the
 * source of truth.
 *
 * The leaf type is left open so the same builder serves a checked file and a
 * bare path; the caller says how to read a path out of one and whether it
 * counts as a problem.
 */
export interface DirNode<T> {
  name: string;
  /** Full path of this folder, used as the expand/collapse key. */
  path: string;
  dirs: Map<string, DirNode<T>>;
  files: T[];
  /** Totals for this folder and everything under it. */
  total: number;
  issues: number;
}

function emptyDir<T>(name: string, path: string): DirNode<T> {
  return { name, path, dirs: new Map(), files: [], total: 0, issues: 0 };
}

export function buildTree<T>(
  items: T[],
  pathOf: (item: T) => string,
  isIssue: (item: T) => boolean = () => false,
): DirNode<T> {
  const root = emptyDir<T>("", "");
  for (const item of items) {
    const issue = isIssue(item);
    // The last segment is the file name; everything before it is the folder.
    const parts = pathOf(item).split("/");
    parts.pop();
    let node = root;
    node.total += 1;
    if (issue) node.issues += 1;
    let prefix = "";
    for (const part of parts) {
      prefix = prefix ? `${prefix}/${part}` : part;
      let next = node.dirs.get(part);
      if (!next) {
        next = emptyDir<T>(part, prefix);
        node.dirs.set(part, next);
      }
      node = next;
      node.total += 1;
      if (issue) node.issues += 1;
    }
    node.files.push(item);
  }
  return root;
}

/** Just enough of a node to walk it, whatever its leaves are. */
interface DirLike {
  path: string;
  dirs: Map<string, DirLike>;
}

/** Every folder path in the tree, for "expand everything". */
export function allDirPaths(node: DirLike, into: Set<string> = new Set()): Set<string> {
  for (const child of node.dirs.values()) {
    into.add(child.path);
    allDirPaths(child, into);
  }
  return into;
}
