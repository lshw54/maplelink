/**
 * User-selectable accent colour. The stylesheet defines the maple-orange
 * default as CSS variables; picking another colour overrides them inline on
 * <html>, and clearing it removes the overrides so the stylesheet wins again.
 */

export const DEFAULT_ACCENT = "#e8a23a";

/** Built-in swatches. Chosen so white text stays readable on a filled button. */
export const ACCENT_PRESETS: { key: string; hex: string }[] = [
  { key: "maple", hex: DEFAULT_ACCENT },
  { key: "blue", hex: "#3b82f6" },
  { key: "teal", hex: "#14b8a6" },
  { key: "green", hex: "#22c55e" },
  { key: "purple", hex: "#a855f7" },
  { key: "pink", hex: "#ec4899" },
  { key: "red", hex: "#ef4444" },
];

export function isHexColor(v: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(v);
}

type Rgb = [number, number, number];

function parse(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex(rgb: Rgb): string {
  return "#" + rgb.map((c) => Math.round(c).toString(16).padStart(2, "0")).join("");
}

/** Mix towards black by `amount` (0..1). */
function darken(rgb: Rgb, amount: number): Rgb {
  return [rgb[0] * (1 - amount), rgb[1] * (1 - amount), rgb[2] * (1 - amount)];
}

/** WCAG relative luminance (0 = black, 1 = white). */
function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** Hue in degrees (0..360) of an sRGB colour. */
function hue([r, g, b]: Rgb): number {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  if (max === min) return 0;
  const d = max - min;
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return Math.round(((h * 60) % 360 < 0 ? h * 60 + 360 : h * 60) * 10) / 10;
}

const VARS = [
  "--accent",
  "--accent-rgb",
  "--accent-dark",
  "--accent-deep",
  "--on-accent",
  "--bg-user",
  "--nav-user",
  "--card-user",
  "--input-user",
  "--bg-user-light",
  "--nav-user-light",
  "--card-user-light",
  "--input-user-light",
];

/** Apply an accent colour app-wide; empty or invalid restores the default. */
export function applyAccent(hex: string): void {
  const root = document.documentElement;
  const value = hex.trim().toLowerCase();
  if (!isHexColor(value) || value === DEFAULT_ACCENT) {
    for (const v of VARS) root.style.removeProperty(v);
    return;
  }
  const rgb = parse(value);
  root.style.setProperty("--accent", value);
  root.style.setProperty("--accent-rgb", rgb.map(Math.round).join(", "));
  root.style.setProperty("--accent-dark", toHex(darken(rgb, 0.15)));
  root.style.setProperty("--accent-deep", toHex(darken(rgb, 0.25)));
  // Light accents (yellow, lime…) need dark text on top to stay legible.
  root.style.setProperty("--on-accent", luminance(rgb) > 0.55 ? "#1d1d1f" : "#ffffff");

  // Tint the solid backgrounds with the accent's hue. Saturation/lightness
  // mirror the default palette (whose near-blacks carry S≈25%, L≈5%), so only
  // the temperature changes — contrast stays exactly as designed.
  const h = hue(rgb);
  const set = (name: string, s: number, l: number) =>
    root.style.setProperty(name, `hsl(${h} ${s}% ${l}%)`);
  set("--bg-user", 25, 4.7);
  set("--nav-user", 23, 5.1);
  set("--card-user", 11, 8.8);
  set("--input-user", 14, 6.9);
  set("--bg-user-light", 11, 96.5);
  set("--nav-user-light", 8, 90);
  set("--card-user-light", 20, 99);
  set("--input-user-light", 20, 97.5);
}
