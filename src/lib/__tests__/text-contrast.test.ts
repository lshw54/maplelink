import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

// The stylesheet is the thing under test, so it is read rather than restated.
const css = readFileSync(new URL("../../styles/globals.css", import.meta.url), "utf8");

/**
 * Secondary text has to stay readable.
 *
 * `--text-dim` and `--text-faint` used to be picked by eye and drifted far
 * below the point where they could be read: 4.5 and 1.9 to one in dark, 2.9
 * and 1.7 in light. Players reported exactly that. These tests pin them to
 * WCAG AA's 4.5 for body text, measured against the worst surface each one
 * lands on, so the next adjustment by eye fails here instead of shipping.
 */
type Rgb = [number, number, number];

function relativeLuminance([r, g, b]: Rgb): number {
  const f = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrast(fg: Rgb, bg: Rgb): number {
  const a = relativeLuminance(fg);
  const b = relativeLuminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** Flatten a translucent white onto the surface behind it. */
function whiteAt(alpha: number, bg: Rgb): Rgb {
  return bg.map((c) => Math.round(255 * alpha + c * (1 - alpha))) as Rgb;
}

function hex(value: string): Rgb {
  const v = value.replace("#", "");
  return [parseInt(v.slice(0, 2), 16), parseInt(v.slice(2, 4), 16), parseInt(v.slice(4, 6), 16)];
}

/** hsl() the way `accent.ts` writes the tinted surfaces. */
function hsl(h: number, s: number, l: number): Rgb {
  const sat = s / 100;
  const light = l / 100;
  const c = (1 - Math.abs(2 * light - 1)) * sat;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = light - c / 2;
  const [r, g, b] =
    h < 60
      ? [c, x, 0]
      : h < 120
        ? [x, c, 0]
        : h < 180
          ? [0, c, x]
          : h < 240
            ? [0, x, c]
            : h < 300
              ? [x, 0, c]
              : [c, 0, x];
  return [r, g, b].map((v) => Math.round((v + m) * 255)) as Rgb;
}

/** The value of `name` inside the first block matching `scope`. */
function token(scope: string, name: string): string {
  const block = css.slice(css.indexOf(scope));
  const value = new RegExp(`${name}:\\s*([^;]+);`).exec(block)?.[1];
  if (value === undefined) throw new Error(`${name} not found after ${scope}`);
  return value.trim();
}

function alphaOf(value: string): number {
  const alpha = /rgba\(255,\s*255,\s*255,\s*([\d.]+)\)/.exec(value)?.[1];
  if (alpha === undefined) throw new Error(`not a translucent white: ${value}`);
  return Number(alpha);
}

/** WCAG AA for body text. */
const AA = 4.5;

describe("dark theme secondary text", () => {
  // The lightest surface text sits on, so the tightest contrast: --tb-card.
  const card = hex("#141619");

  it("dim text clears AA on the lightest surface", () => {
    const alpha = alphaOf(token(":root", "--text-dim"));
    expect(contrast(whiteAt(alpha, card), card)).toBeGreaterThanOrEqual(AA);
  });

  it("faint text clears AA on the lightest surface", () => {
    const alpha = alphaOf(token(":root", "--text-faint"));
    expect(contrast(whiteAt(alpha, card), card)).toBeGreaterThanOrEqual(AA);
  });

  it("clears AA on a card tinted by any accent hue", () => {
    // accent.ts sets --card-user to hsl(h 11% 8.8%).
    const dim = alphaOf(token(":root", "--text-dim"));
    const faint = alphaOf(token(":root", "--text-faint"));
    for (let h = 0; h < 360; h += 10) {
      const tinted = hsl(h, 11, 8.8);
      expect(contrast(whiteAt(dim, tinted), tinted), `dim at hue ${h}`).toBeGreaterThanOrEqual(AA);
      expect(contrast(whiteAt(faint, tinted), tinted), `faint at hue ${h}`).toBeGreaterThanOrEqual(
        AA,
      );
    }
  });

  it("keeps the three tones visibly apart", () => {
    const text = alphaOf(token(":root", "--text"));
    const dim = alphaOf(token(":root", "--text-dim"));
    const faint = alphaOf(token(":root", "--text-faint"));
    expect(text).toBeGreaterThan(dim);
    expect(dim).toBeGreaterThan(faint);
    expect(dim - faint).toBeGreaterThanOrEqual(0.1);
  });
});

describe("light theme secondary text", () => {
  // Here the page itself is the darkest surface, so it is the tightest.
  const page = hex("#e6e6ea");

  it("dim and faint text clear AA on the page", () => {
    expect(contrast(hex(token(".light", "--text-dim")), page)).toBeGreaterThanOrEqual(AA);
    expect(contrast(hex(token(".light", "--text-faint")), page)).toBeGreaterThanOrEqual(AA);
  });

  it("clears AA on a page tinted by any accent hue", () => {
    // accent.ts sets --bg-user-light to hsl(h 42% 90%).
    const dim = hex(token(".light", "--text-dim"));
    const faint = hex(token(".light", "--text-faint"));
    for (let h = 0; h < 360; h += 10) {
      const tinted = hsl(h, 42, 90);
      expect(contrast(dim, tinted), `dim at hue ${h}`).toBeGreaterThanOrEqual(AA);
      expect(contrast(faint, tinted), `faint at hue ${h}`).toBeGreaterThanOrEqual(AA);
    }
  });
});
