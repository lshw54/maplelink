/**
 * The bundled font is cut to the characters this interface uses
 * (`scripts/build-font-subset.py`). A string added afterwards can contain a
 * character the cut font does not have, and nothing about that is visible:
 * the browser quietly falls back to the system stack for that one character,
 * and the result is one glyph in a different typeface, in the middle of a word,
 * on someone else's machine.
 *
 * So the locales are checked against what the font actually holds.
 */
import { describe, expect, it } from "vitest";

import enUS from "../../locales/en-US.json";
import zhCN from "../../locales/zh-CN.json";
import zhTW from "../../locales/zh-TW.json";
// Written by the same script, in the same run, as the stylesheet's
// `unicode-range` declarations — the ranges as data, because a `?raw` import of
// CSS comes back empty under vitest.
import coverage from "../../assets/fonts/coverage.json";

/** Every code point the generated font covers. */
function coveredCodePoints(): Set<number> {
  const covered = new Set<number>();
  for (const [lo, hi] of coverage.ranges) {
    // JSON says `number[]`, not "two ends", so a malformed range is skipped
    // rather than asserted away — and the emptiness check below would catch a
    // generated file that was malformed throughout.
    if (lo === undefined || hi === undefined) continue;
    for (let cp = lo; cp <= hi; cp++) covered.add(cp);
  }
  return covered;
}

/**
 * Characters the font is not expected to carry: the ASCII the system font
 * renders identically anyway, and the private-use and emoji planes, which no
 * text font is asked for.
 */
function needsTheBundledFont(cp: number): boolean {
  if (cp < 0x80) return false;
  if (cp >= 0xe000 && cp <= 0xf8ff) return false;
  return cp < 0x1f000;
}

describe("the bundled font covers what the interface says", () => {
  const covered = coveredCodePoints();

  it("declares some ranges at all", () => {
    // A generator that silently produced nothing would otherwise pass every
    // test below by covering an empty set of requirements.
    expect(covered.size).toBeGreaterThan(500);
  });

  it.each([
    ["en-US", enUS],
    ["zh-CN", zhCN],
    ["zh-TW", zhTW],
  ])("has every character used by %s", (_name, messages) => {
    const missing = new Set<string>();
    for (const char of JSON.stringify(messages)) {
      const cp = char.codePointAt(0);
      if (cp !== undefined && needsTheBundledFont(cp) && !covered.has(cp)) {
        missing.add(char);
      }
    }
    expect(
      [...missing].join(""),
      "these characters would fall back to the system font — re-run scripts/build-font-subset.py",
    ).toBe("");
  });
});
