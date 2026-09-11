/**
 * Property-based tests for i18n locale resource files.
 *
 * Feature: maplelink-rewrite, Property 16: Locale resource files key completeness
 * Feature: maplelink-rewrite, Property 17: Error codes have translations in all locales
 */
import { describe, it, expect } from "vitest";
import * as fc from "fast-check";
import enUS from "../../locales/en-US.json";
import zhTW from "../../locales/zh-TW.json";
import zhCN from "../../locales/zh-CN.json";

type LocaleMap = Record<string, string>;

const locales: Record<string, LocaleMap> = {
  "en-US": enUS,
  "zh-TW": zhTW,
  "zh-CN": zhCN,
};

const localeNames = Object.keys(locales);

/**
 * All known backend error codes derived from the Rust error type hierarchy.
 * These correspond to the `ErrorDto.code` values produced by `From<AppError> for ErrorDto`.
 */

// ---------------------------------------------------------------------------
// Property 16: Locale resource files key completeness
// ---------------------------------------------------------------------------
describe("Property 16: Locale resource files key completeness", () => {
  it("all locale files have identical key sets", () => {
    const keySets: Record<string, Set<string>> = {};
    for (const name of localeNames) {
      keySets[name] = new Set(Object.keys(locales[name] as LocaleMap));
    }

    // Use fast-check to pick arbitrary pairs of locales and verify key equality
    fc.assert(
      fc.property(
        fc.constantFrom(...localeNames),
        fc.constantFrom(...localeNames),
        (localeA, localeB) => {
          const keysA = keySets[localeA] as Set<string>;
          const keysB = keySets[localeB] as Set<string>;

          const missingInB = [...keysA].filter((k) => !keysB.has(k));
          const missingInA = [...keysB].filter((k) => !keysA.has(k));

          expect(missingInB, `${localeB} is missing keys present in ${localeA}`).toEqual([]);
          expect(missingInA, `${localeA} is missing keys present in ${localeB}`).toEqual([]);
        },
      ),
      { numRuns: 100 },
    );
  });

  it("every placeholder is spelled {{name}}, the only form that interpolates", () => {
    // A single-brace `{name}` is silently rendered as literal text, so the
    // mistake reaches the screen rather than the tests. This catches it.
    const single = /(?<!\{)\{(\w+)\}(?!\})/;
    fc.assert(
      fc.property(fc.constantFrom(...localeNames), (localeName) => {
        const locale = locales[localeName] as LocaleMap;
        for (const [key, value] of Object.entries(locale)) {
          const found = single.exec(value);
          expect(
            found?.[0],
            `${localeName}["${key}"] has ${found?.[0]}, which never interpolates`,
          ).toBeUndefined();
        }
      }),
      { numRuns: 100 },
    );
  });

  it("a placeholder in one locale is present in all of them", () => {
    const names = (value: string | undefined) =>
      [...(value ?? "").matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort();
    for (const key of Object.keys(locales["en-US"] as LocaleMap)) {
      const expected = names((locales["en-US"] as LocaleMap)[key]);
      for (const localeName of localeNames) {
        expect(
          names((locales[localeName] as LocaleMap)[key]),
          `${localeName}["${key}"] does not take the same placeholders as en-US`,
        ).toEqual(expected);
      }
    }
  });

  it("no locale file has empty string values", () => {
    fc.assert(
      fc.property(fc.constantFrom(...localeNames), (localeName) => {
        const locale = locales[localeName] as LocaleMap;
        const entries = Object.entries(locale);
        for (const [key, value] of entries) {
          expect(value.trim(), `${localeName}["${key}"] is empty`).not.toBe("");
        }
      }),
      { numRuns: 100 },
    );
  });
});
