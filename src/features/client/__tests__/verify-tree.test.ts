import { describe, expect, it } from "vitest";
import { allDirPaths, buildTree } from "../file-tree";
import type { ClientCheckedFileDto } from "../../../lib/types";

function file(path: string, broken = false): ClientCheckedFileDto {
  return {
    path,
    kind: broken ? "hashMismatch" : null,
    expectedSize: 10,
    localSize: broken ? 9 : 10,
  };
}

describe("the folder view's tree", () => {
  it("nests manifest paths and keeps the flat list's order within a folder", () => {
    const tree = buildTree([
      file("MapleStory.exe"),
      file("Data/Map/Map001.wz"),
      file("Data/Map/Map002.wz"),
      file("Data/Mob/Mob001.wz"),
    ]);

    // Files with no folder stay at the root.
    expect(tree.files.map((f) => f.path)).toEqual(["MapleStory.exe"]);

    const data = tree.dirs.get("Data")!;
    expect([...data.dirs.keys()]).toEqual(["Map", "Mob"]);
    expect(data.dirs.get("Map")!.files.map((f) => f.path)).toEqual([
      "Data/Map/Map001.wz",
      "Data/Map/Map002.wz",
    ]);
  });

  it("counts a folder's files and problems, including nested ones", () => {
    const tree = buildTree([
      file("Data/Map/Map001.wz", true),
      file("Data/Map/Map002.wz"),
      file("Data/Mob/Mob001.wz", true),
    ]);

    expect(tree.total).toBe(3);
    expect(tree.issues).toBe(2);

    const data = tree.dirs.get("Data")!;
    expect(data.total).toBe(3);
    expect(data.issues).toBe(2);
    expect(data.dirs.get("Map")!.issues).toBe(1);
    expect(data.dirs.get("Mob")!.issues).toBe(1);
  });

  it("gives every folder a full path, so expanding one cannot open its namesake", () => {
    const tree = buildTree([file("Data/Map/a.wz"), file("Other/Map/b.wz")]);
    expect(allDirPaths(tree)).toEqual(new Set(["Data", "Data/Map", "Other", "Other/Map"]));
  });

  it("builds nothing from an empty scan", () => {
    const tree = buildTree([]);
    expect(tree.total).toBe(0);
    expect(tree.dirs.size).toBe(0);
  });
});
