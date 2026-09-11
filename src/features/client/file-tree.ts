import type { ClientCheckedFileDto } from "../../lib/types";

/**
 * A directory node built from the flat manifest paths.
 *
 * The manifest is a flat list of forward-slash paths, which is the right shape
 * for comparing but a poor one for reading: `Data/Map/Map001.wz` tells you
 * nothing about how much of `Data/Map` is fine. The tree is derived on demand
 * so the flat list stays the source of truth.
 */
export interface DirNode {
  name: string;
  /** Full path of this folder, used as the expand/collapse key. */
  path: string;
  dirs: Map<string, DirNode>;
  files: ClientCheckedFileDto[];
  /** Totals for this folder and everything under it. */
  total: number;
  issues: number;
}

function emptyDir(name: string, path: string): DirNode {
  return { name, path, dirs: new Map(), files: [], total: 0, issues: 0 };
}

export function buildTree(files: ClientCheckedFileDto[]): DirNode {
  const root = emptyDir("", "");
  for (const file of files) {
    // The last segment is the file name; everything before it is the folder.
    const parts = file.path.split("/");
    parts.pop();
    let node = root;
    node.total += 1;
    if (file.kind !== null) node.issues += 1;
    let prefix = "";
    for (const part of parts) {
      prefix = prefix ? `${prefix}/${part}` : part;
      let next = node.dirs.get(part);
      if (!next) {
        next = emptyDir(part, prefix);
        node.dirs.set(part, next);
      }
      node = next;
      node.total += 1;
      if (file.kind !== null) node.issues += 1;
    }
    node.files.push(file);
  }
  return root;
}

/** Every folder path in the tree, for "expand everything". */
export function allDirPaths(node: DirNode, into: Set<string> = new Set()): Set<string> {
  for (const child of node.dirs.values()) {
    into.add(child.path);
    allDirPaths(child, into);
  }
  return into;
}
