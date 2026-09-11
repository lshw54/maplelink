import { describe, expect, it } from "vitest";
import { allDirPaths, buildTree, type DirNode } from "../file-tree";
import type { ClientCheckedFileDto } from "../../../lib/types";

/** Look a folder up, failing the test rather than asserting it is there. */
function dir<T>(node: DirNode<T>, name: string): DirNode<T> {
  const found = node.dirs.get(name);
  if (!found) throw new Error(`no folder named ${name}`);
  return found;
}

function file(path: string, broken = false): ClientCheckedFileDto {
  return {
    path,
    kind: broken ? "hashMismatch" : null,
    expectedSize: 10,
    localSize: broken ? 9 : 10,
  };
}

/** How the verify panel builds the tree of checked files. */
function treeOf(files: ClientCheckedFileDto[]) {
  return buildTree(
    files,
    (f) => f.path,
    (f) => f.kind !== null,
  );
}

describe("the folder view's tree", () => {
  it("nests manifest paths and keeps the flat list's order within a folder", () => {
    const tree = treeOf([
      file("MapleStory.exe"),
      file("Data/Map/Map001.wz"),
      file("Data/Map/Map002.wz"),
      file("Data/Mob/Mob001.wz"),
    ]);

    // Files with no folder stay at the root.
    expect(tree.files.map((f) => f.path)).toEqual(["MapleStory.exe"]);

    const data = dir(tree, "Data");
    expect([...data.dirs.keys()]).toEqual(["Map", "Mob"]);
    expect(dir(data, "Map").files.map((f) => f.path)).toEqual([
      "Data/Map/Map001.wz",
      "Data/Map/Map002.wz",
    ]);
  });

  it("counts a folder's files and problems, including nested ones", () => {
    const tree = treeOf([
      file("Data/Map/Map001.wz", true),
      file("Data/Map/Map002.wz"),
      file("Data/Mob/Mob001.wz", true),
    ]);

    expect(tree.total).toBe(3);
    expect(tree.issues).toBe(2);

    const data = dir(tree, "Data");
    expect(data.total).toBe(3);
    expect(data.issues).toBe(2);
    expect(dir(data, "Map").issues).toBe(1);
    expect(dir(data, "Mob").issues).toBe(1);
  });

  it("gives every folder a full path, so expanding one cannot open its namesake", () => {
    const tree = treeOf([file("Data/Map/a.wz"), file("Other/Map/b.wz")]);
    expect(allDirPaths(tree)).toEqual(new Set(["Data", "Data/Map", "Other", "Other/Map"]));
  });

  it("builds nothing from an empty scan", () => {
    const tree = treeOf([]);
    expect(tree.total).toBe(0);
    expect(tree.dirs.size).toBe(0);
  });

  it("groups the extra files too, which arrive as bare paths", () => {
    // Nothing is compared for these, so every count is a plain file count.
    const tree = buildTree(["Data/Map/old.wz", "Data/leftover.log", "note.txt"], (p) => p);

    expect(tree.total).toBe(3);
    expect(tree.issues).toBe(0);
    expect(tree.files).toEqual(["note.txt"]);

    const data = dir(tree, "Data");
    expect(data.total).toBe(2);
    expect(data.files).toEqual(["Data/leftover.log"]);
    expect(dir(data, "Map").files).toEqual(["Data/Map/old.wz"]);
  });
});
